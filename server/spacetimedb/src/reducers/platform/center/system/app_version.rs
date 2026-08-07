//! 中心应用版本发布逻辑。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::AdminContext,
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

const DEFAULT_VERSION: &str = "1.0.0";
const DEFAULT_ANDROID_URL: &str = "https://www.yizuw.cn/download/kvapp_v1.0.0.apk";

#[derive(SpacetimeType)]
pub struct CenterAppVersionInput {
    pub version: String,
    pub url: Option<String>,
    pub android_url: String,
    pub ios_url: Option<String>,
    pub notes: Option<String>,
}

#[pure_function::pure]
fn normalize_input(input: CenterAppVersionInput) -> Result<CenterAppVersionInput, String> {
    let version = required_text(input.version, "版本号不能为空")?;
    let android_url = required_text(input.android_url, "安卓下载地址不能为空")?;
    validate_max_length(&version, 191, "版本号不能超过 191 个字符")?;
    validate_max_length(&android_url, 191, "安卓下载地址不能超过 191 个字符")?;
    let url = normalize_optional_text(input.url);
    let ios_url = normalize_optional_text(input.ios_url);
    if let Some(url) = url.as_deref() {
        validate_max_length(url, 191, "通用下载地址不能超过 191 个字符")?;
    }
    if let Some(url) = ios_url.as_deref() {
        validate_max_length(url, 191, "iOS 下载地址不能超过 191 个字符")?;
    }
    Ok(CenterAppVersionInput {
        version,
        url,
        android_url,
        ios_url,
        notes: normalize_optional_text(input.notes),
    })
}

fn require_version(ctx: &ReducerContext, id: u64) -> Result<CenterAppVersion, String> {
    ctx.db
        .center_app_version()
        .id()
        .find(id)
        .ok_or("中心应用版本不存在".into())
}

fn ensure_unique(
    ctx: &ReducerContext,
    version: &str,
    excluded_id: Option<u64>,
) -> Result<(), String> {
    let duplicate = ctx
        .db
        .center_app_version()
        .center_app_version_by_version()
        .filter(version)
        .any(|row| Some(row.id) != excluded_id);
    (!duplicate).then_some(()).ok_or("版本号已经存在".into())
}

pub(crate) fn insert_default_center_app_version(ctx: &ReducerContext) {
    if ctx.db.center_app_version().count() != 0 {
        return;
    }
    ctx.db.center_app_version().insert(CenterAppVersion {
        id: 0,
        center_scope: 0,
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
pub fn create_center_app_version(
    ctx: &ReducerContext,
    input: CenterAppVersionInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let input = normalize_input(input)?;
    ensure_unique(ctx, &input.version, None)?;
    ctx.db.center_app_version().insert(CenterAppVersion {
        id: 0,
        center_scope: 0,
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
pub fn update_center_app_version(
    ctx: &ReducerContext,
    id: u64,
    input: CenterAppVersionInput,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let mut row = require_version(ctx, id)?;
    let input = normalize_input(input)?;
    ensure_unique(ctx, &input.version, Some(id))?;
    row.version = input.version;
    row.url = input.url;
    row.android_url = input.android_url;
    row.ios_url = input.ios_url;
    row.notes = input.notes;
    row.updated_at = ctx.timestamp;
    ctx.db.center_app_version().id().update(row);
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_center_app_version(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_version(ctx, id)?;
    ctx.db.center_app_version().id().delete(id);
    insert_default_center_app_version(ctx);
    Ok(())
}
