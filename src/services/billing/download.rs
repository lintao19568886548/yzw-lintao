//! 浏览器下载服务端生成的二进制文件。

use super::types::GeneratedBillingFile;

#[cfg(target_arch = "wasm32")]
pub fn download_generated_file(file: GeneratedBillingFile) -> Result<(), String> {
    use wasm_bindgen::JsCast;

    let array = js_sys::Uint8Array::from(file.bytes.as_slice());
    let parts = js_sys::Array::new();
    parts.push(&array.buffer());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(&file.content_type);
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|_| "无法创建下载文件".to_string())?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "无法创建下载地址".to_string())?;
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or("浏览器文档不可用")?;
    let anchor = document
        .create_element("a")
        .map_err(|_| "无法创建下载按钮".to_string())?;
    anchor
        .set_attribute("href", &url)
        .map_err(|_| "无法设置下载地址".to_string())?;
    anchor
        .set_attribute("download", &file.file_name)
        .map_err(|_| "无法设置文件名".to_string())?;
    anchor
        .dyn_into::<web_sys::HtmlElement>()
        .map_err(|_| "无法启动文件下载".to_string())?
        .click();
    web_sys::Url::revoke_object_url(&url).map_err(|_| "下载完成，但临时地址清理失败".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn download_generated_file(_file: GeneratedBillingFile) -> Result<(), String> {
    Err("当前运行平台不支持浏览器下载".into())
}
