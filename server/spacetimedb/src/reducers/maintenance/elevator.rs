//! 电梯资产台账与巡检记录维护。
//!
//! 与变压器同一模型（资产 + 巡检分离、巡检只增不删），差异只有一条：
//! **电梯必须属于某个厂房**——`factory_id` 必填非 0，`park_id` 由服务端从
//! 厂房推导写入，不接受客户端直传，从结构上排除电梯与园区归属矛盾。
//! 完整设计见 `docs/电梯台账与扫码巡检.md`。

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
            current_customer_id, require_factory, require_image, require_menu_path,
            require_park_access, require_park_access_unless_archived,
        },
        platform::media::image_reducer::{delete_image_if_unreferenced, ensure_image},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct ElevatorAssetInput {
    pub elevator_name: String,
    pub location: String,
    pub size: Option<String>,
    pub load_capacity_centi_kg: u64,
    pub production_date: Option<String>,
    pub remark: Option<String>,
    /// 必填非 0——电梯必须属于某个厂房；园区由厂房推导，客户端不传。
    pub factory_id: u64,
}

#[derive(SpacetimeType)]
pub struct ElevatorInspectionInput {
    pub asset_id: u64,
    pub status: String,
    pub abnormal_note: Option<String>,
    /// 不传时取提交时刻；传入用于补录。
    pub check_time: Option<Timestamp>,
    pub remark: Option<String>,
}

#[spacetimedb::reducer]
pub fn create_elevator_asset(
    ctx: &ReducerContext,
    input: ElevatorAssetInput,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/elevator")?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_asset(ctx, 0, customer_id.clone(), input)?;
    require_park_access(ctx, row.park_id)?;
    let asset_id = ctx.db.elevator_asset().insert(row).asset_id;
    replace_asset_images(ctx, asset_id, customer_id, Vec::new(), uploads)
}

#[spacetimedb::reducer]
pub fn update_elevator_asset(
    ctx: &ReducerContext,
    id: u64,
    input: ElevatorAssetInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/elevator")?;
    let existing = require_elevator_asset(ctx, id)?;
    let mut row = validated_asset(ctx, id, existing.customer_id.clone(), input)?;
    require_park_access(ctx, row.park_id)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.elevator_asset().asset_id().update(row);
    replace_asset_images(ctx, id, existing.customer_id, existing_image_ids, uploads)
}

/// 注销资产（软删除）。巡检记录随资产一起从视图消失，但数据保留。
#[spacetimedb::reducer]
pub fn delete_elevator_asset(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/elevator")?;
    let mut existing = require_elevator_asset(ctx, id)?;
    // 园区已归档时放行范围检查，否则归档园区名下的设备永远清不掉。
    require_park_access_unless_archived(ctx, existing.park_id)?;
    existing.is_deleted = true;
    existing.updated_at = Some(ctx.timestamp);
    ctx.db.elevator_asset().asset_id().update(existing);
    Ok(())
}

#[spacetimedb::reducer]
pub fn create_elevator_inspection(
    ctx: &ReducerContext,
    input: ElevatorInspectionInput,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    let inspector_user_id = require_inspector(ctx)?;
    let inspector_name = inspector_name_snapshot(ctx, inspector_user_id);
    let asset = require_elevator_asset(ctx, input.asset_id)?;
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
    let row = ctx.db.elevator_inspection().insert(ElevatorInspection {
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
            .elevator_inspection_image()
            .insert(ElevatorInspectionImage {
                id: 0,
                inspection_id: row.inspection_id,
                img_id,
                created_at: ctx.timestamp,
                updated_at: None,
            });
    }
    Ok(())
}

fn require_elevator_asset(ctx: &ReducerContext, id: u64) -> Result<ElevatorAsset, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .elevator_asset()
        .asset_id()
        .find(id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("电梯不存在或已注销".into())
}

fn validated_asset(
    ctx: &ReducerContext,
    asset_id: u64,
    customer_id: String,
    input: ElevatorAssetInput,
) -> Result<ElevatorAsset, String> {
    if input.factory_id == 0 {
        return Err("请选择所在厂房".into());
    }
    // 园区由厂房推导：客户端不传 park_id，结构上不存在"电梯挂 A 厂房、
    // 园区却填了 B"的矛盾。
    let factory = require_factory(ctx, input.factory_id)?;
    let elevator_name = required_text(input.elevator_name, "设备名称不能为空")?;
    let location = required_text(input.location, "位置描述不能为空")?;
    validate_max_length(&elevator_name, 100, "设备名称不能超过100个字符")?;
    validate_max_length(&location, 200, "位置描述不能超过200个字符")?;
    Ok(ElevatorAsset {
        asset_id,
        customer_id,
        park_id: factory.park_id,
        factory_id: input.factory_id,
        elevator_name,
        location,
        size: limited_optional(input.size, 100, "轿厢尺寸不能超过100个字符")?,
        load_capacity_centi_kg: input.load_capacity_centi_kg,
        production_date: limited_optional(input.production_date, 20, "生产日期格式不正确")?,
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
        return Err(format!("每台设备最多保留 {MAX_MAINTENANCE_IMAGES} 张图片"));
    }
    let old_links = ctx
        .db
        .elevator_asset_image()
        .elevator_asset_image_by_owner()
        .filter(asset_id)
        .collect::<Vec<_>>();
    for link in &old_links {
        ctx.db.elevator_asset_image().id().delete(link.id);
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
        ctx.db.elevator_asset_image().insert(ElevatorAssetImage {
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
