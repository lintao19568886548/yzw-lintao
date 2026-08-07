//! Cloudflare R2 图片存储服务。

mod r2;
mod r2_cleanup;
mod upload;

pub use r2_cleanup::{delete_business_images_from_r2, delete_salary_images_from_r2, StoredR2Image};
pub use upload::{
    upload_business_image, upload_salary_image, validate_business_image, validate_salary_image,
};

#[cfg(feature = "server")]
pub(crate) async fn validate_admin_access(
    token: &str,
) -> Result<(), dioxus::prelude::ServerFnError> {
    r2::validate_admin_token(token).await.map(|_| ())
}
