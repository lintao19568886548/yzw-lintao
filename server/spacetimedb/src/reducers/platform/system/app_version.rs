//! 客户端应用版本维护逻辑。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{AdminContext, current_customer_id},
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

const DEFAULT_VERSION: &str = "1.0.0";
const DEFAULT_ANDROID_URL: &str = "https://www.yizuw.cn/download/kvapp_v1.0.0.apk";

/// 应用版本的可编辑字段。
#[derive(SpacetimeType)]
pub struct AppVersionInput {
    pub version: String,
    pub url: Option<String>,
    pub android_url: String,
    pub ios_url: Option<String>,
    pub notes: Option<String>,
}

#[pure_function::pure]
fn normalize_input(input: AppVersionInput) -> Result<AppVersionInput, String> {
    let version = required_text(input.version, "版本号不能为空")?;
    let android_url = required_text(input.android_url, "安卓下载地址不能为空")?;
    validate_max_length(&version, 191, "版本号不能超过 191 个字符")?;
    validate_max_length(&android_url, 191, "安卓下载地址不能超过 191 个字符")?;
    let url = normalize_optional_text(input.url);
    let ios_url = normalize_optional_text(input.ios_url);
    let notes = normalize_optional_text(input.notes);
    if let Some(url) = url.as_deref() {
        validate_max_length(url, 191, "通用下载地址不能超过 191 个字符")?;
    }
    if let Some(url) = ios_url.as_deref() {
        validate_max_length(url, 191, "iOS 下载地址不能超过 191 个字符")?;
    }
    Ok(AppVersionInput {
        version,
        url,
        android_url,
        ios_url,
        notes,
    })
}

fn require_app_version(ctx: &ReducerContext, id: u64) -> Result<AppVersion, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .app_version()
        .id()
        .find(id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("应用版本不存在".into())
}

fn ensure_unique_version(
    ctx: &ReducerContext,
    customer_id: &str,
    version: &str,
    excluded_id: Option<u64>,
) -> Result<(), String> {
    let duplicate = ctx
        .db
        .app_version()
        .app_version_by_customer_version()
        .filter((customer_id, version))
        .any(|row| Some(row.id) != excluded_id);
    (!duplicate).then_some(()).ok_or("版本号已经存在".into())
}

pub(crate) fn insert_default_app_version(ctx: &ReducerContext, customer_id: String) {
    if ctx
        .db
        .app_version()
        .app_version_by_customer()
        .filter(customer_id.as_str())
        .next()
        .is_some()
    {
        return;
    }
    ctx.db.app_version().insert(AppVersion {
        id: 0,
        customer_id,
        version: DEFAULT_VERSION.into(),
        url: None,
        android_url: DEFAULT_ANDROID_URL.into(),
        ios_url: None,
        notes: Some("初始版本".into()),
        created_at: ctx.timestamp,
        updated_at: ctx.timestamp,
    });
}

#[spacetimedb::reducer]
pub fn create_app_version(ctx: &ReducerContext, input: AppVersionInput) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let input = normalize_input(input)?;
    ensure_unique_version(ctx, &customer_id, &input.version, None)?;
    ctx.db.app_version().insert(AppVersion {
        id: 0,
        customer_id,
        version: input.version,
        url: input.url,
        android_url: input.android_url,
        ios_url: input.ios_url,
        notes: input.notes,
        created_at: ctx.timestamp,
        updated_at: ctx.timestamp,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_app_version(
    ctx: &ReducerContext,
    id: u64,
    input: AppVersionInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut row = require_app_version(ctx, id)?;
    let input = normalize_input(input)?;
    ensure_unique_version(ctx, &row.customer_id, &input.version, Some(id))?;
    row.version = input.version;
    row.url = input.url;
    row.android_url = input.android_url;
    row.ios_url = input.ios_url;
    row.notes = input.notes;
    row.updated_at = ctx.timestamp;
    ctx.db.app_version().id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_app_version(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let row = require_app_version(ctx, id)?;
    ctx.db.app_version().id().delete(id);
    insert_default_app_version(ctx, row.customer_id);
    Ok(())
}
