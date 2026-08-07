//! 设施巡检权限码的自愈式种入。
//!
//! `maintenance:inspect` 由管理员在权限管理页显式授予（角色或个人直授）。这里
//! 只负责保证权限码本身存在于 `code` 表，让授权界面有东西可勾——与 `hr_manage`
//! 的自愈迁移同一手法：`client_connected` 为每个租户补齐一次，幂等且廉价。
//! 权限语义见 `docs/变压器台账与扫码巡检.md` §3.4。

use spacetimedb::{ReducerContext, Table};

use crate::{access::CODE_MAINTENANCE_INSPECT, tables::*};

/// 确保租户的设施巡检权限码已存在。幂等：已存在时只做一次索引查找。
pub(crate) fn ensure_maintenance_inspect_code(ctx: &ReducerContext, customer_id: &str) {
    let exists = ctx
        .db
        .code()
        .code_by_customer_value()
        .filter((customer_id, CODE_MAINTENANCE_INSPECT))
        .any(|row| row.template_deleted_at.is_none());
    if exists {
        return;
    }
    ctx.db.code().insert(Code {
        code_id: 0,
        customer_id: customer_id.to_string(),
        code: CODE_MAINTENANCE_INSPECT.to_string(),
        name: "设施巡检".to_string(),
        content: Some("可以扫码或在后台登记设施巡检记录（变压器等）".to_string()),
        menu_id: None,
        created_at: ctx.timestamp,
        updated_at: None,
        template_deleted_at: None,
        template_internal_only: false,
        template_key: None,
        template_managed: false,
        template_version: 1,
    });
    log::info!("租户 {customer_id} 已种入设施巡检权限码");
}
