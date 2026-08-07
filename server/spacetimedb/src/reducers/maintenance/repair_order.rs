//! 报修工单创建、编辑、删除和状态机。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use super::common::{limited_optional, validate_location};
use crate::{
    reducers::{
        access::{current_customer_id, require_menu_path},
        validation::{required_text, validate_max_length},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct RepairOrderInput {
    pub source: String,
    pub tenant_name: Option<String>,
    pub tenant_phone: Option<String>,
    pub repair_type: String,
    pub description: String,
    pub priority: String,
    pub assignee: Option<String>,
    pub assignee_phone: Option<String>,
    pub factory_id: Option<u64>,
    pub park_id: u64,
}

#[spacetimedb::reducer]
pub fn create_repair_order(ctx: &ReducerContext, input: RepairOrderInput) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/repair-order")?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let order_no = format!("WX-{}", ctx.timestamp.to_micros_since_unix_epoch());
    let row = validated_row(ctx, 0, order_no, customer_id, input)?;
    ctx.db.repair_order().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_repair_order(
    ctx: &ReducerContext,
    id: u64,
    input: RepairOrderInput,
) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/repair-order")?;
    let existing = require_record(ctx, id)?;
    let mut row = validated_row(
        ctx,
        id,
        existing.order_no.clone(),
        existing.customer_id.clone(),
        input,
    )?;
    row.status = existing.status;
    row.process_remark = existing.process_remark;
    row.accept_time = existing.accept_time;
    row.finish_time = existing.finish_time;
    row.confirm_time = existing.confirm_time;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.repair_order().repair_order_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn transition_repair_order(
    ctx: &ReducerContext,
    id: u64,
    action: String,
    remark: Option<String>,
) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/repair-order")?;
    let mut row = require_record(ctx, id)?;
    let action = required_text(action, "工单操作不能为空")?;
    let next_status = match (row.status.as_str(), action.as_str()) {
        ("待接单", "accept") => {
            row.accept_time = Some(ctx.timestamp);
            "处理中"
        }
        ("待接单" | "处理中", "cancel") => "已取消",
        ("处理中", "finish") => {
            row.finish_time = Some(ctx.timestamp);
            "待验收"
        }
        ("待验收", "verify") => {
            row.confirm_time = Some(ctx.timestamp);
            "已完成"
        }
        ("待验收", "return") => "处理中",
        _ => return Err("当前工单状态不允许执行该操作".into()),
    };
    if matches!(action.as_str(), "cancel" | "finish" | "return")
        && remark
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err("请填写本次处理说明".into());
    }
    row.status = next_status.into();
    row.process_remark = limited_optional(remark, 500, "处理说明不能超过500个字符")?;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.repair_order().repair_order_id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_repair_order(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    require_menu_path(ctx, "/maintenance/repair-order")?;
    require_record(ctx, id)?;
    ctx.db.repair_order().repair_order_id().delete(id);
    Ok(())
}

fn require_record(ctx: &ReducerContext, id: u64) -> Result<RepairOrder, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .repair_order()
        .repair_order_id()
        .find(id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("报修工单不存在".into())
}

fn validated_row(
    ctx: &ReducerContext,
    repair_order_id: u64,
    order_no: String,
    customer_id: String,
    input: RepairOrderInput,
) -> Result<RepairOrder, String> {
    validate_location(ctx, input.park_id, input.factory_id)?;
    let source = required_text(input.source, "报修来源不能为空")?;
    let repair_type = required_text(input.repair_type, "报修类型不能为空")?;
    let description = required_text(input.description, "问题描述不能为空")?;
    let priority = required_text(input.priority, "优先级不能为空")?;
    validate_max_length(&source, 20, "报修来源不能超过20个字符")?;
    validate_max_length(&repair_type, 30, "报修类型不能超过30个字符")?;
    validate_max_length(&description, 500, "问题描述不能超过500个字符")?;
    validate_max_length(&priority, 20, "优先级不能超过20个字符")?;
    Ok(RepairOrder {
        repair_order_id,
        order_no,
        customer_id,
        source,
        tenant_name: limited_optional(input.tenant_name, 100, "租户名称不能超过100个字符")?,
        tenant_phone: limited_optional(input.tenant_phone, 30, "联系电话不能超过30个字符")?,
        repair_type,
        description,
        status: "待接单".into(),
        priority,
        assignee: limited_optional(input.assignee, 50, "维修人员不能超过50个字符")?,
        assignee_phone: limited_optional(input.assignee_phone, 30, "维修人员电话不能超过30个字符")?,
        process_remark: None,
        accept_time: None,
        finish_time: None,
        confirm_time: None,
        factory_id: input.factory_id,
        park_id: input.park_id,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}
