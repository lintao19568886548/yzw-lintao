//! 园区租赁主档的新增、修改与逻辑删除。

use spacetimedb::{ReducerContext, Table};

use super::ParkInput;
use crate::{
    reducers::{
        access::{current_customer_id, require_park, require_park_access, require_rental_manager},
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

#[spacetimedb::reducer]
pub fn create_park(ctx: &ReducerContext, input: ParkInput) -> Result<(), String> {
    require_rental_manager(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let row = validated_park(ctx, 0, customer_id, input)?;
    ctx.db.park().insert(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_park(ctx: &ReducerContext, park_id: u64, input: ParkInput) -> Result<(), String> {
    require_rental_manager(ctx)?;
    require_park_access(ctx, park_id)?;
    let existing = require_park(ctx, park_id)?;
    let mut row = validated_park(ctx, park_id, existing.customer_id, input)?;
    row.created_at = existing.created_at;
    row.updated_at = Some(ctx.timestamp);
    ctx.db.park().park_id().update(row);
    Ok(())
}

pub(super) fn validated_park(
    ctx: &ReducerContext,
    park_id: u64,
    customer_id: String,
    input: ParkInput,
) -> Result<Park, String> {
    let park_name = required_text(input.park_name, "园区名称不能为空")?;
    let address = required_text(input.address, "园区地址不能为空")?;
    validate_max_length(&park_name, 100, "园区名称不能超过100个字符")?;
    validate_max_length(&address, 200, "园区地址不能超过200个字符")?;
    let manager = limited_optional(input.manager, 50, "负责人不能超过50个字符")?;
    let contact = limited_optional(input.contact, 50, "联系方式不能超过50个字符")?;
    let description = limited_optional(input.description, 300, "园区说明不能超过300个字符")?;
    let status = limited_optional(input.status, 30, "园区状态不能超过30个字符")?
        .or_else(|| Some("1".into()));

    let duplicate = ctx
        .db
        .park()
        .park_by_customer()
        .filter(customer_id.as_str())
        .any(|row| row.park_id != park_id && !row.is_deleted && row.park_name == park_name);
    if duplicate {
        return Err("同名园区已经存在".into());
    }

    Ok(Park {
        park_id,
        customer_id,
        park_name,
        address,
        description,
        status,
        contact,
        manager,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    })
}

fn limited_optional(
    value: Option<String>,
    max: usize,
    message: &'static str,
) -> Result<Option<String>, String> {
    let value = normalize_optional_text(value);
    if let Some(value) = &value {
        validate_max_length(value, max, message)?;
    }
    Ok(value)
}
