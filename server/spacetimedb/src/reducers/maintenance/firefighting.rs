//! 消防设施资产台账与巡检记录维护。
//!
//! 与变压器、电梯同一模型（资产 + 巡检分离、巡检只增不删），自身规则两条：
//! **一行一个设施**（类型：灭火器/消防栓/消防出口/应急照明/其他），且设施
//! 必须挂在**厂房楼层或宿舍楼层**之一上——两个楼层外键恰好一个非 0，
//! `park_id` 由服务端沿楼层反查推导。灭火器必须填有效期，供前端做到期提醒。
//! 完整设计见 `docs/消防设施台账与扫码巡检.md`。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::{
    inspector_name_snapshot, limited_optional, require_inspector, MAX_MAINTENANCE_IMAGES,
};
use super::transformer::{
    check_abnormal_note, check_inspection_status, UploadedMaintenanceImageInput,
};
use crate::{
    reducers::{
        access::{
            current_customer_id, require_dormitory, require_dormitory_floor, require_factory,
            require_factory_floor, require_image, require_menu_path, require_park_access,
            require_park_access_unless_archived,
        },
        platform::media::image_reducer::{delete_image_if_unreferenced, ensure_image},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 设施类型的合法取值。
const FACILITY_TYPES: [&str; 5] = ["灭火器", "消防栓", "消防出口", "应急照明", "其他"];

#[derive(SpacetimeType)]
pub struct FirefightingAssetInput {
    pub facility_type: String,
    pub facility_name: String,
    pub location: String,
    pub specifications: Option<String>,
    pub expiry_on: Option<String>,
    pub remark: Option<String>,
    /// 所在厂房楼层；`0` = 不在厂房。与宿舍楼层恰好一个非 0。
    pub factory_floor_id: u64,
    /// 所在宿舍楼层；`0` = 不在宿舍。
    pub dormitory_floor_id: u64,
}

#[derive(SpacetimeType)]
pub struct FirefightingInspectionInput {
    pub asset_id: u64,
    pub status: String,
    pub abnormal_note: Option<String>,
    /// 不传时取提交时刻；传入用于补录。
    pub check_time: Option<Timestamp>,
    pub remark: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_firefighting_asset(
    ctx: &ReducerContext,
    input: FirefightingAssetInput,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/firefighting")?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_asset(ctx, 0, customer_id.clone(), input)?;
    require_park_access(ctx, row.park_id)?;
    let asset_id = ctx.db.firefighting_asset().insert(row).asset_id;
    replace_asset_images(ctx, asset_id, customer_id, Vec::new(), uploads)
}

#[spacetimedb::reducer]
pub fn update_firefighting_asset(
    ctx: &ReducerContext,
    id: u64,
    input: FirefightingAssetInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/firefighting")?;
    let existing = require_firefighting_asset(ctx, id)?;
    let mut row = validated_asset(ctx, id, existing.customer_id.clone(), input)?;
    require_park_access(ctx, row.park_id)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.firefighting_asset().asset_id().update(row);
    replace_asset_images(ctx, id, existing.customer_id, existing_image_ids, uploads)
}

/// 注销设施（软删除）。巡检记录随资产一起从视图消失，但数据保留。
#[spacetimedb::reducer]
pub fn delete_firefighting_asset(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/firefighting")?;
    let mut existing = require_firefighting_asset(ctx, id)?;
    // 园区已归档时放行范围检查，否则归档园区名下的设施永远清不掉。
    require_park_access_unless_archived(ctx, existing.park_id)?;
    existing.is_deleted = true;
    existing.updated_at = Some(ctx.timestamp);
    ctx.db.firefighting_asset().asset_id().update(existing);
    Ok(())
}

#[spacetimedb::reducer]
pub fn create_firefighting_inspection(
    ctx: &ReducerContext,
    input: FirefightingInspectionInput,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    let inspector_user_id = require_inspector(ctx)?;
    let inspector_name = inspector_name_snapshot(ctx, inspector_user_id);
    let asset = require_firefighting_asset(ctx, input.asset_id)?;
    require_park_access(ctx, asset.park_id)?;
    let status = required_text(input.status, "运行状态不能为空")?;
    check_inspection_status(&status)?;
    let abnormal_note = limited_optional(input.abnormal_note, 200, "异常描述不能超过200个字符")?;
    check_abnormal_note(&status, abnormal_note.as_deref())?;
    let remark = limited_optional(input.remark, 200, "备注不能超过200个字符")?;
    if uploads.len() > MAX_MAINTENANCE_IMAGES {
        return Err(format!("每次巡检最多上传 {MAX_MAINTENANCE_IMAGES} 张照片"));
    }
    let customer_id = asset.customer_id.clone();
    let row = ctx
        .db
        .firefighting_inspection()
        .insert(FirefightingInspection {
            inspection_id: 0,
            customer_id: asset.customer_id,
            asset_id: asset.asset_id,
            status,
            abnormal_note,
            inspector_user_id,
            inspector_name,
            check_time: input.check_time.unwrap_or(ctx.timestamp),
            remark,
            created_at: ctx.timestamp,
        });
    // 巡检记录只增不删，照片只在创建时写入一次，没有替换场景。
    for upload in uploads {
        let img_id = ensure_image(ctx, customer_id.clone(), upload.img_url, upload.hash)?;
        ctx.db
            .firefighting_inspection_image()
            .insert(FirefightingInspectionImage {
                id: 0,
                inspection_id: row.inspection_id,
                img_id,
                created_at: ctx.timestamp,
                updated_at: None,
            });
    }
    Ok(())
}

fn require_firefighting_asset(ctx: &ReducerContext, id: u64) -> Result<FirefightingAsset, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .firefighting_asset()
        .asset_id()
        .find(id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("消防设施不存在或已注销".into())
}

/// 沿楼层反查园区归属：厂房楼层 → 厂房 → 园区，或宿舍楼层 → 宿舍楼 → 园区。
fn resolve_park(
    ctx: &ReducerContext,
    factory_floor_id: u64,
    dormitory_floor_id: u64,
) -> Result<u64, String> {
    if factory_floor_id != 0 {
        let floor = require_factory_floor(ctx, factory_floor_id)?;
        let factory = require_factory(ctx, floor.factory_id)?;
        return Ok(factory.park_id);
    }
    let floor = require_dormitory_floor(ctx, dormitory_floor_id)?;
    let dormitory = require_dormitory(ctx, floor.dormitory_id)?;
    Ok(dormitory.park_id)
}

fn validated_asset(
    ctx: &ReducerContext,
    asset_id: u64,
    customer_id: String,
    input: FirefightingAssetInput,
) -> Result<FirefightingAsset, String> {
    check_firefighting_location(input.factory_floor_id, input.dormitory_floor_id)?;
    let park_id = resolve_park(ctx, input.factory_floor_id, input.dormitory_floor_id)?;
    let facility_type = required_text(input.facility_type, "设施类型不能为空")?;
    check_facility_type(&facility_type)?;
    let expiry_on = limited_optional(input.expiry_on, 20, "有效期格式不正确")?;
    check_extinguisher_expiry(&facility_type, expiry_on.as_deref())?;
    let facility_name = required_text(input.facility_name, "设施编号或名称不能为空")?;
    let location = required_text(input.location, "位置描述不能为空")?;
    validate_max_length(&facility_name, 100, "设施编号或名称不能超过100个字符")?;
    validate_max_length(&location, 200, "位置描述不能超过200个字符")?;
    Ok(FirefightingAsset {
        asset_id,
        customer_id,
        park_id,
        factory_floor_id: input.factory_floor_id,
        dormitory_floor_id: input.dormitory_floor_id,
        facility_type,
        facility_name,
        location,
        specifications: limited_optional(input.specifications, 100, "型号规格不能超过100个字符")?,
        expiry_on,
        remark: limited_optional(input.remark, 200, "备注不能超过200个字符")?,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}

fn replace_asset_images(
    ctx: &ReducerContext,
    asset_id: u64,
    customer_id: String,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    if existing_image_ids.len() + uploads.len() > MAX_MAINTENANCE_IMAGES {
        return Err(format!("每个设施最多保留 {MAX_MAINTENANCE_IMAGES} 张图片"));
    }
    let old_links = ctx
        .db
        .firefighting_asset_image()
        .firefighting_asset_image_by_owner()
        .filter(asset_id)
        .collect::<Vec<_>>();
    for link in &old_links {
        ctx.db.firefighting_asset_image().id().delete(link.id);
    }

    let mut image_ids = BTreeSet::new();
    for img_id in existing_image_ids {
        require_image(ctx, img_id)?;
        image_ids.insert(img_id);
    }
    for upload in uploads {
        image_ids.insert(ensure_image(
            ctx,
            customer_id.clone(),
            upload.img_url,
            upload.hash,
        )?);
    }
    for img_id in &image_ids {
        ctx.db
            .firefighting_asset_image()
            .insert(FirefightingAssetImage {
                id: 0,
                asset_id,
                img_id: *img_id,
                created_at: ctx.timestamp,
                updated_at: None,
            });
    }
    for img_id in old_links.into_iter().map(|link| link.img_id) {
        if !image_ids.contains(&img_id) {
            delete_image_if_unreferenced(ctx, img_id);
        }
    }
    Ok(())
}

/// 位置校验：厂房楼层与宿舍楼层恰好一个非 0——消防设施必须在楼层中，
/// 没有"园区公共区域"口子（园区没有室外消防设施）。
#[pure_function::pure]
pub(crate) fn check_firefighting_location(
    factory_floor_id: u64,
    dormitory_floor_id: u64,
) -> Result<(), String> {
    match (factory_floor_id, dormitory_floor_id) {
        (0, 0) => Err("消防设施必须挂在一个厂房楼层或宿舍楼层上".into()),
        (_, 0) | (0, _) => Ok(()),
        _ => Err("消防设施只能挂在一个楼层上".into()),
    }
}

/// 设施类型必须落在固定枚举内，台账筛选与统计口径依赖它。
#[pure_function::pure]
pub(crate) fn check_facility_type(facility_type: &str) -> Result<(), String> {
    if FACILITY_TYPES.contains(&facility_type) {
        Ok(())
    } else {
        Err("设施类型只能是：灭火器、消防栓、消防出口、应急照明、其他".into())
    }
}

/// 灭火器有法定有效期，必须填写「有效期至」，到期提醒依赖它。
#[pure_function::pure]
pub(crate) fn check_extinguisher_expiry(
    facility_type: &str,
    expiry_on: Option<&str>,
) -> Result<(), String> {
    if facility_type == "灭火器" && expiry_on.is_none_or(|value| value.trim().is_empty()) {
        return Err("灭火器必须填写有效期至".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 消防设施必须恰好挂在一个楼层上() {
        assert!(check_firefighting_location(3, 0).is_ok());
        assert!(check_firefighting_location(0, 5).is_ok());
        assert!(check_firefighting_location(0, 0).is_err());
        assert!(check_firefighting_location(3, 5).is_err());
    }

    #[test]
    fn 设施类型只接受固定枚举() {
        assert!(check_facility_type("灭火器").is_ok());
        assert!(check_facility_type("应急照明").is_ok());
        assert!(check_facility_type("摄像头").is_err());
        assert!(check_facility_type("").is_err());
    }

    #[test]
    fn 灭火器必须填写有效期() {
        assert!(check_extinguisher_expiry("灭火器", None).is_err());
        assert!(check_extinguisher_expiry("灭火器", Some("  ")).is_err());
        assert!(check_extinguisher_expiry("灭火器", Some("2027-06-30")).is_ok());
        assert!(check_extinguisher_expiry("消防栓", None).is_ok());
    }
}
