//! 变压器资产台账与巡检记录维护。
//!
//! 资产与巡检分离：`transformer_asset` 一行一台真实设备，增删改要求维护菜单
//! 权限；`transformer_inspection` 一行一次巡检事件，**只增不删**——不提供更新
//! 或删除 Reducer，填错了补一条更正记录。巡检提交要求显式授予的
//! `maintenance:inspect` 权限码，巡检人取自登录会话，不可代填。
//! 完整设计见 `docs/变压器台账与扫码巡检.md`。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::{
    inspector_name_snapshot, limited_optional, require_inspector, require_transformer_asset,
    validate_location, MAX_MAINTENANCE_IMAGES,
};
use crate::{
    reducers::{
        access::{
            current_customer_id, require_image, require_menu_path, require_park_access,
            require_park_access_unless_archived,
        },
        platform::media::image_reducer::{delete_image_if_unreferenced, ensure_image},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

/// 用户确认保存后已经上传到 R2 的维护域图片。
#[derive(SpacetimeType)]
pub struct UploadedMaintenanceImageInput {
    pub img_url: String,
    pub hash: String,
}

#[derive(SpacetimeType)]
pub struct TransformerAssetInput {
    pub transformer_name: String,
    pub location: String,
    pub specifications: String,
    pub capacity_centi_kw: u64,
    pub commissioned_on: Option<String>,
    pub remark: Option<String>,
    /// `0` 表示园区公共区域（配电房、室外等）。
    pub factory_id: u64,
    pub park_id: u64,
}

#[derive(SpacetimeType)]
pub struct TransformerInspectionInput {
    pub asset_id: u64,
    pub status: String,
    pub abnormal_note: Option<String>,
    /// 不传时取提交时刻；传入用于补录。
    pub check_time: Option<Timestamp>,
    pub remark: Option<String>,
}

/// 建台账并写入设备图片，两者在同一事务内完成（与园区建档同一手法：
/// 自增主键落库后才拿得到，分两步写会留下没有图片的中间态）。
#[spacetimedb::reducer]
pub fn create_transformer_asset(
    ctx: &ReducerContext,
    input: TransformerAssetInput,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/transformer")?;
    require_park_access(ctx, input.park_id)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_asset(ctx, 0, customer_id.clone(), input)?;
    let asset_id = ctx.db.transformer_asset().insert(row).asset_id;
    replace_asset_images(ctx, asset_id, customer_id, Vec::new(), uploads)
}

#[spacetimedb::reducer]
pub fn update_transformer_asset(
    ctx: &ReducerContext,
    id: u64,
    input: TransformerAssetInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/transformer")?;
    let existing = require_transformer_asset(ctx, id)?;
    require_park_access(ctx, input.park_id)?;
    let mut row = validated_asset(ctx, id, existing.customer_id.clone(), input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.transformer_asset().asset_id().update(row);
    replace_asset_images(ctx, id, existing.customer_id, existing_image_ids, uploads)
}

/// 注销资产（软删除）。巡检记录随资产一起从视图消失，但数据保留。
#[spacetimedb::reducer]
pub fn delete_transformer_asset(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/transformer")?;
    let mut existing = require_transformer_asset(ctx, id)?;
    // 园区已归档时放行范围检查，否则归档园区名下的设备永远清不掉。
    require_park_access_unless_archived(ctx, existing.park_id)?;
    existing.is_deleted = true;
    existing.updated_at = Some(ctx.timestamp);
    ctx.db.transformer_asset().asset_id().update(existing);
    Ok(())
}

#[spacetimedb::reducer]
pub fn create_transformer_inspection(
    ctx: &ReducerContext,
    input: TransformerInspectionInput,
    uploads: Vec<UploadedMaintenanceImageInput>,
) -> Result<(), String> {
    let inspector_user_id = require_inspector(ctx)?;
    let inspector_name = inspector_name_snapshot(ctx, inspector_user_id);
    let asset = require_transformer_asset(ctx, input.asset_id)?;
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
        .transformer_inspection()
        .insert(TransformerInspection {
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
            .transformer_inspection_image()
            .insert(TransformerInspectionImage {
                id: 0,
                inspection_id: row.inspection_id,
                img_id,
                created_at: ctx.timestamp,
                updated_at: None,
            });
    }
    Ok(())
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
        .transformer_asset_image()
        .transformer_asset_image_by_owner()
        .filter(asset_id)
        .collect::<Vec<_>>();
    for link in &old_links {
        ctx.db.transformer_asset_image().id().delete(link.id);
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
            .transformer_asset_image()
            .insert(TransformerAssetImage {
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

fn validated_asset(
    ctx: &ReducerContext,
    asset_id: u64,
    customer_id: String,
    input: TransformerAssetInput,
) -> Result<TransformerAsset, String> {
    if input.park_id == 0 {
        return Err("请选择所属园区".into());
    }
    validate_location(
        ctx,
        input.park_id,
        (input.factory_id != 0).then_some(input.factory_id),
    )?;
    let transformer_name = required_text(input.transformer_name, "设备名称不能为空")?;
    let location = required_text(input.location, "位置描述不能为空")?;
    let specifications = required_text(input.specifications, "变压器规格不能为空")?;
    validate_max_length(&transformer_name, 100, "设备名称不能超过100个字符")?;
    validate_max_length(&location, 200, "位置描述不能超过200个字符")?;
    validate_max_length(&specifications, 50, "变压器规格不能超过50个字符")?;
    Ok(TransformerAsset {
        asset_id,
        customer_id,
        park_id: input.park_id,
        factory_id: input.factory_id,
        transformer_name,
        location,
        specifications,
        capacity_centi_kw: input.capacity_centi_kw,
        commissioned_on: limited_optional(input.commissioned_on, 20, "投运日期格式不正确")?,
        remark: limited_optional(input.remark, 200, "备注不能超过200个字符")?,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}

/// 校验巡检运行状态取值。
#[pure_function::pure]
pub(crate) fn check_inspection_status(status: &str) -> Result<(), String> {
    if matches!(status, "正常" | "异常") {
        Ok(())
    } else {
        Err("运行状态只能是「正常」或「异常」".into())
    }
}

/// 状态为异常时必须填写异常描述——异常却不说明哪里异常的记录没有意义。
#[pure_function::pure]
pub(crate) fn check_abnormal_note(status: &str, note: Option<&str>) -> Result<(), String> {
    if status == "异常" && note.is_none_or(|text| text.trim().is_empty()) {
        return Err("状态为异常时必须填写异常描述".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 运行状态只接受正常或异常() {
        assert!(check_inspection_status("正常").is_ok());
        assert!(check_inspection_status("异常").is_ok());
        assert!(check_inspection_status("待检").is_err());
        assert!(check_inspection_status("").is_err());
    }

    #[test]
    fn 异常状态必须填写异常描述() {
        assert!(check_abnormal_note("异常", None).is_err());
        assert!(check_abnormal_note("异常", Some("  ")).is_err());
        assert!(check_abnormal_note("异常", Some("油温偏高")).is_ok());
    }

    #[test]
    fn 正常状态不要求异常描述() {
        assert!(check_abnormal_note("正常", None).is_ok());
        assert!(check_abnormal_note("正常", Some("例行检查")).is_ok());
    }
}
