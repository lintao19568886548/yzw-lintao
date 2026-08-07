//! 设施维保共用关系和图片校验。

use spacetimedb::{DbContext, ReducerContext, Table};

use crate::{
    access as core_access,
    reducers::{
        access::{current_customer_id, require_factory, require_park},
        validation::{normalize_optional_text, validate_max_length},
    },
    tables::*,
};

/// 单台设备与单次巡检最多保留的图片数量，与园区、合同等业务口径一致。
pub(super) const MAX_MAINTENANCE_IMAGES: usize = 8;

/// 巡检权限：系统管理员，或持有 `maintenance:inspect` 权限码的账号。
///
/// 变压器与电梯共用同一个权限码——它叫「设施巡检」，为整个维护域设计，
/// 巡检员一码通检。二维码只是入口不是授权，权限完全由登录身份决定
/// （docs/变压器台账与扫码巡检.md §3.1、§3.4）。
pub(super) fn require_inspector(ctx: &ReducerContext) -> Result<u64, String> {
    let scope = core_access::read_scope(ctx.db_read_only(), ctx.sender(), Some(ctx.timestamp))
        .ok_or("当前身份未绑定用户")?;
    (scope.is_unrestricted() || scope.has_code(core_access::CODE_MAINTENANCE_INSPECT))
        .then_some(scope.user_id)
        .ok_or_else(|| "需要巡检权限，请联系管理员开通".to_string())
}

/// 巡检人姓名快照：优先真实姓名，缺失回退登录名。
pub(super) fn inspector_name_snapshot(ctx: &ReducerContext, inspector_user_id: u64) -> String {
    ctx.db
        .system_user()
        .id()
        .find(inspector_user_id)
        .map(|user| {
            if user.real_name.trim().is_empty() {
                user.username
            } else {
                user.real_name
            }
        })
        .unwrap_or_else(|| format!("用户 #{inspector_user_id}"))
}

pub(super) fn validate_location(
    ctx: &ReducerContext,
    park_id: u64,
    factory_id: Option<u64>,
) -> Result<(), String> {
    require_park(ctx, park_id)?;
    if let Some(factory_id) = factory_id {
        let factory = require_factory(ctx, factory_id)?;
        if factory.park_id != park_id {
            return Err("厂房不属于所选园区".into());
        }
    }
    Ok(())
}

pub(super) fn limited_optional(
    value: Option<String>,
    max_chars: usize,
    message: &'static str,
) -> Result<Option<String>, String> {
    let value = normalize_optional_text(value);
    if let Some(text) = &value {
        validate_max_length(text, max_chars, message)?;
    }
    Ok(value)
}

pub(super) fn require_transformer_asset(
    ctx: &ReducerContext,
    id: u64,
) -> Result<TransformerAsset, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .transformer_asset()
        .asset_id()
        .find(id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("变压器不存在或已注销".into())
}


