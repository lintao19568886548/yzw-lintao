//! YMSINO 实时设备与日冻结数据适配器。

use std::{
    sync::{Mutex, OnceLock},
    time::Duration,
};

use reqwest::{Client, StatusCode};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

use super::{
    common::{env_or, unix_seconds, validate_date, validate_workspace_token},
    types::{
        MeterKind, SmartMeterCatalog, SmartMeterDevice, SmartMeterReading, SmartMeterReadingBatch,
        SmartMeterSnapshot,
    },
};

const DEFAULT_BASE_URL: &str = "http://pt.ymsino1.com/ymcb/inter";
const DEFAULT_USERNAME: &str = "yzwl";
const DEFAULT_PASSWORD: &str = "yzwl";
const DEFAULT_ORG_ID: &str = "1024";
const DEFAULT_PT_ID: &str = "YZWL";

#[derive(Clone)]
struct ProviderConfig {
    base_url: String,
    username: String,
    password: String,
    org_id: String,
    pt_id: String,
    timeout_seconds: u64,
}

#[derive(Clone)]
struct CachedToken {
    value: String,
    expires_at: u64,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    #[serde(rename = "Code", default)]
    code: Value,
    #[serde(rename = "Msg", default)]
    message: String,
    #[serde(rename = "Token", default)]
    token: String,
}

#[derive(Debug)]
struct ProviderResponse<T> {
    code: Value,
    message: String,
    data: Vec<T>,
}

#[derive(Clone, Debug, Deserialize)]
struct ProviderDevice {
    #[serde(rename = "RmId", default)]
    room_id: String,
    #[serde(rename = "RmName", default)]
    room_name: String,
    #[serde(rename = "DeviceId", default)]
    device_id: String,
    #[serde(rename = "FactoryNo", default)]
    factory_no: String,
    #[serde(rename = "Pt", default)]
    pt: String,
    #[serde(rename = "Ct", default)]
    ct: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ProviderReading {
    #[serde(rename = "RmId", default)]
    room_id: String,
    #[serde(rename = "RmName", default)]
    room_name: String,
    #[serde(rename = "DeviceId", default)]
    device_id: String,
    #[serde(rename = "FactoryNo", default)]
    factory_no: String,
    #[serde(rename = "TranDate", default)]
    freeze_time: String,
    #[serde(rename = "ZTotal", default)]
    total: String,
    #[serde(rename = "ZTip", default)]
    tip: String,
    #[serde(rename = "ZPeak", default)]
    peak: String,
    #[serde(rename = "ZComm", default)]
    flat: String,
    #[serde(rename = "ZVale", default)]
    valley: String,
}

static TOKEN_CACHE: OnceLock<Mutex<Option<CachedToken>>> = OnceLock::new();

pub async fn load_snapshot(
    spacetime_token: &str,
    kind: MeterKind,
    date: &str,
) -> Result<SmartMeterSnapshot, String> {
    validate_workspace_token(spacetime_token).await?;
    validate_date(date)?;
    let config = provider_config();
    let client = Client::builder()
        .timeout(Duration::from_secs(config.timeout_seconds))
        .build()
        .map_err(|error| format!("创建智能水电表客户端失败：{error}"))?;
    let meter_type = match kind {
        MeterKind::Electric => "0",
        MeterKind::Water => "1",
    };
    let devices = authenticated_request::<ProviderDevice>(
        &client,
        &config,
        "/GetInfo",
        json!({
            "OrgId": config.org_id,
            "PtId": config.pt_id,
            "TjType": meter_type,
        }),
    )
    .await?;
    let readings = authenticated_request::<ProviderReading>(
        &client,
        &config,
        "/GetTranDay",
        json!({
            "OrgId": config.org_id,
            "PtId": config.pt_id,
            "TyDate": date,
            "TjType": meter_type,
        }),
    )
    .await?;
    Ok(normalize_snapshot(kind, date, &config, devices, readings))
}

pub(super) async fn load_catalog(
    spacetime_token: &str,
    kind: MeterKind,
) -> Result<SmartMeterCatalog, String> {
    validate_workspace_token(spacetime_token).await?;
    let config = provider_config();
    let client = provider_client(&config)?;
    let meter_type = meter_type(kind);
    let devices = authenticated_request::<ProviderDevice>(
        &client,
        &config,
        "/GetInfo",
        json!({
            "OrgId": config.org_id,
            "PtId": config.pt_id,
            "TjType": meter_type,
        }),
    )
    .await?;
    let snapshot = normalize_snapshot(kind, "", &config, devices, Vec::new());
    Ok(SmartMeterCatalog {
        kind,
        provider_name: snapshot.provider_name,
        park_name: snapshot.park_name,
        protocol: snapshot.protocol,
        devices: snapshot.devices,
    })
}

pub(super) async fn load_readings(
    spacetime_token: &str,
    kind: MeterKind,
    date: &str,
) -> Result<SmartMeterReadingBatch, String> {
    validate_workspace_token(spacetime_token).await?;
    validate_date(date)?;
    let config = provider_config();
    let client = provider_client(&config)?;
    let readings = authenticated_request::<ProviderReading>(
        &client,
        &config,
        "/GetTranDay",
        json!({
            "OrgId": config.org_id,
            "PtId": config.pt_id,
            "TyDate": date,
            "TjType": meter_type(kind),
        }),
    )
    .await?;
    let snapshot = normalize_snapshot(kind, date, &config, Vec::new(), readings);
    Ok(SmartMeterReadingBatch {
        kind,
        requested_date: date.into(),
        readings: snapshot.readings,
    })
}

fn provider_client(config: &ProviderConfig) -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(config.timeout_seconds))
        .build()
        .map_err(|error| format!("创建智能水电表客户端失败：{error}"))
}

fn meter_type(kind: MeterKind) -> &'static str {
    match kind {
        MeterKind::Electric => "0",
        MeterKind::Water => "1",
    }
}

async fn authenticated_request<T: DeserializeOwned>(
    client: &Client,
    config: &ProviderConfig,
    path: &str,
    payload: Value,
) -> Result<Vec<T>, String> {
    let token = get_provider_token(client, config, false).await?;
    let first = send_provider_request::<T>(client, config, path, &payload, &token).await;
    match first {
        Ok(response) if !is_auth_failure(&response.code, &response.message) => {
            ensure_read_success(response)
        }
        Ok(_) | Err(ProviderRequestError::Unauthorized) => {
            invalidate_provider_token();
            let refreshed = get_provider_token(client, config, true).await?;
            let response = send_provider_request::<T>(client, config, path, &payload, &refreshed)
                .await
                .map_err(ProviderRequestError::into_message)?;
            ensure_read_success(response)
        }
        Err(error) => Err(error.into_message()),
    }
}

#[derive(Debug)]
enum ProviderRequestError {
    Unauthorized,
    Message(String),
}

impl ProviderRequestError {
    fn into_message(self) -> String {
        match self {
            Self::Unauthorized => "YMSINO 鉴权失败".into(),
            Self::Message(message) => message,
        }
    }
}

async fn send_provider_request<T: DeserializeOwned>(
    client: &Client,
    config: &ProviderConfig,
    path: &str,
    payload: &Value,
    token: &str,
) -> Result<ProviderResponse<T>, ProviderRequestError> {
    let mut authenticated_payload = payload.clone();
    if let Some(fields) = authenticated_payload.as_object_mut() {
        // YMSINO 当前实现要求 Token 位于 JSON 请求体；Header 同时保留用于兼容代理层。
        fields.insert("Token".into(), Value::String(token.to_string()));
    }
    let response = client
        .post(format!("{}{}", config.base_url.trim_end_matches('/'), path))
        .header("Token", token)
        .json(&authenticated_payload)
        .send()
        .await
        .map_err(|error| ProviderRequestError::Message(format!("YMSINO 连接失败：{error}")))?;
    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return Err(ProviderRequestError::Unauthorized);
    }
    if !response.status().is_success() {
        return Err(ProviderRequestError::Message(format!(
            "YMSINO 返回 HTTP {}",
            response.status()
        )));
    }
    let payload = response
        .json::<Value>()
        .await
        .map_err(|error| ProviderRequestError::Message(format!("解析 YMSINO 响应失败：{error}")))?;
    parse_provider_response(payload)
}

fn parse_provider_response<T: DeserializeOwned>(
    payload: Value,
) -> Result<ProviderResponse<T>, ProviderRequestError> {
    let data = match payload.get("Date").cloned() {
        Some(Value::Array(items)) => serde_json::from_value::<Vec<T>>(Value::Array(items))
            .map_err(|error| {
                ProviderRequestError::Message(format!("解析 YMSINO 数据列表失败：{error}"))
            })?,
        // 供应商在无数据时会返回空字符串，按空列表处理。
        _ => Vec::new(),
    };
    Ok(ProviderResponse {
        code: payload.get("Code").cloned().unwrap_or(Value::Null),
        message: payload
            .get("Msg")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        data,
    })
}

fn ensure_read_success<T>(response: ProviderResponse<T>) -> Result<Vec<T>, String> {
    let code = value_text(&response.code);
    let code_zero_is_success = code == "0"
        && (response.message.contains("成功")
            || response.message.contains("无数据")
            || response.message.trim().is_empty());
    if code.is_empty()
        || matches!(code.as_str(), "1" | "200" | "success" | "true")
        || code_zero_is_success
    {
        Ok(response.data)
    } else {
        Err(format!(
            "YMSINO 拒绝读取：{}{}",
            code,
            (!response.message.is_empty())
                .then(|| format!(" · {}", response.message))
                .unwrap_or_default()
        ))
    }
}

async fn get_provider_token(
    client: &Client,
    config: &ProviderConfig,
    force_refresh: bool,
) -> Result<String, String> {
    if !force_refresh {
        if let Some(token) = cached_provider_token() {
            return Ok(token);
        }
    }
    let response = client
        .post(format!(
            "{}/GetToken",
            config.base_url.trim_end_matches('/')
        ))
        .json(&json!({
            "UserName": config.username,
            "PassWord": config.password,
            "OrgId": config.org_id,
        }))
        .send()
        .await
        .map_err(|error| format!("YMSINO 登录失败：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("YMSINO 登录返回 HTTP {}", response.status()));
    }
    let payload = response
        .json::<LoginResponse>()
        .await
        .map_err(|error| format!("解析 YMSINO 登录响应失败：{error}"))?;
    if payload.token.trim().is_empty() {
        return Err(format!(
            "YMSINO 未签发 Token：{} {}",
            value_text(&payload.code),
            payload.message
        ));
    }
    let token = payload.token;
    let expires_at = unix_seconds() + 86_400;
    if let Ok(mut cache) = TOKEN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = Some(CachedToken {
            value: token.clone(),
            expires_at,
        });
    }
    Ok(token)
}

fn cached_provider_token() -> Option<String> {
    let now = unix_seconds();
    TOKEN_CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|cache| cache.clone())
        .filter(|token| token.expires_at.saturating_sub(now) > 600)
        .map(|token| token.value)
}

fn invalidate_provider_token() {
    if let Ok(mut cache) = TOKEN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = None;
    }
}

fn is_auth_failure(code: &Value, message: &str) -> bool {
    value_text(code) == "0"
        && ["鉴权", "授权", "认证", "token", "Token"]
            .iter()
            .any(|keyword| message.contains(keyword))
}

fn normalize_snapshot(
    kind: MeterKind,
    date: &str,
    config: &ProviderConfig,
    devices: Vec<ProviderDevice>,
    readings: Vec<ProviderReading>,
) -> SmartMeterSnapshot {
    let protocol = match kind {
        MeterKind::Electric => {
            std::env::var("YIZU_YMSINO_ELECTRIC_PROTOCOL").unwrap_or_else(|_| "M-Bus".into())
        }
        MeterKind::Water => {
            std::env::var("YIZU_YMSINO_WATER_PROTOCOL").unwrap_or_else(|_| "未确认".into())
        }
    };
    let mut normalized_devices = devices
        .into_iter()
        .filter(|device| {
            !device.factory_no.trim().is_empty()
                || !device.device_id.trim().is_empty()
                || !device.room_name.trim().is_empty()
        })
        .map(|device| {
            let room_name = non_empty(&device.room_name, &device.room_id);
            let pt = device.pt.trim();
            let ct = device.ct.trim();
            SmartMeterDevice {
                park_id: config.pt_id.clone(),
                park_name: config.pt_id.clone(),
                building_name: "设备档案".into(),
                floor_name: "设备列表".into(),
                room_id: device.room_id.trim().to_string(),
                room_name,
                device_id: device.device_id.trim().to_string(),
                factory_no: device.factory_no.trim().to_string(),
                protocol: protocol.clone(),
                current_ratio: [
                    (!pt.is_empty()).then(|| format!("PT:{pt}")),
                    (!ct.is_empty()).then(|| format!("CT:{ct}")),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" "),
                multiplier: calculate_multiplier(pt, ct),
            }
        })
        .collect::<Vec<_>>();
    normalized_devices.sort_by(|left, right| {
        left.room_name
            .cmp(&right.room_name)
            .then(left.factory_no.cmp(&right.factory_no))
    });
    let mut normalized_readings = readings
        .into_iter()
        .map(|reading| SmartMeterReading {
            room_id: reading.room_id.trim().to_string(),
            room_name: non_empty(&reading.room_name, &reading.room_id),
            device_id: reading.device_id.trim().to_string(),
            com_address: reading.factory_no.trim().to_string(),
            data_item_name: match kind {
                MeterKind::Electric => "正向有功电能".into(),
                MeterKind::Water => "累计流量".into(),
            },
            data_value: reading.total.trim().to_string(),
            data_value_tip: reading.tip.trim().to_string(),
            data_value_peak: reading.peak.trim().to_string(),
            data_value_flat: reading.flat.trim().to_string(),
            data_value_valley: reading.valley.trim().to_string(),
            freeze_time: reading.freeze_time.trim().to_string(),
            write_time: reading.freeze_time.trim().to_string(),
        })
        .collect::<Vec<_>>();
    normalized_readings.sort_by(|left, right| {
        right
            .freeze_time
            .cmp(&left.freeze_time)
            .then(left.com_address.cmp(&right.com_address))
    });
    SmartMeterSnapshot {
        kind,
        requested_date: date.into(),
        provider_name: "YMSINO".into(),
        park_name: config.pt_id.clone(),
        protocol,
        devices: normalized_devices,
        readings: normalized_readings,
    }
}

fn provider_config() -> ProviderConfig {
    dotenvy::dotenv().ok();
    ProviderConfig {
        base_url: env_or("YIZU_YMSINO_BASE_URL", DEFAULT_BASE_URL),
        username: env_or("YIZU_YMSINO_USERNAME", DEFAULT_USERNAME),
        password: env_or("YIZU_YMSINO_PASSWORD", DEFAULT_PASSWORD),
        org_id: env_or("YIZU_YMSINO_ORG_ID", DEFAULT_ORG_ID),
        pt_id: env_or("YIZU_YMSINO_PT_ID", DEFAULT_PT_ID),
        timeout_seconds: env_or("YIZU_YMSINO_TIMEOUT_SECONDS", "15")
            .parse()
            .unwrap_or(15),
    }
}

fn calculate_multiplier(pt: &str, ct: &str) -> f64 {
    match (pt.parse::<f64>().ok(), ct.parse::<f64>().ok()) {
        (Some(pt), Some(ct)) => pt * ct,
        (Some(pt), None) => pt,
        (None, Some(ct)) => ct,
        (None, None) => 1.0,
    }
}

fn non_empty(primary: &str, fallback: &str) -> String {
    let primary = primary.trim();
    if primary.is_empty() {
        fallback.trim().to_string()
    } else {
        primary.to_string()
    }
}

fn value_text(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string().trim_matches('"').to_string())
        .trim()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 变比计算兼容缺失字段() {
        assert_eq!(calculate_multiplier("10", "20"), 200.0);
        assert_eq!(calculate_multiplier("", "5"), 5.0);
        assert_eq!(calculate_multiplier("", ""), 1.0);
    }

    #[test]
    fn 冻结日期格式必须完整() {
        assert!(validate_date("2026-07-13").is_ok());
        assert!(validate_date("2026-7-13").is_err());
    }

    #[test]
    fn 供应商无数据空字符串按空列表处理() {
        let response = parse_provider_response::<ProviderDevice>(json!({
            "Code": "1",
            "Msg": "无数据",
            "Date": "",
        }))
        .expect("空字符串应兼容为无数据");
        assert!(response.data.is_empty());
    }
}
