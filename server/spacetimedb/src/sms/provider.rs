//! 联麓短信模板发送协议。

use std::collections::BTreeMap;

use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use serde::Deserialize;
use serde_json::json;
use sha2::Sha256;
use spacetimedb::{ProcedureContext, http::Request};

use crate::tables::SmsProviderConfig;

#[derive(Deserialize)]
struct SmsResponse {
    status: Option<String>,
    message: Option<String>,
}

pub(crate) fn send_verification_code(
    ctx: &mut ProcedureContext,
    config: &SmsProviderConfig,
    phone_number: &str,
    code: &str,
) -> Result<(), String> {
    let timestamp = (ctx.timestamp.to_micros_since_unix_epoch() / 1_000).to_string();
    let signature = generate_signature(config, &timestamp)?;
    let payload = json!({
        "AppId": config.app_id,
        "MchId": config.merchant_id,
        "Version": config.version,
        "Type": config.message_type,
        "PhoneNumberSet": [phone_number],
        "TemplateId": config.template_id,
        "TemplateParamSet": [code],
        "TimeStamp": timestamp,
        "SignType": config.sign_type,
        "Signature": signature,
    });
    let request = Request::builder()
        .uri(&config.api_host)
        .method("POST")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json;charset=utf-8")
        .body(payload.to_string())
        .map_err(|error| format!("创建联麓短信请求失败：{error}"))?;
    let response = ctx
        .http
        .send(request)
        .map_err(|error| format!("联麓短信连接失败：{error}"))?;
    let status = response.status();
    let body = response.into_body().into_string_lossy();
    let result: SmsResponse =
        serde_json::from_str(&body).map_err(|_| "联麓短信响应格式无效".to_string())?;
    if !status.is_success() || result.status.as_deref() != Some("00") {
        return Err(format!(
            "联麓短信拒绝请求：HTTP {}，状态 {}，消息 {}",
            status.as_u16(),
            result.status.as_deref().unwrap_or("UNKNOWN"),
            result.message.as_deref().unwrap_or("")
        ));
    }
    Ok(())
}

/// 发送业务模板短信，并返回可写入审计日志的供应商响应。
pub(crate) fn send_template_message(
    ctx: &mut ProcedureContext,
    config: &SmsProviderConfig,
    phone_number: &str,
    template_params: &[String],
) -> Result<String, String> {
    let timestamp = (ctx.timestamp.to_micros_since_unix_epoch() / 1_000).to_string();
    let signature = generate_signature(config, &timestamp)?;
    let payload = json!({
        "AppId": config.app_id,
        "MchId": config.merchant_id,
        "Version": config.version,
        "Type": config.message_type,
        "PhoneNumberSet": [phone_number],
        "TemplateId": config.template_id,
        "TemplateParamSet": template_params,
        "TimeStamp": timestamp,
        "SignType": config.sign_type,
        "Signature": signature,
    });
    let request = Request::builder()
        .uri(&config.api_host)
        .method("POST")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json;charset=utf-8")
        .body(payload.to_string())
        .map_err(|error| format!("创建联麓短信请求失败：{error}"))?;
    let response = ctx
        .http
        .send(request)
        .map_err(|error| format!("联麓短信连接失败：{error}"))?;
    let status = response.status();
    let body = response.into_body().into_string_lossy();
    let result: SmsResponse =
        serde_json::from_str(&body).map_err(|_| "联麓短信响应格式无效".to_string())?;
    if !status.is_success() || result.status.as_deref() != Some("00") {
        return Err(format!(
            "联麓短信拒绝请求：HTTP {}，状态 {}，消息 {}",
            status.as_u16(),
            result.status.as_deref().unwrap_or("UNKNOWN"),
            result.message.as_deref().unwrap_or("")
        ));
    }
    Ok(body)
}

/// 按原合同系统的 personal/send 协议发送带自定义正文的业务短信。
pub(crate) fn send_personal_message(
    ctx: &mut ProcedureContext,
    config: &SmsProviderConfig,
    phone_number: &str,
    session_context: &str,
    context_params: &[String],
) -> Result<String, String> {
    let timestamp = (ctx.timestamp.to_micros_since_unix_epoch() / 1_000).to_string();
    let sign_name = "【东莞市宜租网络科技有限公司】";
    let signature = generate_personal_signature(config, &timestamp, sign_name)?;
    let mut params = vec![phone_number.to_string()];
    params.extend_from_slice(context_params);
    let payload = json!({
        "MchId": config.merchant_id,
        "AppId": config.app_id,
        "Version": config.version,
        "Type": config.message_type,
        "SignName": sign_name,
        "SessionContextSet": [session_context],
        "ContextParamSet": [params],
        "TimeStamp": timestamp,
        "SignType": config.sign_type,
        "Signature": signature,
    });
    let request = Request::builder()
        .uri("https://apis.shlianlu.com/sms/trade/personal/send")
        .method("POST")
        .header("Accept", "application/json")
        .header("Content-Type", "application/json;charset=utf-8")
        .body(payload.to_string())
        .map_err(|error| format!("创建联麓合同短信请求失败：{error}"))?;
    let response = ctx
        .http
        .send(request)
        .map_err(|error| format!("联麓合同短信连接失败：{error}"))?;
    let status = response.status();
    let body = response.into_body().into_string_lossy();
    let result: SmsResponse =
        serde_json::from_str(&body).map_err(|_| "联麓合同短信响应格式无效".to_string())?;
    if !status.is_success() || result.status.as_deref() != Some("00") {
        return Err(format!(
            "联麓合同短信拒绝请求：HTTP {}，状态 {}，消息 {}",
            status.as_u16(),
            result.status.as_deref().unwrap_or("UNKNOWN"),
            result.message.as_deref().unwrap_or("")
        ));
    }
    Ok(body)
}

fn generate_signature(config: &SmsProviderConfig, timestamp: &str) -> Result<String, String> {
    let mut fields = BTreeMap::new();
    fields.insert("AppId", config.app_id.as_str());
    fields.insert("MchId", config.merchant_id.as_str());
    fields.insert("SignType", config.sign_type.as_str());
    fields.insert("TemplateId", config.template_id.as_str());
    fields.insert("TimeStamp", timestamp);
    fields.insert("Type", config.message_type.as_str());
    fields.insert("Version", config.version.as_str());
    let parameters = fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    let raw = format!("{parameters}&key={}", config.secret_key);
    let bytes = match config.sign_type.as_str() {
        "MD5" => Md5::digest(raw.as_bytes()).to_vec(),
        "HMACSHA256" => {
            let mut signer = Hmac::<Sha256>::new_from_slice(config.secret_key.as_bytes())
                .map_err(|_| "短信签名密钥无效".to_string())?;
            signer.update(raw.as_bytes());
            signer.finalize().into_bytes().to_vec()
        }
        _ => return Err("短信签名算法只支持 MD5 或 HMACSHA256".into()),
    };
    Ok(bytes.iter().map(|byte| format!("{byte:02X}")).collect())
}

fn generate_personal_signature(
    config: &SmsProviderConfig,
    timestamp: &str,
    sign_name: &str,
) -> Result<String, String> {
    let mut fields = BTreeMap::new();
    fields.insert("AppId", config.app_id.as_str());
    fields.insert("MchId", config.merchant_id.as_str());
    fields.insert("SignName", sign_name);
    fields.insert("SignType", config.sign_type.as_str());
    fields.insert("TimeStamp", timestamp);
    fields.insert("Type", config.message_type.as_str());
    fields.insert("Version", config.version.as_str());
    let parameters = fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    let raw = format!("{parameters}&key={}", config.secret_key);
    let bytes = match config.sign_type.as_str() {
        "MD5" => Md5::digest(raw.as_bytes()).to_vec(),
        "HMACSHA256" => {
            let mut signer = Hmac::<Sha256>::new_from_slice(config.secret_key.as_bytes())
                .map_err(|_| "短信签名密钥无效".to_string())?;
            signer.update(raw.as_bytes());
            signer.finalize().into_bytes().to_vec()
        }
        _ => return Err("短信签名算法只支持 MD5 或 HMACSHA256".into()),
    };
    Ok(bytes.iter().map(|byte| format!("{byte:02X}")).collect())
}
