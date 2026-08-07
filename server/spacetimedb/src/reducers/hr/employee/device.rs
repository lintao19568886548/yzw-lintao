//! 考勤设备绑定、换机与异常审计。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use super::common::current_enabled_user;
use crate::{
    reducers::{
        access::current_customer_id,
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 打卡设备信息。
#[derive(SpacetimeType)]
pub struct AttendanceDeviceInput {
    pub device_id: Option<String>,
    pub device_model: Option<String>,
    pub device_system: Option<String>,
    pub bind_current_device: bool,
    pub confirm_device_abnormal: bool,
}

pub(super) struct DeviceDecision {
    pub abnormal_type: Option<String>,
    pub bound_device_id: Option<String>,
    pub current_device_id: Option<String>,
    pub duplicate_user_ids: Option<String>,
    pub duplicate_user_names: Option<String>,
}

#[spacetimedb::reducer]
pub fn replace_attendance_device(
    ctx: &ReducerContext,
    device_id: String,
    device_model: Option<String>,
    device_system: Option<String>,
) -> Result<(), String> {
    let user = current_enabled_user(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let (device_id, device_model, device_system) =
        validated_device(device_id, device_model, device_system)?;
    replace_binding(
        ctx,
        customer_id,
        user.id,
        device_id,
        device_model,
        device_system,
    );
    Ok(())
}

pub(super) fn prepare_device(
    ctx: &ReducerContext,
    user: &SystemUser,
    input: AttendanceDeviceInput,
) -> Result<DeviceDecision, String> {
    let Some(raw_device_id) = input.device_id else {
        // 原项目允许旧版客户端不上传设备标识。
        return Ok(DeviceDecision {
            abnormal_type: None,
            bound_device_id: None,
            current_device_id: None,
            duplicate_user_ids: None,
            duplicate_user_names: None,
        });
    };
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let (device_id, device_model, device_system) =
        validated_device(raw_device_id, input.device_model, input.device_system)?;
    let binding = active_binding(ctx, user.id);
    let bound_device_id = binding.as_ref().map(|row| row.device_id.clone());
    let duplicate_users = ctx
        .db
        .attendance_device_binding()
        .attendance_device_binding_by_device()
        .filter(device_id.as_str())
        .filter(|row| row.customer_id == customer_id && row.user_id != user.id)
        .filter_map(|row| ctx.db.system_user().id().find(row.user_id))
        .collect::<Vec<_>>();
    let mut abnormal_types = Vec::new();
    if binding
        .as_ref()
        .is_some_and(|row| row.device_id != device_id)
    {
        abnormal_types.push("device_changed");
    }
    if !duplicate_users.is_empty() {
        abnormal_types.push("same_device_multi_account");
    }
    if binding.is_none() {
        if !input.bind_current_device {
            return Err("当前账号尚未绑定打卡设备，请确认绑定当前设备后继续打卡".into());
        }
        replace_binding(
            ctx,
            customer_id.clone(),
            user.id,
            device_id.clone(),
            device_model,
            device_system,
        );
    } else if !abnormal_types.is_empty() {
        if !input.confirm_device_abnormal {
            return Err("检测到考勤设备异常，请确认后继续打卡".into());
        }
        // 异常确认只允许本次打卡，不能隐式替换或更新原绑定设备。
    } else if let Some(mut row) = binding {
        row.device_model = device_model.or(row.device_model);
        row.device_system = device_system.or(row.device_system);
        ctx.db.attendance_device_binding().id().update(row);
    }
    let duplicate_user_ids = (!duplicate_users.is_empty()).then(|| {
        duplicate_users
            .iter()
            .map(|row| row.id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    });
    let duplicate_user_names = (!duplicate_users.is_empty()).then(|| {
        duplicate_users
            .iter()
            .map(|row| row.real_name.clone())
            .collect::<Vec<_>>()
            .join(",")
    });
    Ok(DeviceDecision {
        abnormal_type: (!abnormal_types.is_empty()).then(|| abnormal_types.join(",")),
        bound_device_id,
        current_device_id: Some(device_id),
        duplicate_user_ids,
        duplicate_user_names,
    })
}

pub(super) fn record_device_abnormal(
    ctx: &ReducerContext,
    user_id: u64,
    attendance_id: u64,
    action: &str,
    punch_time: Timestamp,
    decision: DeviceDecision,
) {
    let (Some(abnormal_type), Some(current_device_id)) =
        (decision.abnormal_type, decision.current_device_id)
    else {
        return;
    };
    let Some(customer_id) = current_customer_id(ctx) else {
        return;
    };
    ctx.db
        .attendance_device_abnormal_log()
        .insert(AttendanceDeviceAbnormalLog {
            id: 0,
            customer_id,
            user_id,
            attendance_id: Some(attendance_id),
            action: action.into(),
            abnormal_type,
            bound_device_id: decision.bound_device_id,
            current_device_id,
            duplicate_user_ids: decision.duplicate_user_ids,
            duplicate_user_names: decision.duplicate_user_names,
            punch_time: Some(punch_time),
            created_at: ctx.timestamp,
        });
}

fn active_binding(ctx: &ReducerContext, user_id: u64) -> Option<AttendanceDeviceBinding> {
    ctx.db
        .attendance_device_binding()
        .attendance_device_binding_by_user()
        .filter(user_id)
        .max_by_key(|row| row.id)
}

fn replace_binding(
    ctx: &ReducerContext,
    customer_id: String,
    user_id: u64,
    device_id: String,
    device_model: Option<String>,
    device_system: Option<String>,
) {
    let ids = ctx
        .db
        .attendance_device_binding()
        .attendance_device_binding_by_user()
        .filter(user_id)
        .map(|row| row.id)
        .collect::<Vec<_>>();
    for id in ids {
        ctx.db.attendance_device_binding().id().delete(id);
    }
    ctx.db
        .attendance_device_binding()
        .insert(AttendanceDeviceBinding {
            id: 0,
            customer_id,
            user_id,
            device_id,
            device_model,
            device_system,
            first_bind_time: ctx.timestamp,
        });
}

fn validated_device(
    device_id: String,
    device_model: Option<String>,
    device_system: Option<String>,
) -> Result<(String, Option<String>, Option<String>), String> {
    let device_id = required_text(device_id, "未获取到当前设备标识，请刷新页面后重试")?;
    validate_max_length(&device_id, 128, "设备标识不能超过128个字符")?;
    let device_model = normalize_optional_text(device_model);
    let device_system = normalize_optional_text(device_system);
    if let Some(value) = &device_model {
        validate_max_length(value, 191, "设备型号不能超过191个字符")?;
    }
    if let Some(value) = &device_system {
        validate_max_length(value, 120, "设备系统不能超过120个字符")?;
    }
    Ok((device_id, device_model, device_system))
}
