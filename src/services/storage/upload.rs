//! 客户端图片读取、哈希计算与 R2 直传。

use dioxus::html::FileData;

use super::r2::presign_r2_image_upload;
use crate::services::credentials::load_saved_token;

#[derive(Clone, Debug, PartialEq)]
pub struct UploadedImage {
    pub name: String,
    pub public_url: String,
    pub sha256: String,
}

/// 在本地校验待上传文件；该步骤不会向 R2 发送任何数据。
pub fn validate_salary_image(file: &FileData) -> Result<(), String> {
    let size = file.size();
    if size == 0 || size > 10 * 1024 * 1024 {
        return Err("图片大小必须在 1 字节到 10MB 之间".into());
    }
    let content_type = file.content_type().unwrap_or_default();
    if !matches!(
        content_type.as_str(),
        "image/jpeg" | "image/jpg" | "image/png" | "image/webp"
    ) {
        return Err("仅支持 JPG、PNG 和 WebP 图片".into());
    }
    Ok(())
}

/// 合同与工资共用同一套内容寻址图片校验规则。
pub fn validate_business_image(file: &FileData) -> Result<(), String> {
    validate_salary_image(file)
}

/// 用户确认保存后，计算哈希、获取预签名地址并直接上传 R2。
pub async fn upload_salary_image(file: FileData) -> Result<UploadedImage, String> {
    validate_salary_image(&file)?;
    let size = file.size();
    let content_type = file.content_type().unwrap_or_default();
    let bytes = file
        .read_bytes()
        .await
        .map_err(|error| format!("读取图片失败：{error}"))?;
    let sha256 = calculate_sha256(bytes.as_ref());
    let token = load_saved_token().ok_or("登录凭证不存在，请重新登录")?;
    let signed = presign_r2_image_upload(token, sha256.clone(), content_type.clone(), size)
        .await
        .map_err(|error| format!("获取 R2 上传地址失败：{error}"))?;
    put_bytes(signed.put_url, bytes.to_vec(), content_type).await?;
    Ok(UploadedImage {
        name: file.name(),
        public_url: signed.public_url,
        sha256,
    })
}

/// 用户确认保存后上传合同图片；R2 对象按 SHA-256 去重。
pub async fn upload_business_image(file: FileData) -> Result<UploadedImage, String> {
    upload_salary_image(file).await
}

fn calculate_sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    // 在 WASM 内部直接计算，避免局域网 HTTP 页面无法使用 Web Crypto。
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::calculate_sha256;

    #[test]
    fn calculates_known_sha256() {
        assert_eq!(
            calculate_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

#[cfg(target_arch = "wasm32")]
async fn put_bytes(url: String, bytes: Vec<u8>, content_type: String) -> Result<(), String> {
    use gloo_net::http::Request;
    use js_sys::Uint8Array;

    let body = Uint8Array::from(bytes.as_slice());
    let response = Request::put(&url)
        .header("Content-Type", &content_type)
        .body(wasm_bindgen::JsValue::from(body))
        .map_err(|error| format!("构建 R2 请求失败：{error}"))?
        .send()
        .await
        .map_err(|error| format!("上传 R2 失败：{error}"))?;
    response
        .ok()
        .then_some(())
        .ok_or_else(|| format!("R2 返回状态码 {}", response.status()))
}

#[cfg(not(target_arch = "wasm32"))]
async fn put_bytes(url: String, bytes: Vec<u8>, content_type: String) -> Result<(), String> {
    let response = reqwest::Client::new()
        .put(url)
        .header(reqwest::header::CONTENT_TYPE, content_type)
        .body(bytes)
        .send()
        .await
        .map_err(|error| format!("上传 R2 失败：{error}"))?;
    response
        .status()
        .is_success()
        .then_some(())
        .ok_or_else(|| format!("R2 返回状态码 {}", response.status()))
}
