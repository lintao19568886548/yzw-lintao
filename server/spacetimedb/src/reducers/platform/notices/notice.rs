//! 公告采集结果写入和有效性维护事务。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::{
        access::AdminContext,
        validation::{limited_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 公告采集器写入的标准化记录。
#[derive(SpacetimeType)]
pub struct NoticeInput {
    pub notice_id: String,
    pub title: Option<String>,
    pub date: Option<String>,
    pub notice_type: Option<String>,
    pub category: Option<String>,
    pub project_type: Option<String>,
    pub site_code: Option<String>,
    pub owner: Option<String>,
    pub platform: Option<String>,
    pub link: Option<String>,
    pub created_at: Option<Timestamp>,
}

#[spacetimedb::reducer]
pub fn upsert_notice(ctx: &ReducerContext, input: NoticeInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let notice_id = required_text(input.notice_id, "公告标识不能为空")?;
    validate_max_length(&notice_id, 64, "公告标识不能超过 64 个字符")?;
    let title = limited_optional_text(input.title, 512, "公告标题不能超过 512 个字符")?;
    let date = limited_optional_text(input.date, 32, "公告日期不能超过 32 个字符")?;
    let notice_type = limited_optional_text(input.notice_type, 64, "公告类型不能超过 64 个字符")?;
    let category = limited_optional_text(input.category, 64, "公告分类不能超过 64 个字符")?;
    let project_type =
        limited_optional_text(input.project_type, 128, "项目类型不能超过 128 个字符")?;
    let site_code = limited_optional_text(input.site_code, 32, "站点编码不能超过 32 个字符")?;
    let owner = limited_optional_text(input.owner, 256, "公告业主不能超过 256 个字符")?;
    let platform = limited_optional_text(input.platform, 256, "公告平台不能超过 256 个字符")?;
    let link = input
        .link
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string());
    if let Some(mut row) = ctx.db.notice().notice_id().find(&notice_id) {
        row.title = title;
        row.date = date;
        row.notice_type = notice_type;
        row.category = category;
        row.project_type = project_type;
        row.site_code = site_code;
        row.owner = owner;
        row.platform = platform;
        row.link = link;
        row.updated_at = ctx.timestamp;
        ctx.db.notice().notice_id().update(row);
    } else {
        ctx.db.notice().insert(Notice {
            notice_id,
            global_scope: 0,
            title,
            date,
            notice_type,
            category,
            project_type,
            site_code,
            owner,
            platform,
            link,
            is_valid: true,
            invalid_reason: None,
            last_checked_at: None,
            created_at: input.created_at.unwrap_or(ctx.timestamp),
            updated_at: ctx.timestamp,
        });
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn set_notice_validity(
    ctx: &ReducerContext,
    notice_id: String,
    is_valid: bool,
    invalid_reason: Option<String>,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut row = ctx
        .db
        .notice()
        .notice_id()
        .find(&notice_id)
        .ok_or("公告不存在")?;
    row.is_valid = is_valid;
    row.invalid_reason = if is_valid {
        None
    } else {
        limited_optional_text(invalid_reason, 255, "失效原因不能超过 255 个字符")?
    };
    row.last_checked_at = Some(ctx.timestamp);
    row.updated_at = ctx.timestamp;
    ctx.db.notice().notice_id().update(row);
    Ok(())
}
