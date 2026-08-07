//! SpacetimeDB 身份令牌的本地持久化。

const CREDENTIAL_KEY: &str = "yizu-app-spacetimedb-main";
const ACCOUNT_KEY: &str = "yizu-app-last-account";

/// 短于这个存活时长的令牌一律不落盘。
///
/// 长期令牌的 `exp` 是 null，一次性 WebSocket 令牌是 `iat + 60`，两者相差极大，
/// 阈值取一小时既能可靠区分，又给将来服务端改成「签发有限期的长期令牌」留出余地。
const MIN_DURABLE_LIFETIME_SECONDS: i64 = 60 * 60;

fn jwt_claims(token: &str) -> Option<serde_json::Value> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload.trim_end_matches('=')).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// 判断令牌是否值得长期保存。
///
/// 浏览器端无法给 WebSocket 握手设置请求头，spacetimedb-sdk 的 `fetch_ws_token`
/// 会先拿长期令牌去 `POST /v1/identity/websocket-token` 换一枚**只活 60 秒**的
/// 一次性令牌，再作为 query 参数建连。而服务端首帧 `IdentityToken` 回传的正是
/// 这枚短命令牌（已实测：字符串与换来的完全一致，`exp = iat + 60`）。
///
/// 若把它当成新的长期令牌写回本地，就会覆盖真正的长期令牌：60 秒后刷新页面，
/// 换取接口对着过期令牌返回 401，连接层清除令牌并匿名重连，identity 随之改变。
/// 服务端的业务会话按 identity 存（`user_session`），换了 identity 就认领不回
/// 原来那一行，于是「勾了保持 30 天，过一会儿刷新却要重新登录」。
///
/// 匿名建连时服务端签发的是 `exp` 为 null 的长期令牌，这条路必须放行，
/// 否则登录态永远存不下来。
pub(crate) fn is_durable_token(token: &str) -> bool {
    let Some(claims) = jwt_claims(token) else {
        // 解不开的令牌按老行为保存，至少不比修复前更差。
        return true;
    };
    let Some(expires_at) = claims.get("exp").and_then(serde_json::Value::as_i64) else {
        // 字段缺失或为 null，即不过期。
        return true;
    };
    claims
        .get("iat")
        .and_then(serde_json::Value::as_i64)
        .is_some_and(|issued_at| expires_at - issued_at >= MIN_DURABLE_LIFETIME_SECONDS)
}

#[cfg(target_arch = "wasm32")]
pub fn load_saved_token() -> Option<String> {
    use spacetimedb_sdk::credentials::{LocalStorage, Storage};

    LocalStorage::get::<String>(CREDENTIAL_KEY)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_saved_token() -> Option<String> {
    spacetimedb_sdk::credentials::File::new(CREDENTIAL_KEY)
        .load()
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn save_token(token: &str) {
    use spacetimedb_sdk::credentials::{LocalStorage, Storage};

    let _ = LocalStorage::set(CREDENTIAL_KEY, token);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn save_token(token: &str) {
    let _ = spacetimedb_sdk::credentials::File::new(CREDENTIAL_KEY).save(token);
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn clear_saved_token() {
    use spacetimedb_sdk::credentials::{LocalStorage, Storage};

    LocalStorage::delete(CREDENTIAL_KEY);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn clear_saved_token() {
    // 服务端凭据存储没有删除接口，写入空值后读取逻辑会将其视为无令牌。
    let _ = spacetimedb_sdk::credentials::File::new(CREDENTIAL_KEY).save("");
}

#[cfg(target_arch = "wasm32")]
pub fn load_saved_account() -> Option<String> {
    use spacetimedb_sdk::credentials::{LocalStorage, Storage};

    LocalStorage::get::<String>(ACCOUNT_KEY)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_saved_account() -> Option<String> {
    spacetimedb_sdk::credentials::File::new(ACCOUNT_KEY)
        .load()
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn save_account(account: Option<&str>) {
    use spacetimedb_sdk::credentials::{LocalStorage, Storage};

    match account {
        Some(account) => {
            let _ = LocalStorage::set(ACCOUNT_KEY, account);
        }
        None => LocalStorage::delete(ACCOUNT_KEY),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn save_account(account: Option<&str>) {
    let _ = spacetimedb_sdk::credentials::File::new(ACCOUNT_KEY).save(account.unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    /// 只有中段的 claims 参与判断，头部与签名用占位串即可。
    fn token_with(claims: &str) -> String {
        format!("header.{}.signature", URL_SAFE_NO_PAD.encode(claims))
    }

    #[test]
    fn 长期令牌可以保存() {
        // SpacetimeDB 实际签发的长期令牌就是 exp 为 null。
        assert!(is_durable_token(&token_with(
            r#"{"hex_identity":"c200","iat":1784858168,"exp":null}"#
        )));
        assert!(is_durable_token(&token_with(r#"{"iat":1784858168}"#)));
    }

    #[test]
    fn 一次性websocket令牌不保存() {
        // 换取接口返回的令牌固定是 iat + 60。
        assert!(!is_durable_token(&token_with(
            r#"{"iat":1784858168,"exp":1784858228}"#
        )));
    }

    #[test]
    fn 有限期但足够长的令牌仍可保存() {
        // 服务端若改成签发一天有效期的长期令牌，不应被这道闸门误伤。
        assert!(is_durable_token(&token_with(
            r#"{"iat":1784858168,"exp":1784944568}"#
        )));
    }

    #[test]
    fn 缺少签发时间的有限期令牌不保存() {
        // 算不出存活时长就无法证明它是长期令牌，从严处理。
        assert!(!is_durable_token(&token_with(r#"{"exp":1784858228}"#)));
    }

    #[test]
    fn 解不开的令牌按老行为保存() {
        assert!(is_durable_token("not-a-jwt"));
        assert!(is_durable_token("header.@@@invalid-base64@@@.signature"));
    }
}
