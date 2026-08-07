//! R2 预签名上传接口；密钥只在 Dioxus 服务端读取。

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[cfg(feature = "server")]
const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresignedImageUpload {
    pub put_url: String,
    pub public_url: String,
}

#[post("/api/storage/r2/presign")]
pub async fn presign_r2_image_upload(
    token: String,
    sha256: String,
    content_type: String,
    size: u64,
) -> Result<PresignedImageUpload, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use aws_sdk_s3::presigning::PresigningConfig;
        use std::time::Duration;

        dotenvy::dotenv().ok();
        validate_request(&token, &sha256, &content_type, size).await?;

        let (client, bucket, public_base) = r2_client().await?;
        let extension = extension_for_mime(&content_type)
            .ok_or_else(|| ServerFnError::new("仅支持 JPG、PNG 和 WebP 图片"))?;
        let key = format!("yizu/salary-images/{sha256}.{extension}");
        let expires = PresigningConfig::builder()
            .expires_in(Duration::from_secs(600))
            .build()
            .map_err(|error| ServerFnError::new(error.to_string()))?;
        let put_url = client
            .put_object()
            .bucket(bucket)
            .key(&key)
            .content_type(&content_type)
            .presigned(expires)
            .await
            .map_err(|error| ServerFnError::new(format!("生成 R2 上传地址失败：{error}")))?
            .uri()
            .to_string();

        Ok(PresignedImageUpload {
            put_url,
            public_url: format!("{}/{}", public_base.trim_end_matches('/'), key),
        })
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (token, sha256, content_type, size);
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[cfg(feature = "server")]
async fn validate_request(
    token: &str,
    sha256: &str,
    content_type: &str,
    size: u64,
) -> Result<(), ServerFnError> {
    if token.trim().is_empty() {
        return Err(ServerFnError::new("登录凭证不存在，请重新登录"));
    }
    if size == 0 || size > MAX_IMAGE_BYTES {
        return Err(ServerFnError::new("图片大小必须在 1 字节到 10MB 之间"));
    }
    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ServerFnError::new("图片哈希格式无效"));
    }
    if extension_for_mime(content_type).is_none() {
        return Err(ServerFnError::new("仅支持 JPG、PNG 和 WebP 图片"));
    }
    validate_admin_token(token).await.map(|_| ())
}

#[cfg(feature = "server")]
pub(super) struct AuthenticatedSpacetime {
    pub client: reqwest::Client,
    pub server: String,
    pub database: String,
}

#[cfg(feature = "server")]
pub(super) async fn validate_admin_token(
    token: &str,
) -> Result<AuthenticatedSpacetime, ServerFnError> {
    dotenvy::dotenv().ok();
    if token.trim().is_empty() {
        return Err(ServerFnError::new("登录凭证不存在，请重新登录"));
    }
    let server = std::env::var("YIZU_SPACETIMEDB_SERVER_URL")
        .unwrap_or_else(|_| "https://yz.furong.org".into());
    let client = reqwest::Client::new();
    let response = client
        .post(format!(
            "{}/v1/identity/websocket-token",
            server.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| ServerFnError::new(format!("无法校验登录凭证：{error}")))?;
    if !response.status().is_success() {
        return Err(ServerFnError::new("登录凭证已失效，请重新登录"));
    }
    let database =
        std::env::var("YIZU_SPACETIMEDB_DATABASE").unwrap_or_else(|_| "yizu-server-yz18m".into());
    let role_response = client
        .post(format!(
            "{}/v1/database/{database}/sql",
            server.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/sql")
        .body("SELECT * FROM my_roles WHERE name = 'Super'")
        .send()
        .await
        .map_err(|error| ServerFnError::new(format!("无法校验管理权限：{error}")))?;
    if !role_response.status().is_success() {
        return Err(ServerFnError::new("无法确认当前账号的管理权限"));
    }
    let roles = role_response
        .text()
        .await
        .map_err(|error| ServerFnError::new(format!("读取管理权限失败：{error}")))?;
    if !roles.contains("Super") || !roles.contains("system") {
        return Err(ServerFnError::new("当前账号没有系统管理权限"));
    }
    Ok(AuthenticatedSpacetime {
        client,
        server,
        database,
    })
}

#[cfg(feature = "server")]
pub(super) async fn image_metadata_exists(
    auth: &AuthenticatedSpacetime,
    token: &str,
    img_id: u64,
) -> Result<bool, ServerFnError> {
    let response = auth
        .client
        .post(format!(
            "{}/v1/database/{}/sql",
            auth.server.trim_end_matches('/'),
            auth.database
        ))
        .bearer_auth(token)
        .header(reqwest::header::CONTENT_TYPE, "application/sql")
        .body(format!("SELECT * FROM my_images WHERE img_id = {img_id}"))
        .send()
        .await
        .map_err(|error| ServerFnError::new(format!("无法检查图片引用状态：{error}")))?;
    if !response.status().is_success() {
        return Err(ServerFnError::new("无法检查图片引用状态"));
    }
    let payload = response
        .text()
        .await
        .map_err(|error| ServerFnError::new(format!("读取图片引用状态失败：{error}")))?;
    let payload: serde_json::Value = serde_json::from_str(&payload)
        .map_err(|error| ServerFnError::new(format!("解析图片引用状态失败：{error}")))?;
    Ok(payload
        .as_array()
        .and_then(|statements| statements.first())
        .and_then(|statement| statement.get("rows"))
        .and_then(serde_json::Value::as_array)
        .is_some_and(|rows| !rows.is_empty()))
}

#[cfg(feature = "server")]
pub(super) async fn r2_client() -> Result<(aws_sdk_s3::Client, String, String), ServerFnError> {
    use aws_config::Region;
    use aws_sdk_s3::config::Credentials;

    let access_key = required_env("R2_ACCESS_KEY_ID")?;
    let secret_key = required_env("R2_SECRET_ACCESS_KEY")?;
    let endpoint = required_env("R2_ENDPOINT_URL")?;
    let bucket = required_env("R2_BUCKET_NAME")?;
    let public_base = required_env("R2_PUBLIC_BASE_URL")?;
    let credentials = Credentials::new(access_key, secret_key, None, None, "yizu-r2");
    let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(Region::new("auto"))
        .endpoint_url(endpoint)
        .credentials_provider(credentials)
        .load()
        .await;
    Ok((aws_sdk_s3::Client::new(&config), bucket, public_base))
}

#[cfg(feature = "server")]
pub(super) fn required_env(name: &str) -> Result<String, ServerFnError> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ServerFnError::new(format!("服务端缺少环境变量 {name}")))
}

#[cfg(feature = "server")]
fn extension_for_mime(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}
