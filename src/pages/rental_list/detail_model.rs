//! 园区详情聚合模型，对齐原系统园区、厂房、楼层、设施与宿舍关系。

use std::collections::BTreeMap;

use spacetimedb_sdk::Timestamp;

use crate::spacetime_bindings::{
    dormitory_image_preview_type::DormitoryImagePreview, dormitory_type::Dormitory,
    elevator_asset_type::ElevatorAsset, elevator_inspection_type::ElevatorInspection,
    factory_floor_image_preview_type::FactoryFloorImagePreview,
    dormitory_floor_type::DormitoryFloor, factory_floor_type::FactoryFloor,
    factory_type::Factory, firefighting_asset_type::FirefightingAsset,
    firefighting_inspection_type::FirefightingInspection,
    park_image_preview_type::ParkImagePreview, park_type::Park,
    rental_tenant_floor_type::RentalTenantFloor, rental_tenant_type::RentalTenant,
    transformer_asset_type::TransformerAsset,
    transformer_inspection_type::TransformerInspection,
};

use crate::pages::{floor_used_areas, park_areas, ParkAreas};

#[derive(Clone, PartialEq)]
pub(super) struct ParkDetailRecord {
    pub park: Park,
    pub images: Vec<String>,
    /// 园区面积。厂房与宿舍分开计量，都由楼层台账算出，不再人工填写。
    pub areas: ParkAreas,
    /// 每个厂房楼层被合同占用的面积，供下游区块直接查。
    pub floor_used: BTreeMap<u64, i64>,
    pub used_area: i64,
    pub available_area: i64,
    pub factories: Vec<FactoryDetailRecord>,
    pub dormitories: Vec<DormitoryDetailRecord>,
}

#[derive(Clone, PartialEq)]
pub(super) struct FactoryDetailRecord {
    pub factory: Factory,
    pub floors: Vec<FloorDetailRecord>,
    pub total_area: i64,
    pub used_area: i64,
    pub available_area: i64,
    pub firefighting: Option<FirefightingSummaryView>,
    pub transformer: Option<DeviceFacilityView>,
    pub elevator: Option<DeviceFacilityView>,
}

/// 设施概览卡里列出的一台设备。
///
/// 卡片是体检报告而不是设备清单，因此每台只留「叫什么、状态如何、什么时候
/// 检的」三项——规格容量这类属于维护管理页（见 园区管理.md §2.3.1）。
#[derive(Clone, PartialEq)]
pub(super) struct FacilityEntry {
    pub name: String,
    /// 最近一次巡检状态；从未巡检为 `None`。
    pub status: Option<String>,
    /// 最近一次巡检日期 `MM-DD`；从未巡检为 `None`。
    pub checked_on: Option<String>,
}

impl FacilityEntry {
    pub fn status_label(&self) -> &str {
        self.status.as_deref().unwrap_or("未巡检")
    }

    pub fn abnormal(&self) -> bool {
        self.status.as_deref() == Some("异常")
    }
}

/// 变压器、电梯这类「少数大件」的概览：逐台点名有意义，但列表要有界。
#[derive(Clone, PartialEq)]
pub(super) struct DeviceFacilityView {
    pub total: usize,
    /// 最近一次巡检为异常的台数。
    pub abnormal: usize,
    /// 卡片上实际列出的设备，异常优先、其次按巡检时间倒序，至多 `MAX_FACILITY_ENTRIES` 条。
    pub entries: Vec<FacilityEntry>,
}

impl DeviceFacilityView {
    /// 列表之外还剩多少台没显示。
    pub fn overflow(&self) -> usize {
        self.total.saturating_sub(self.entries.len())
    }
}

/// 消防设施这类「大量同类小件」的概览：逐个点名没意义，按类型汇总。
#[derive(Clone, PartialEq)]
pub(super) struct FirefightingSummaryView {
    pub total: usize,
    /// 按设施类型分组的数量，多的排前面。
    pub by_type: Vec<(String, usize)>,
    /// 最近一次巡检为异常的设施数。
    pub abnormal: usize,
    /// 已过有效期的设施数（主要是灭火器）。
    pub expired: usize,
}

/// 概览卡最多列出的设备台数，超出的折成「还有 N 台」。
pub(super) const MAX_FACILITY_ENTRIES: usize = 3;

#[derive(Clone, PartialEq)]
pub(super) struct FloorDetailRecord {
    pub floor: FactoryFloor,
    pub images: Vec<String>,
    /// 这一层被有效合同占用的面积，由合同↔楼层关联算出。
    pub used_area: i64,
}

#[derive(Clone, PartialEq)]
pub(super) struct DormitoryDetailRecord {
    pub dormitory: Dormitory,
    pub images: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_park_detail(
    park_id: u64,
    parks: &[Park],
    park_images: &[ParkImagePreview],
    factories: &[Factory],
    floors: &[FactoryFloor],
    floor_images: &[FactoryFloorImagePreview],
    dormitories: &[Dormitory],
    dormitory_floors: &[DormitoryFloor],
    dormitory_images: &[DormitoryImagePreview],
    tenants: &[RentalTenant],
    tenant_floors: &[RentalTenantFloor],
    firefighting_assets: &[FirefightingAsset],
    firefighting_inspections: &[FirefightingInspection],
    transformer_assets: &[TransformerAsset],
    transformer_inspections: &[TransformerInspection],
    elevator_assets: &[ElevatorAsset],
    elevator_inspections: &[ElevatorInspection],
    // today：今天（`YYYY-MM-DD`），由组件读时钟后传入，本模块不碰时钟。
    today: &str,
) -> Option<ParkDetailRecord> {
    let floor_used = floor_used_areas(tenant_floors, tenants);
    let areas = park_areas(park_id, factories, floors, dormitories, dormitory_floors);
    let park = parks
        .iter()
        .find(|row| row.park_id == park_id && !row.is_deleted)?
        .clone();

    let mut floor_image_map = BTreeMap::<u64, Vec<String>>::new();
    for image in floor_images {
        floor_image_map
            .entry(image.floor_id)
            .or_default()
            .push(image.img_url.clone());
    }
    let mut dormitory_image_map = BTreeMap::<u64, Vec<String>>::new();
    for image in dormitory_images {
        dormitory_image_map
            .entry(image.dormitory_id)
            .or_default()
            .push(image.img_url.clone());
    }

    let mut factory_rows = factories
        .iter()
        .filter(|row| row.park_id == park_id && !row.is_deleted)
        .map(|factory| {
            let mut floor_rows = floors
                .iter()
                .filter(|row| row.factory_id == factory.factory_id && !row.is_deleted)
                .map(|floor| FloorDetailRecord {
                    used_area: floor_used.get(&floor.floor_id).copied().unwrap_or(0),
                    floor: floor.clone(),
                    images: floor_image_map
                        .get(&floor.floor_id)
                        .cloned()
                        .unwrap_or_default(),
                })
                .collect::<Vec<_>>();
            floor_rows.sort_by_key(|row| row.floor.floor_id);
            let total_area = floor_rows
                .iter()
                .map(|row| row.floor.total_area_centi_square_metres.max(0))
                .sum();
            let used_area = floor_rows.iter().map(|row| row.used_area.max(0)).sum();
            let floor_ids = floor_rows
                .iter()
                .map(|row| row.floor.floor_id)
                .collect::<Vec<_>>();
            FactoryDetailRecord {
                factory: factory.clone(),
                firefighting: firefighting_summary(
                    firefighting_assets,
                    firefighting_inspections,
                    &floor_ids,
                    today,
                ),
                floors: floor_rows,
                total_area,
                used_area,
                available_area: (total_area - used_area).max(0),
                transformer: transformer_view(
                    transformer_assets,
                    transformer_inspections,
                    factory.factory_id,
                ),
                elevator: elevator_view(elevator_assets, elevator_inspections, factory.factory_id),
            }
        })
        .collect::<Vec<_>>();
    factory_rows.sort_by_key(|row| row.factory.factory_id);

    let used_area = factory_rows.iter().map(|row| row.used_area).sum();
    let available_area = factory_rows.iter().map(|row| row.available_area).sum();
    let mut dormitory_rows = dormitories
        .iter()
        .filter(|row| row.park_id == park_id && !row.is_deleted)
        .map(|dormitory| DormitoryDetailRecord {
            dormitory: dormitory.clone(),
            images: dormitory_image_map
                .get(&dormitory.dormitory_id)
                .cloned()
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    dormitory_rows.sort_by_key(|row| row.dormitory.dormitory_id);

    Some(ParkDetailRecord {
        park,
        images: park_images
            .iter()
            .filter(|row| row.park_id == park_id)
            .map(|row| row.img_url.clone())
            .collect(),
        areas,
        floor_used,
        used_area,
        available_area,
        factories: factory_rows,
        dormitories: dormitory_rows,
    })
}

/// 消防设施按类型汇总。`today` 由组件传入（`YYYY-MM-DD`），本函数不读时钟。
fn firefighting_summary(
    assets: &[FirefightingAsset],
    inspections: &[FirefightingInspection],
    floor_ids: &[u64],
    today: &str,
) -> Option<FirefightingSummaryView> {
    let matching = assets
        .iter()
        .filter(|row| !row.is_deleted && floor_ids.contains(&row.factory_floor_id))
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return None;
    }
    let total = matching.len();
    let mut abnormal = 0;
    let mut expired = 0;
    let mut counts = BTreeMap::<String, usize>::new();
    for asset in matching {
        *counts.entry(asset.facility_type.clone()).or_default() += 1;
        if crate::pages::latest_firefighting_inspection(inspections, asset.asset_id)
            .is_some_and(|row| row.status == "异常")
        {
            abnormal += 1;
        }
        if asset
            .expiry_on
            .as_deref()
            .and_then(|value| crate::pages::expiry_days_left(value, today))
            .is_some_and(|days| days < 0)
        {
            expired += 1;
        }
    }
    let mut by_type = counts.into_iter().collect::<Vec<_>>();
    // 数量多的排前面，同数量按类型名排，输出稳定。
    by_type.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    Some(FirefightingSummaryView {
        total,
        by_type,
        abnormal,
        expired,
    })
}

/// 把「设备名 + 最近巡检」装成概览行，并按「异常优先、其次巡检时间倒序」排好。
///
/// 排序口径是这三张卡的核心：要出问题的东西必须先被看见，而不是按录入顺序。
fn rank_entries(mut rows: Vec<(FacilityEntry, Option<Timestamp>)>) -> (usize, Vec<FacilityEntry>) {
    rows.sort_by(|left, right| {
        right
            .0
            .abnormal()
            .cmp(&left.0.abnormal())
            .then(right.1.cmp(&left.1))
    });
    let abnormal = rows.iter().filter(|row| row.0.abnormal()).count();
    (
        abnormal,
        rows.into_iter()
            .map(|row| row.0)
            .take(MAX_FACILITY_ENTRIES)
            .collect(),
    )
}

fn transformer_view(
    assets: &[TransformerAsset],
    inspections: &[TransformerInspection],
    factory_id: u64,
) -> Option<DeviceFacilityView> {
    let rows = assets
        .iter()
        .filter(|row| row.factory_id == factory_id && !row.is_deleted)
        .map(|asset| {
            let latest = crate::pages::latest_inspection(inspections, asset.asset_id);
            (
                FacilityEntry {
                    name: asset.transformer_name.clone(),
                    status: latest.as_ref().map(|row| row.status.clone()),
                    checked_on: latest.as_ref().map(|row| short_date(row.check_time)),
                },
                latest.as_ref().map(|row| row.check_time),
            )
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }
    let total = rows.len();
    let (abnormal, entries) = rank_entries(rows);
    Some(DeviceFacilityView {
        total,
        abnormal,
        entries,
    })
}

fn elevator_view(
    assets: &[ElevatorAsset],
    inspections: &[ElevatorInspection],
    factory_id: u64,
) -> Option<DeviceFacilityView> {
    let rows = assets
        .iter()
        .filter(|row| row.factory_id == factory_id && !row.is_deleted)
        .map(|asset| {
            let latest = crate::pages::latest_elevator_inspection(inspections, asset.asset_id);
            (
                FacilityEntry {
                    name: asset.elevator_name.clone(),
                    status: latest.as_ref().map(|row| row.status.clone()),
                    checked_on: latest.as_ref().map(|row| short_date(row.check_time)),
                },
                latest.as_ref().map(|row| row.check_time),
            )
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return None;
    }
    let total = rows.len();
    let (abnormal, entries) = rank_entries(rows);
    Some(DeviceFacilityView {
        total,
        abnormal,
        entries,
    })
}

/// 概览行只需要「几月几号」，完整时间在维护页看。
fn short_date(value: Timestamp) -> String {
    let text = format_datetime(value);
    text.get(5..10).unwrap_or(&text).to_string()
}

pub(super) fn format_datetime(value: Timestamp) -> String {
    const MICROS_PER_DAY: i64 = 86_400_000_000;
    const MICROS_PER_SECOND: i64 = 1_000_000;
    const CHINA_OFFSET_MICROS: i64 = 8 * 3_600_000_000;
    let local = value.to_micros_since_unix_epoch() + CHINA_OFFSET_MICROS;
    let days = local.div_euclid(MICROS_PER_DAY);
    let seconds = local.rem_euclid(MICROS_PER_DAY) / MICROS_PER_SECOND;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds / 3600;
    let minute = seconds % 3600 / 60;
    let second = seconds % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

pub(super) fn format_optional_datetime(value: Option<Timestamp>) -> String {
    value
        .map(format_datetime)
        .unwrap_or_else(|| "未更新".into())
}

pub(super) fn format_centi(value: Option<i64>) -> String {
    value.map(format_fixed).unwrap_or_else(|| "--".into())
}

pub(super) fn format_money(value: Option<i64>) -> String {
    value.map(format_fixed).unwrap_or_else(|| "--".into())
}

fn format_fixed(value: i64) -> String {
    let negative = value < 0;
    let absolute = value.saturating_abs();
    let text = if absolute % 100 == 0 {
        (absolute / 100).to_string()
    } else {
        format!("{}.{:02}", absolute / 100, absolute % 100)
            .trim_end_matches('0')
            .to_string()
    };
    if negative {
        format!("-{text}")
    } else {
        text
    }
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year as i32, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 详情数值最多显示两位小数() {
        assert_eq!(format_centi(Some(12_345)), "123.45");
        assert_eq!(format_money(Some(1_200)), "12");
        assert_eq!(format_centi(None), "--");
    }
}
