//! 智能水电表管理服务端适配器共享的鉴权、配置与参数校验。

use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::Client;
use sha2::{Digest, Sha256};

static VALID_TOKEN_CACHE: OnceLock<Mutex<BTreeMap<String, u64>>> = OnceLock::new();

/// 在访问第三方抄表平台前，先确认当前工作台登录凭证仍然有效。
pub(super) async fn validate_workspace_token(token: &str) -> Result<(), String> {
    if token.trim().is_empty() {
        return Err("登录凭证不存在，请重新登录".into());
    }
    let cache_key = format!("{:x}", Sha256::digest(token.as_bytes()));
    let now = unix_seconds();
    if VALID_TOKEN_CACHE
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .ok()
        .and_then(|cache| cache.get(&cache_key).copied())
        .is_some_and(|expires_at| expires_at > now)
    {
        return Ok(());
    }
    let server = std::env::var("YIZU_SPACETIMEDB_SERVER_URL")
        .unwrap_or_else(|_| "https://yz.furong.org".into());
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(6))
        .build()
        .map_err(|error| format!("创建登录校验客户端失败：{error}"))?;
    let response = client
        .post(format!(
            "{}/v1/identity/websocket-token",
            server.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| format!("无法校验登录凭证：{error}"))?;
    if response.status().is_success() {
        if let Ok(mut cache) = VALID_TOKEN_CACHE
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
        {
            cache.retain(|_, expires_at| *expires_at > now);
            cache.insert(cache_key, now.saturating_add(60));
        }
        Ok(())
    } else {
        Err("登录凭证已失效，请重新登录".into())
    }
}

/// 第三方平台均按自然日查询，统一限制为 YYYY-MM-DD。
pub(super) fn validate_date(date: &str) -> Result<(), String> {
    let bytes = date.as_bytes();
    if bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
    {
        Ok(())
    } else {
        Err("冻结日期格式必须为 YYYY-MM-DD".into())
    }
}

pub(super) fn env_or(name: &str, fallback: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.into())
}

pub(super) fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 冻结日期只接受标准格式() {
        assert!(validate_date("2026-07-13").is_ok());
        assert!(validate_date("2026/07/13").is_err());
        assert!(validate_date("2026-7-13").is_err());
    }
}
