//! 工资凭证解除关系后的 Cloudflare R2 安全清理。

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

use super::super::credentials::load_saved_token;
#[cfg(feature = "server")]
use super::r2::{image_metadata_exists, r2_client, validate_admin_token};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoredR2Image {
    pub img_id: u64,
    pub public_url: String,
}

/// 数据库事务完成后清理已无任何业务引用的 R2 工资凭证。
pub async fn delete_salary_images_from_r2(images: Vec<StoredR2Image>) -> Result<u32, String> {
    if images.is_empty() {
        return Ok(0);
    }
    let token = load_saved_token().ok_or("登录凭证不存在，请重新登录")?;
    delete_r2_salary_images(token, images)
        .await
        .map_err(|error| format!("清理 R2 工资凭证失败：{error}"))
}

/// 合同与工资共用安全清理接口；只有数据库元数据已无引用时才删除 R2 对象。
pub async fn delete_business_images_from_r2(images: Vec<StoredR2Image>) -> Result<u32, String> {
    delete_salary_images_from_r2(images)
        .await
        .map_err(|error| error.replace("工资凭证", "业务图片"))
}

#[post("/api/storage/r2/delete-salary-images")]
async fn delete_r2_salary_images(
    token: String,
    images: Vec<StoredR2Image>,
) -> Result<u32, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use std::collections::BTreeSet;

        dotenvy::dotenv().ok();
        if images.len() > 50 {
            return Err(ServerFnError::new("单次最多清理 50 张工资凭证"));
        }
        let auth = validate_admin_token(&token).await?;
        let (client, bucket, public_base) = r2_client().await?;
        let mut deleted = 0u32;
        let mut processed = BTreeSet::new();
        for image in images {
            if !processed.insert(image.img_id)
                || image_metadata_exists(&auth, &token, image.img_id).await?
            {
                continue;
            }
            let Some(key) = salary_object_key(&public_base, &image.public_url) else {
                // MySQL 遗留的 /uploads 地址不属于 R2，无需调用删除接口。
                continue;
            };
            client
                .delete_object()
                .bucket(&bucket)
                .key(key)
                .send()
                .await
                .map_err(|error| ServerFnError::new(format!("删除 R2 对象失败：{error}")))?;
            deleted += 1;
        }
        Ok(deleted)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (token, images);
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[cfg(any(feature = "server", test))]
fn salary_object_key(public_base: &str, public_url: &str) -> Option<String> {
    let key = public_url
        .strip_prefix(public_base.trim_end_matches('/'))?
        .strip_prefix('/')?;
    let filename = key.strip_prefix("yizu/salary-images/")?;
    let (hash, extension) = filename.rsplit_once('.')?;
    (hash.len() == 64
        && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        && matches!(extension, "jpg" | "png" | "webp"))
    .then(|| key.to_string())
}

#[cfg(test)]
mod tests {
    use super::salary_object_key;

    const HASH: &str = "803b88bf05a35dff4ca0d0666b38953c19e3d4304c6617ca5c9c4ba2cb9b57fc";

    #[test]
    fn 只解析当前_r2_域名下的工资凭证路径() {
        let base = "https://r2.example.com";
        assert_eq!(
            salary_object_key(base, &format!("{base}/yizu/salary-images/{HASH}.jpg")),
            Some(format!("yizu/salary-images/{HASH}.jpg"))
        );
        assert_eq!(
            salary_object_key(
                "https://another.example.com",
                &format!("{base}/yizu/salary-images/{HASH}.jpg")
            ),
            None
        );
    }
}
