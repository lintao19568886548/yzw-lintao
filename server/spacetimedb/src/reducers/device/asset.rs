//! 设备台账维护。
//!
//! 台账只记「哪里有什么设备」，不记实时状态——在线与否要向厂商平台实时取，
//! 落库会得到一份永远慢半拍的假状态（口径与水电表读数不落库一致）。
//!
//! 权限按设备分类分流：摄像头要「摄像头管理」菜单，门禁类要「门禁设备管理」
//! 菜单。分类由设备类型推导，不接受客户端传入。完整设计见 `docs/设备管理.md`。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{
            current_customer_id, require_factory, require_image, require_menu_path, require_park,
            require_park_access, require_park_access_unless_archived,
        },
        platform::media::image_reducer::{delete_image_if_unreferenced, ensure_image},
        validation::{limited_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 传感器：只读物理世界的状态。
pub(crate) const DEVICE_CLASS_SENSOR: &str = "sensor";
/// 执行终端：接受指令改变物理世界的状态。
pub(crate) const DEVICE_CLASS_ACTUATOR: &str = "actuator";

const DEVICE_TYPE_CAMERA: &str = "camera";
const DEVICE_TYPE_ACCESS_CONTROLLER: &str = "access_controller";
const DEVICE_TYPE_BARRIER: &str = "barrier";
const DEVICE_TYPE_TURNSTILE: &str = "turnstile";

/// 单台设备最多保留的现场照片数，与维护设备、园区档案同一口径。
const MAX_DEVICE_IMAGES: usize = 8;

/// 用户确认保存后已经上传到 R2 的设备图片。
#[derive(SpacetimeType)]
pub struct UploadedDeviceImageInput {
    pub img_url: String,
    pub hash: String,
}

#[derive(SpacetimeType)]
pub struct DeviceAssetInput {
    pub device_type: String,
    pub device_name: String,
    pub device_code: String,
    pub location: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub external_device_id: Option<String>,
    pub commissioned_on: Option<String>,
    pub remark: Option<String>,
    /// `0` 表示园区公共区域（大门、道路、围墙）。门禁与摄像头在这里为 0 是常态。
    pub factory_id: u64,
    pub park_id: u64,
}

#[spacetimedb::reducer]
pub fn create_device_asset(
    ctx: &ReducerContext,
    input: DeviceAssetInput,
    uploads: Vec<UploadedDeviceImageInput>,
) -> Result<(), String> {
    let device_class = check_device_type(&input.device_type)?;
    require_menu_path(ctx, menu_path_for_class(device_class))?;
    require_park_access(ctx, input.park_id)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_asset(ctx, 0, customer_id.clone(), input)?;
    let asset_id = ctx.db.device_asset().insert(row).asset_id;
    replace_asset_images(ctx, asset_id, customer_id, Vec::new(), uploads)
}

#[spacetimedb::reducer]
pub fn update_device_asset(
    ctx: &ReducerContext,
    id: u64,
    input: DeviceAssetInput,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedDeviceImageInput>,
) -> Result<(), String> {
    let existing = require_device_asset(ctx, id)?;
    let device_class = check_device_type(&input.device_type)?;
    // 改类型会连带换页面：两边的菜单权限都要有，否则「在摄像头页把一台设备改成
    // 道闸」就成了绕过门禁设备权限的后门。
    require_menu_path(ctx, menu_path_for_class(&existing.device_class))?;
    require_menu_path(ctx, menu_path_for_class(device_class))?;
    require_park_access(ctx, input.park_id)?;
    let mut row = validated_asset(ctx, id, existing.customer_id.clone(), input)?;
    row.created_at = existing.created_at;
    // 边缘设备绑定不在这张表单里，编辑时必须原样带回——`validated_asset` 造的是
    // 一行全新记录，不带回就等于「改一次名字，摄像头和边缘设备的对应关系没了」。
    row.gateway_id = existing.gateway_id;
    row.channel_no = existing.channel_no;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.device_asset().asset_id().update(row);
    replace_asset_images(ctx, id, existing.customer_id, existing_image_ids, uploads)
}

/// 注销设备（软删除）。
#[spacetimedb::reducer]
pub fn delete_device_asset(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    let mut existing = require_device_asset(ctx, id)?;
    require_menu_path(ctx, menu_path_for_class(&existing.device_class))?;
    // 园区已归档时放行范围检查，否则归档园区名下的设备永远清不掉。
    require_park_access_unless_archived(ctx, existing.park_id)?;
    existing.is_deleted = true;
    existing.updated_at = Some(ctx.timestamp);
    ctx.db.device_asset().asset_id().update(existing);
    Ok(())
}

fn require_device_asset(ctx: &ReducerContext, id: u64) -> Result<DeviceAsset, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .device_asset()
        .asset_id()
        .find(id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("设备不存在或已注销".into())
}

fn validated_asset(
    ctx: &ReducerContext,
    asset_id: u64,
    customer_id: String,
    input: DeviceAssetInput,
) -> Result<DeviceAsset, String> {
    if input.park_id == 0 {
        return Err("请选择所属园区".into());
    }
    require_park(ctx, input.park_id)?;
    if input.factory_id != 0 {
        let factory = require_factory(ctx, input.factory_id)?;
        if factory.park_id != input.park_id {
            return Err("厂房不属于所选园区".into());
        }
    }
    let device_class = check_device_type(&input.device_type)?.to_string();
    let device_name = required_text(input.device_name, "设备名称不能为空")?;
    let device_code = required_text(input.device_code, "设备编号不能为空")?;
    let location = required_text(input.location, "位置描述不能为空")?;
    validate_max_length(&device_name, 100, "设备名称不能超过100个字符")?;
    validate_max_length(&device_code, 50, "设备编号不能超过50个字符")?;
    validate_max_length(&location, 200, "位置描述不能超过200个字符")?;
    let external_device_id =
        limited_optional_text(input.external_device_id, 100, "平台设备号不能超过100个字符")?;
    require_unique_device_code(ctx, &customer_id, asset_id, input.park_id, &device_code)?;
    require_unique_external_device_id(ctx, &customer_id, asset_id, external_device_id.as_deref())?;
    Ok(DeviceAsset {
        asset_id,
        customer_id,
        park_id: input.park_id,
        factory_id: input.factory_id,
        device_class,
        device_type: input.device_type,
        device_name,
        device_code,
        location,
        vendor: limited_optional_text(input.vendor, 50, "厂商不能超过50个字符")?,
        model: limited_optional_text(input.model, 50, "型号不能超过50个字符")?,
        external_device_id,
        commissioned_on: limited_optional_text(input.commissioned_on, 20, "启用日期格式不正确")?,
        remark: limited_optional_text(input.remark, 200, "备注不能超过200个字符")?,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
        // 摄像头挂到哪台边缘设备的哪一路，由边缘计算设备页单独维护，不在设备表单里手填
        // ——通道号要和边缘设备实际扫描到的对上，手填必然漂移。
        gateway_id: 0,
        channel_no: 0,
    })
}

/// 设备编号在园区内唯一——现场贴的标签认的就是它，重号等于两台设备混成一台。
fn require_unique_device_code(
    ctx: &ReducerContext,
    customer_id: &str,
    asset_id: u64,
    park_id: u64,
    device_code: &str,
) -> Result<(), String> {
    let duplicated = ctx
        .db
        .device_asset()
        .device_asset_by_park()
        .filter(park_id)
        .any(|row| {
            !row.is_deleted
                && row.customer_id == customer_id
                && row.asset_id != asset_id
                && row.device_code == device_code
        });
    if duplicated {
        return Err(format!("园区内已存在设备编号「{device_code}」"));
    }
    Ok(())
}

/// 平台设备号在**整个租户内**唯一，不是园区内唯一。
///
/// 与水电表同一条理由：设备号是厂商侧的身份，同一台设备绑两行台账会让同一份
/// 状态被两处各认一次。范围只覆盖园区的话，跨园区重绑照样漏过。
fn require_unique_external_device_id(
    ctx: &ReducerContext,
    customer_id: &str,
    asset_id: u64,
    external_device_id: Option<&str>,
) -> Result<(), String> {
    let Some(device_id) = external_device_id else {
        return Ok(());
    };
    let bound = ctx
        .db
        .device_asset()
        .device_asset_by_customer()
        .filter(customer_id)
        .find(|row| {
            !row.is_deleted
                && row.asset_id != asset_id
                && row.external_device_id.as_deref() == Some(device_id)
        });
    if let Some(bound) = bound {
        return Err(format!(
            "平台设备号 {device_id} 已经绑定给设备「{}」，请先解绑",
            bound.device_name
        ));
    }
    Ok(())
}

fn replace_asset_images(
    ctx: &ReducerContext,
    asset_id: u64,
    customer_id: String,
    existing_image_ids: Vec<u64>,
    uploads: Vec<UploadedDeviceImageInput>,
) -> Result<(), String> {
    if existing_image_ids.len() + uploads.len() > MAX_DEVICE_IMAGES {
        return Err(format!("每台设备最多保留 {MAX_DEVICE_IMAGES} 张图片"));
    }
    let old_links = ctx
        .db
        .device_asset_image()
        .device_asset_image_by_owner()
        .filter(asset_id)
        .collect::<Vec<_>>();
    for link in &old_links {
        ctx.db.device_asset_image().id().delete(link.id);
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
        ctx.db.device_asset_image().insert(DeviceAssetImage {
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

/// 设备类型合法性校验，顺带给出它属于传感器还是执行终端。
///
/// 分类只有这一处定义。`DeviceAsset::device_class` 存的是这个函数的结果，输入
/// 结构里没有分类字段——「摄像头被标成执行终端」这种自相矛盾的行因此写不进去。
#[pure_function::pure]
pub(crate) fn check_device_type(device_type: &str) -> Result<&'static str, String> {
    match device_type {
        DEVICE_TYPE_CAMERA => Ok(DEVICE_CLASS_SENSOR),
        // 门禁一体机读卡也控锁，按「失效后果更严重的那一面」归执行终端：
        // 读卡失败只是识别不了，锁不动才是真有人堵在门口。
        DEVICE_TYPE_ACCESS_CONTROLLER | DEVICE_TYPE_BARRIER | DEVICE_TYPE_TURNSTILE => {
            Ok(DEVICE_CLASS_ACTUATOR)
        }
        _ => Err(format!("未知的设备类型「{device_type}」")),
    }
}

/// 该分类的设备归哪个页面管——权限就按这个页面要。
///
/// 未知分类回落到门禁设备页而不是放行：分类只可能来自 [`check_device_type`]，
/// 走到这里说明库里有脏数据，此时要的是更严的那把锁。
#[pure_function::pure]
pub(crate) fn menu_path_for_class(device_class: &str) -> &'static str {
    if device_class == DEVICE_CLASS_SENSOR {
        "/device/camera"
    } else {
        "/device/access"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 摄像头是传感器() {
        assert_eq!(check_device_type("camera"), Ok(DEVICE_CLASS_SENSOR));
    }

    #[test]
    fn 门禁道闸闸机都是执行终端() {
        for device_type in ["access_controller", "barrier", "turnstile"] {
            assert_eq!(
                check_device_type(device_type),
                Ok(DEVICE_CLASS_ACTUATOR),
                "{device_type} 应当是执行终端"
            );
        }
    }

    #[test]
    fn 未知类型被拒绝() {
        assert!(check_device_type("").is_err());
        assert!(check_device_type("电表").is_err());
        // 水电表不进这张表，即使把类型名传进来也不认。
        assert!(check_device_type("meter_electric").is_err());
    }

    #[test]
    fn 两类设备落在不同页面的权限上() {
        assert_eq!(menu_path_for_class(DEVICE_CLASS_SENSOR), "/device/camera");
        assert_eq!(menu_path_for_class(DEVICE_CLASS_ACTUATOR), "/device/access");
    }

    #[test]
    fn 脏分类按更严的那把锁处理() {
        assert_eq!(menu_path_for_class("说不清"), "/device/access");
    }
}
