//! 打卡地点的维护，以及打卡时的范围判定。
//!
//! 距离计算是纯函数 [`distance_metres`]，判定是纯函数 [`check_punch_range`]，
//! 都不碰数据库——算错的后果是员工在公司门口打不了卡、或者在家能打卡，这两种
//! 都只能靠测试发现，界面上看不出来。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::common::validate_coordinate;
use crate::{
    reducers::shared::{
        access::{current_customer_id, require_admin},
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 坐标的定点倍数，与 `Attendance` 一致。
const COORD_SCALE: f64 = 1e15;
/// 地球平均半径，米。
const EARTH_RADIUS_METRES: f64 = 6_371_000.0;

/// 半径下限。城区 GPS 误差常有 20～100 米，比这更小会把正常上班的人挡在门外。
const MIN_RADIUS_METRES: i32 = 50;
/// 半径上限。再大就失去「限定地点」的意义了。
const MAX_RADIUS_METRES: i32 = 20_000;

/// 打卡点可修改字段。
#[derive(SpacetimeType)]
pub struct AttendanceLocationInput {
    pub location_name: String,
    pub address: Option<String>,
    pub longitude_e15: i64,
    pub latitude_e15: i64,
    pub radius_metres: i32,
    pub is_enabled: bool,
    pub remark: Option<String>,
}

/// 两点间的地表距离，米。
///
/// 用等距圆柱近似而不是 Haversine：几百米量级两者差别在千分之几以内，而这里的
/// 判定阈值本身就有几十米的 GPS 误差，多出来的精度没有意义。
///
/// 经度差要乘 `cos(纬度)`——同样是 0.001 度，在赤道约 111 米，在北纬 40 度只有
/// 85 米。不乘的话越往北，实际允许范围越大。
pub(super) fn distance_metres(
    longitude_a_e15: i64,
    latitude_a_e15: i64,
    longitude_b_e15: i64,
    latitude_b_e15: i64,
) -> f64 {
    let lon_a = longitude_a_e15 as f64 / COORD_SCALE;
    let lat_a = latitude_a_e15 as f64 / COORD_SCALE;
    let lon_b = longitude_b_e15 as f64 / COORD_SCALE;
    let lat_b = latitude_b_e15 as f64 / COORD_SCALE;

    let mean_lat_rad = ((lat_a + lat_b) / 2.0).to_radians();
    let delta_lon = (lon_b - lon_a).to_radians() * mean_lat_rad.cos();
    let delta_lat = (lat_b - lat_a).to_radians();
    EARTH_RADIUS_METRES * (delta_lon * delta_lon + delta_lat * delta_lat).sqrt()
}

/// 打卡位置的判定结果。
#[derive(Debug, PartialEq)]
pub(super) enum PunchRange {
    /// 一个启用的打卡点都没有，不做限制。
    Unrestricted,
    /// 落在某个打卡点范围内。
    InRange { location_name: String },
    /// 都不在范围内，附上最近的那个点和距离，好让员工知道差多远。
    OutOfRange {
        nearest_name: String,
        metres: i64,
        radius_metres: i32,
    },
}

/// 判定这个坐标能不能打卡。
///
/// 只看启用且未删除的点；一个都没有就放行——不设置就是原来的行为，设了才开始管。
#[pure_function::pure]
pub(super) fn check_punch_range(
    locations: &[AttendanceLocation],
    longitude_e15: i64,
    latitude_e15: i64,
) -> PunchRange {
    let mut nearest: Option<(&AttendanceLocation, f64)> = None;
    for location in locations
        .iter()
        .filter(|row| row.is_enabled && !row.is_deleted)
    {
        let metres = distance_metres(
            longitude_e15,
            latitude_e15,
            location.longitude_e15,
            location.latitude_e15,
        );
        if metres <= location.radius_metres as f64 {
            return PunchRange::InRange {
                location_name: location.location_name.clone(),
            };
        }
        if nearest.is_none_or(|(_, best)| metres < best) {
            nearest = Some((location, metres));
        }
    }
    match nearest {
        None => PunchRange::Unrestricted,
        Some((location, metres)) => PunchRange::OutOfRange {
            nearest_name: location.location_name.clone(),
            metres: metres.round() as i64,
            radius_metres: location.radius_metres,
        },
    }
}

/// 超范围时给员工看的说明。
///
/// 一定要带上「差多远」：只说「不在范围内」的话，站在门口因为 GPS 漂移被挡下来
/// 的人不知道是该往前走两步，还是这个点根本没设对。
pub(super) fn out_of_range_message(nearest_name: &str, metres: i64, radius_metres: i32) -> String {
    let distance = if metres >= 1_000 {
        format!("{:.1} 公里", metres as f64 / 1_000.0)
    } else {
        format!("{metres} 米")
    };
    format!(
        "不在打卡范围内：距最近的打卡点「{nearest_name}」{distance}，该点允许 {radius_metres} 米以内。"
    )
}

/// 打卡时校验位置。放行返回 `Ok`，超范围返回给员工看的说明。
pub(super) fn require_punch_in_range(
    ctx: &ReducerContext,
    customer_id: &str,
    longitude_e15: i64,
    latitude_e15: i64,
) -> Result<(), String> {
    let locations = ctx
        .db
        .attendance_location()
        .attendance_location_by_customer()
        .filter(customer_id)
        .collect::<Vec<_>>();
    match check_punch_range(&locations, longitude_e15, latitude_e15) {
        PunchRange::Unrestricted | PunchRange::InRange { .. } => Ok(()),
        PunchRange::OutOfRange {
            nearest_name,
            metres,
            radius_metres,
        } => Err(out_of_range_message(&nearest_name, metres, radius_metres)),
    }
}

#[spacetimedb::reducer]
pub fn create_attendance_location(
    ctx: &ReducerContext,
    input: AttendanceLocationInput,
) -> Result<(), String> {
    require_admin(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_location(ctx, 0, customer_id, input)?;
    ctx.db.attendance_location().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_attendance_location(
    ctx: &ReducerContext,
    location_id: u64,
    input: AttendanceLocationInput,
) -> Result<(), String> {
    require_admin(ctx)?;
    let existing = require_location(ctx, location_id)?;
    let mut row = validated_location(ctx, location_id, existing.customer_id.clone(), input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.attendance_location().location_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_attendance_location(ctx: &ReducerContext, location_id: u64) -> Result<(), String> {
    require_admin(ctx)?;
    let mut row = require_location(ctx, location_id)?;
    row.is_deleted = true;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.attendance_location().location_id().update(row);
    Ok(())
}

fn require_location(
    ctx: &ReducerContext,
    location_id: u64,
) -> Result<AttendanceLocation, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .attendance_location()
        .location_id()
        .find(location_id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("未找到打卡点".into())
}

fn validated_location(
    ctx: &ReducerContext,
    location_id: u64,
    customer_id: String,
    input: AttendanceLocationInput,
) -> Result<AttendanceLocation, String> {
    let location_name = required_text(input.location_name, "打卡点名称不能为空")?;
    validate_max_length(&location_name, 50, "打卡点名称不能超过 50 个字符")?;
    // 复用打卡时的那条校验：经纬度必须是合法坐标，不能是定位失败留下的 0。
    validate_coordinate(input.longitude_e15, input.latitude_e15)?;
    if !(MIN_RADIUS_METRES..=MAX_RADIUS_METRES).contains(&input.radius_metres) {
        return Err(format!(
            "允许半径必须在 {MIN_RADIUS_METRES} 到 {MAX_RADIUS_METRES} 米之间"
        ));
    }
    let address = normalize_optional_text(input.address);
    if let Some(address) = &address {
        validate_max_length(address, 200, "参考地址不能超过 200 个字符")?;
    }
    let remark = normalize_optional_text(input.remark);
    if let Some(remark) = &remark {
        validate_max_length(remark, 200, "备注不能超过 200 个字符")?;
    }
    Ok(AttendanceLocation {
        location_id,
        customer_id,
        location_name,
        address,
        longitude_e15: input.longitude_e15,
        latitude_e15: input.latitude_e15,
        radius_metres: input.radius_metres,
        is_enabled: input.is_enabled,
        is_deleted: false,
        remark,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use spacetimedb::Timestamp;

    fn e15(degrees: f64) -> i64 {
        (degrees * COORD_SCALE) as i64
    }

    fn location(name: &str, lon: f64, lat: f64, radius: i32) -> AttendanceLocation {
        AttendanceLocation {
            location_id: 1,
            customer_id: "public".into(),
            location_name: name.into(),
            address: None,
            longitude_e15: e15(lon),
            latitude_e15: e15(lat),
            radius_metres: radius,
            is_enabled: true,
            is_deleted: false,
            remark: None,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 同一点距离为零() {
        assert_eq!(distance_metres(e15(113.1), e15(23.0), e15(113.1), e15(23.0)), 0.0);
    }

    #[test]
    fn 纬度每千分之一度约一百一十一米() {
        // 纬度是均匀的：0.001° × 6371000 × π/180 ≈ 111.19 米
        let metres = distance_metres(e15(113.0), e15(23.0), e15(113.0), e15(23.001));
        assert!((metres - 111.19).abs() < 0.5, "实际 {metres}");
    }

    #[test]
    fn 经度距离随纬度收窄() {
        // 同样 0.001 度经度：赤道约 111 米，北纬 40 度只有约 85 米。
        // 不乘 cos(纬度) 的话越往北实际允许范围越大，等于半径悄悄放宽。
        let at_equator = distance_metres(e15(113.0), e15(0.0), e15(113.001), e15(0.0));
        let at_lat40 = distance_metres(e15(113.0), e15(40.0), e15(113.001), e15(40.0));
        assert!((at_equator - 111.19).abs() < 0.5, "赤道 {at_equator}");
        assert!((at_lat40 - 85.2).abs() < 1.0, "北纬 40 度 {at_lat40}");
        assert!(at_lat40 < at_equator);
    }

    #[test]
    fn 一个打卡点都没有时不做限制() {
        // 这张表刚上线、老板还没设置时，所有人必须照常能打卡。
        assert_eq!(
            check_punch_range(&[], e15(113.1), e15(23.0)),
            PunchRange::Unrestricted
        );
    }

    #[test]
    fn 停用和已删除的点不参与判定() {
        let mut disabled = location("已停用的门", 113.1, 23.0, 300);
        disabled.is_enabled = false;
        let mut removed = location("已删除的门", 113.1, 23.0, 300);
        removed.is_deleted = true;
        // 全部不算数，等于一个点都没有——不能因为存在一条停用记录就把人挡在外面。
        assert_eq!(
            check_punch_range(&[disabled, removed], e15(113.1), e15(23.0)),
            PunchRange::Unrestricted
        );
    }

    #[test]
    fn 落在任一点范围内即可打卡() {
        let locations = vec![
            location("总部办公室", 113.100, 23.000, 300),
            location("园区门卫", 113.200, 23.000, 200),
        ];
        // 在第二个点旁边，离第一个点十公里开外，照样算数。
        let range = check_punch_range(&locations, e15(113.2005), e15(23.0));
        assert_eq!(
            range,
            PunchRange::InRange {
                location_name: "园区门卫".into()
            }
        );
    }

    #[test]
    fn 超出范围时给出最近的点和距离() {
        let locations = vec![
            location("总部办公室", 113.100, 23.000, 300),
            location("园区门卫", 113.200, 23.000, 200),
        ];
        // 站在两点之间偏向门卫的位置，两个都够不着。
        let range = check_punch_range(&locations, e15(113.190), e15(23.000));
        match range {
            PunchRange::OutOfRange {
                nearest_name,
                metres,
                radius_metres,
            } => {
                // 报最近的那个，而不是列表里的第一个。
                assert_eq!(nearest_name, "园区门卫");
                assert_eq!(radius_metres, 200);
                assert!((metres - 1_024).abs() < 20, "实际 {metres} 米");
            }
            other => panic!("应当超出范围，实际 {other:?}"),
        }
    }

    #[test]
    fn 边界上算在范围内() {
        // 半径 300 米，距离约 111 米，肯定在内；把半径压到 111 米以下才算超出。
        let inside = check_punch_range(
            &[location("门", 113.0, 23.001, 300)],
            e15(113.0),
            e15(23.0),
        );
        assert!(matches!(inside, PunchRange::InRange { .. }));
        let outside = check_punch_range(
            &[location("门", 113.0, 23.001, MIN_RADIUS_METRES)],
            e15(113.0),
            e15(23.0),
        );
        assert!(matches!(outside, PunchRange::OutOfRange { .. }));
    }

    #[test]
    fn 超范围的说明要带上差多远() {
        // 只说「不在范围内」的话，站在门口被 GPS 漂移挡下来的人不知道该怎么办。
        let near = out_of_range_message("总部办公室", 420, 300);
        assert!(near.contains("总部办公室"), "{near}");
        assert!(near.contains("420 米"), "{near}");
        assert!(near.contains("300 米以内"), "{near}");
        // 上千米改用公里，否则「1234 米」读起来要换算。
        let far = out_of_range_message("总部办公室", 1_234, 300);
        assert!(far.contains("1.2 公里"), "{far}");
    }
}
