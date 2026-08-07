//! 请假申请的提交、修改、审批与删除。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::{current_enabled_user, require_hr_manager};
use crate::{
    reducers::{
        access::current_customer_id,
        park_ref::required_park_ref,
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

#[derive(SpacetimeType)]
pub struct LeaveApplicationInput {
    pub start_date: Timestamp,
    pub end_date: Timestamp,
    pub leave_type: String,
    pub reason: String,
    pub park_id: u64,
}

#[spacetimedb::reducer]
pub fn create_leave_application(
    ctx: &ReducerContext,
    input: LeaveApplicationInput,
) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    let park = required_park_ref(ctx, input.park_id)?;
    let (leave_type, reason) = validate_input(&input)?;
    ctx.db.leave_application().insert(LeaveApplication {
        id: 0,
        customer_id: user.customer_id,
        start_date: input.start_date,
        end_date: input.end_date,
        leave_type,
        reason,
        status: 0,
        reply: None,
        username: Some(user.real_name.clone()),
        park_name: Some(park.park_name),
        applicant_name: Some(user.real_name),
        audit_user_name: None,
        user_id: Some(user.id),
        park_id: input.park_id,
        audit_user_id: None,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_leave_application(
    ctx: &ReducerContext,
    id: u64,
    input: LeaveApplicationInput,
) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    let mut row = require_leave(ctx, id)?;
    if row.user_id != Some(user.id) {
        return Err("只能修改自己的请假申请".into());
    }
    if row.status != 0 {
        return Err("已审批的请假申请不能修改".into());
    }
    let park = required_park_ref(ctx, input.park_id)?;
    let (leave_type, reason) = validate_input(&input)?;
    row.start_date = input.start_date;
    row.end_date = input.end_date;
    row.leave_type = leave_type;
    row.reason = reason;
    row.park_id = input.park_id;
    row.park_name = Some(park.park_name);
    row.updated_at = Some(ctx.timestamp);
    ctx.db.leave_application().id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn audit_leave_application(
    ctx: &ReducerContext,
    id: u64,
    status: i8,
    reply: Option<String>,
) -> Result<(), String> {
    require_hr_manager(ctx)?;
    if !matches!(status, 1 | 2) {
        return Err("审批状态无效".into());
    }
    let auditor = current_enabled_user(ctx)?;
    let mut row = require_leave(ctx, id)?;
    if row.status != 0 {
        return Err("该请假申请已审批，无法重复操作".into());
    }
    row.status = status;
    row.reply = normalize_optional_text(reply);
    row.audit_user_id = Some(auditor.id);
    row.audit_user_name = Some(auditor.real_name);
    row.updated_at = Some(ctx.timestamp);
    ctx.db.leave_application().id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_leave_application(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    let row = require_leave(ctx, id)?;
    let is_admin = require_hr_manager(ctx).is_ok();
    if row.user_id != Some(user.id) && !is_admin {
        return Err("无删除该请假申请权限".into());
    }
    if row.status != 0 && !is_admin {
        return Err("已审批的请假申请不能删除".into());
    }
    ctx.db.leave_application().id().delete(id);
    Ok(())
}

fn require_leave(ctx: &ReducerContext, id: u64) -> Result<LeaveApplication, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .leave_application()
        .id()
        .find(id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("请假申请不存在".into())
}

fn validate_input(input: &LeaveApplicationInput) -> Result<(String, String), String> {
    if input.end_date <= input.start_date {
        return Err("请假结束时间必须晚于开始时间".into());
    }
    let leave_type = required_text(input.leave_type.clone(), "请假类型不能为空")?;
    let reason = required_text(input.reason.clone(), "请假事由不能为空")?;
    validate_max_length(&leave_type, 191, "请假类型不能超过191个字符")?;
    validate_max_length(&reason, 191, "请假事由不能超过191个字符")?;
    Ok((leave_type, reason))
}
