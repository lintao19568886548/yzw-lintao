//! 合众平台电表设备与日冻结数据适配器。

use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{
    common::{env_or, unix_seconds, validate_date, validate_workspace_token},
    types::{
        MeterKind, SmartMeterCatalog, SmartMeterDevice, SmartMeterReading, SmartMeterReadingBatch,
        SmartMeterSnapshot,
    },
};

const DEFAULT_BASE_URL: &str = "https://devhzeb.szhzzd.top";
const DEFAULT_USERNAME: &str = "qcwyapi01";
const DEFAULT_LOGIN_KEY: &str = "41b1076bfe116a9ccfddd0c365";
const DEFAULT_PROJECT_CODE: &str = "241";
const DEFAULT_ELECTRIC_COM_TYPE: &str = "D.ZDG.FIWBM-GD04";

#[derive(Clone)]
struct HezhongConfig {
    base_url: String,
    username: String,
    login_key: String,
    project_code: String,
    electric_com_type: String,
    timeout_seconds: u64,
    connect_timeout_seconds: u64,
    retry_attempts: usize,
    retry_delay_ms: u64,
    operation_timeout_seconds: u64,
    cache_seconds: u64,
}

#[derive(Clone)]
struct CachedToken {
    value: String,
    expires_at: u64,
}

#[derive(Clone)]
struct TimedCache<T> {
    value: T,
    expires_at: u64,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    #[serde(default)]
    code: Value,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    token: String,
    #[serde(default)]
    expire: Value,
}

#[derive(Clone, Debug, Deserialize)]
struct ProviderDevice {
    #[serde(default)]
    address: String,
    #[serde(rename = "piplineName", default)]
    pipeline_name: String,
    #[serde(rename = "comAddress", default)]
    com_address: String,
    #[serde(rename = "meterId", default)]
    meter_id: Value,
    #[serde(rename = "productModelName", default)]
    product_model_name: String,
    #[serde(rename = "currentRatio", default)]
    current_ratio: Value,
    #[serde(default)]
    ratio: Value,
}

#[derive(Clone, Debug, Deserialize)]
struct ProviderReading {
    #[serde(rename = "comAddress", default)]
    com_address: String,
    #[serde(rename = "dataItemName", default)]
    data_item_name: String,
    #[serde(rename = "dataValue", default)]
    total: Value,
    #[serde(rename = "dataValue1", default)]
    tip: Value,
    #[serde(rename = "dataValue2", default)]
    peak: Value,
    #[serde(rename = "dataValue3", default)]
    flat: Value,
    #[serde(rename = "dataValue4", default)]
    valley: Value,
    #[serde(rename = "freezeTime", default)]
    freeze_time: String,
    #[serde(rename = "writeTime", default)]
    write_time: String,
}

static TOKEN_CACHE: OnceLock<Mutex<Option<CachedToken>>> = OnceLock::new();
static DEVICE_CACHE: OnceLock<Mutex<BTreeMap<String, TimedCache<Vec<ProviderDevice>>>>> =
    OnceLock::new();
static READING_CACHE: OnceLock<Mutex<BTreeMap<String, TimedCache<Vec<ProviderReading>>>>> =
    OnceLock::new();

/// 电表沿用正式系统的合众项目 241，水表仍由 YMSINO 适配器负责。
pub(super) async fn load_electric_snapshot(
    spacetime_token: &str,
    date: &str,
) -> Result<SmartMeterSnapshot, String> {
    validate_workspace_token(spacetime_token).await?;
    validate_date(date)?;
    dotenvy::dotenv().ok();
    let config = provider_config();
    let client = Client::builder()
        .timeout(Duration::from_secs(config.timeout_seconds))
        .connect_timeout(Duration::from_secs(config.connect_timeout_seconds))
        // 合众网关偶尔会关闭闲置连接，缩短连接池存活并强制 HTTP/1.1 更接近原 Axios 行为。
        .pool_idle_timeout(Duration::from_secs(20))
        .tcp_keepalive(Duration::from_secs(30))
        .http1_only()
        .build()
        .map_err(|error| format!("创建合众抄表客户端失败：{error}"))?;
    let devices = fetch_devices(&client, &config).await?;
    let readings = fetch_daily_readings(&client, &config, date).await?;
    Ok(normalize_snapshot(date, &config, devices, readings))
}

pub(super) async fn load_electric_catalog(
    spacetime_token: &str,
) -> Result<SmartMeterCatalog, String> {
    validate_workspace_token(spacetime_token).await?;
    dotenvy::dotenv().ok();
    let config = provider_config();
    let client = provider_client(&config)?;
    let operation = async {
        let devices = fetch_devices_cached(&client, &config).await?;
        let snapshot = normalize_snapshot("", &config, devices, Vec::new());
        Ok::<_, String>(SmartMeterCatalog {
            kind: MeterKind::Electric,
            provider_name: snapshot.provider_name,
            park_name: snapshot.park_name,
            protocol: snapshot.protocol,
            devices: snapshot.devices,
        })
    };
    tokio::time::timeout(
        Duration::from_secs(config.operation_timeout_seconds),
        operation,
    )
    .await
    .map_err(|_| "读取合众设备目录超时，请稍后重试".to_string())?
}

pub(super) async fn load_electric_readings(
    spacetime_token: &str,
    date: &str,
) -> Result<SmartMeterReadingBatch, String> {
    validate_workspace_token(spacetime_token).await?;
    validate_date(date)?;
    dotenvy::dotenv().ok();
    let config = provider_config();
    let client = provider_client(&config)?;
    let operation = async {
        // 设备目录通常已由左侧树请求写入缓存，此处只需再读取冻结数据。
        let devices = fetch_devices_cached(&client, &config).await?;
        let readings = fetch_daily_readings_cached(&client, &config, date).await?;
        let snapshot = normalize_snapshot(date, &config, devices, readings);
        Ok::<_, String>(SmartMeterReadingBatch {
            kind: MeterKind::Electric,
            requested_date: date.into(),
            readings: snapshot.readings,
        })
    };
    tokio::time::timeout(
        Duration::from_secs(config.operation_timeout_seconds),
        operation,
    )
    .await
    .map_err(|_| "读取合众日冻结数据超时，请稍后重试".to_string())?
}

fn provider_client(config: &HezhongConfig) -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(config.timeout_seconds))
        .connect_timeout(Duration::from_secs(config.connect_timeout_seconds))
        .pool_idle_timeout(Duration::from_secs(20))
        .tcp_keepalive(Duration::from_secs(30))
        .http1_only()
        .build()
        .map_err(|error| format!("创建合众抄表客户端失败：{error}"))
}

async fn fetch_devices_cached(
    client: &Client,
    config: &HezhongConfig,
) -> Result<Vec<ProviderDevice>, String> {
    let key = format!("{}:{}", config.project_code, config.electric_com_type);
    if let Some(devices) = cached_value(&DEVICE_CACHE, &key, false) {
        return Ok(devices);
    }
    match fetch_devices(client, config).await {
        Ok(devices) => {
            store_cached_value(&DEVICE_CACHE, key, devices.clone(), config.cache_seconds);
            Ok(devices)
        }
        Err(error) => cached_value(&DEVICE_CACHE, &key, true).ok_or(error),
    }
}

async fn fetch_daily_readings_cached(
    client: &Client,
    config: &HezhongConfig,
    date: &str,
) -> Result<Vec<ProviderReading>, String> {
    let key = format!(
        "{}:{}:{date}",
        config.project_code, config.electric_com_type
    );
    if let Some(readings) = cached_value(&READING_CACHE, &key, false) {
        return Ok(readings);
    }
    match fetch_daily_readings(client, config, date).await {
        Ok(readings) => {
            store_cached_value(&READING_CACHE, key, readings.clone(), config.cache_seconds);
            Ok(readings)
        }
        Err(error) => cached_value(&READING_CACHE, &key, true).ok_or(error),
    }
}

fn cached_value<T: Clone>(
    cache: &OnceLock<Mutex<BTreeMap<String, TimedCache<T>>>>,
    key: &str,
    allow_stale: bool,
) -> Option<T> {
    let now = unix_seconds();
    cache
        .get_or_init(|| Mutex::new(BTreeMap::new()))
        .lock()
        .ok()
        .and_then(|cache| cache.get(key).cloned())
        .filter(|entry| allow_stale || entry.expires_at > now)
        .map(|entry| entry.value)
}

fn store_cached_value<T>(
    cache: &OnceLock<Mutex<BTreeMap<String, TimedCache<T>>>>,
    key: String,
    value: T,
    cache_seconds: u64,
) {
    if let Ok(mut cache) = cache.get_or_init(|| Mutex::new(BTreeMap::new())).lock() {
        cache.insert(
            key,
            TimedCache {
                value,
                expires_at: unix_seconds().saturating_add(cache_seconds),
            },
        );
    }
}

async fn fetch_devices(
    client: &Client,
    config: &HezhongConfig,
) -> Result<Vec<ProviderDevice>, String> {
    let query = vec![
        ("comtype", config.electric_com_type.clone()),
        ("projCode", config.project_code.clone()),
        ("pageSize", "1000".into()),
        ("page", "1".into()),
    ];
    let payload =
        authenticated_get(client, config, "/hzeb-push/app/meterinfo/getDevice", &query).await?;
    parse_records(payload, "合众电表设备")
}

async fn fetch_daily_readings(
    client: &Client,
    config: &HezhongConfig,
    date: &str,
) -> Result<Vec<ProviderReading>, String> {
    let query = vec![
        ("projCode", config.project_code.clone()),
        ("type", "2".into()),
        ("timeFrom", format!("{date} 00:00:00")),
        ("timeTo", format!("{date} 23:59:59")),
        ("comType", config.electric_com_type.clone()),
        ("pageSize", "1000".into()),
        ("page", "1".into()),
    ];
    let payload = authenticated_get(
        client,
        config,
        "/hzeb-push/app/meterinfo/getHDMData",
        &query,
    )
    .await?;
    parse_records(payload, "合众日冻结数据")
}

async fn authenticated_get(
    client: &Client,
    config: &HezhongConfig,
    path: &str,
    query: &[(&str, String)],
) -> Result<Value, String> {
    let token = get_provider_token(client, config, false).await?;
    match send_get(client, config, path, query, &token).await {
        Ok(payload) if !is_auth_failure(&payload) => ensure_success(payload),
        Ok(_) | Err(ProviderRequestError::Unauthorized) => {
            invalidate_provider_token();
            let refreshed = get_provider_token(client, config, true).await?;
            let payload = send_get(client, config, path, query, &refreshed)
                .await
                .map_err(ProviderRequestError::into_message)?;
            ensure_success(payload)
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
            Self::Unauthorized => "合众平台鉴权失败".into(),
            Self::Message(message) => message,
        }
    }
}

async fn send_get(
    client: &Client,
    config: &HezhongConfig,
    path: &str,
    query: &[(&str, String)],
    token: &str,
) -> Result<Value, ProviderRequestError> {
    let url = format!("{}{}", config.base_url.trim_end_matches('/'), path);
    for attempt in 1..=config.retry_attempts {
        let response = client
            .get(&url)
            .header("token", token)
            .query(query)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) if attempt < config.retry_attempts && is_retryable_error(&error) => {
                wait_before_retry(config, attempt).await;
                continue;
            }
            Err(error) => {
                return Err(ProviderRequestError::Message(format!(
                    "合众平台连接失败（已尝试 {attempt} 次）：{}",
                    request_error_detail(&error)
                )));
            }
        };
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(ProviderRequestError::Unauthorized);
        }
        if is_retryable_status(response.status()) && attempt < config.retry_attempts {
            wait_before_retry(config, attempt).await;
            continue;
        }
        if !response.status().is_success() {
            return Err(ProviderRequestError::Message(format!(
                "合众平台返回 HTTP {}（已尝试 {attempt} 次）",
                response.status()
            )));
        }
        match response.json::<Value>().await {
            Ok(payload) => return Ok(payload),
            Err(_) if attempt < config.retry_attempts => {
                // 上游偶尔在响应体传输中断开连接，重新请求可恢复。
                wait_before_retry(config, attempt).await;
            }
            Err(error) => {
                return Err(ProviderRequestError::Message(format!(
                    "解析合众平台响应失败（已尝试 {attempt} 次）：{error}"
                )));
            }
        }
    }
    Err(ProviderRequestError::Message("合众平台请求重试耗尽".into()))
}

async fn get_provider_token(
    client: &Client,
    config: &HezhongConfig,
    force_refresh: bool,
) -> Result<String, String> {
    if !force_refresh {
        if let Some(token) = cached_provider_token() {
            return Ok(token);
        }
    }
    let now = unix_seconds();
    let num = format!("{:07}", signature_nonce() % 10_000_000);
    let raw = format!(
        "userName={}&time={now}&num={num}&key={}",
        config.username, config.login_key
    );
    let sign = format!("{:x}", Sha256::digest(raw.as_bytes()));
    let time = now.to_string();
    let login_url = format!(
        "{}/hzeb-push/app/xcx/login",
        config.base_url.trim_end_matches('/')
    );
    let login_query = [
        ("userName", config.username.as_str()),
        ("sign", sign.as_str()),
        ("time", time.as_str()),
        ("num", num.as_str()),
    ];
    let payload = send_login(client, config, &login_url, &login_query).await?;
    if payload.token.trim().is_empty() {
        let message = if payload.msg.is_empty() {
            String::new()
        } else {
            format!(" · {}", payload.msg)
        };
        return Err(format!(
            "合众平台未签发 Token：{}{message}",
            value_text(&payload.code),
        ));
    }
    let expires_in = value_text(&payload.expire).parse::<u64>().unwrap_or(3_600);
    let token = payload.token;
    if let Ok(mut cache) = TOKEN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = Some(CachedToken {
            value: token.clone(),
            expires_at: now.saturating_add(expires_in),
        });
    }
    Ok(token)
}

async fn send_login(
    client: &Client,
    config: &HezhongConfig,
    url: &str,
    query: &[(&str, &str)],
) -> Result<LoginResponse, String> {
    for attempt in 1..=config.retry_attempts {
        let response = client.post(url).query(query).send().await;
        let response = match response {
            Ok(response) => response,
            Err(error) if attempt < config.retry_attempts && is_retryable_error(&error) => {
                wait_before_retry(config, attempt).await;
                continue;
            }
            Err(error) => {
                return Err(format!(
                    "合众平台登录失败（已尝试 {attempt} 次）：{}",
                    request_error_detail(&error)
                ));
            }
        };
        if is_retryable_status(response.status()) && attempt < config.retry_attempts {
            wait_before_retry(config, attempt).await;
            continue;
        }
        if !response.status().is_success() {
            return Err(format!(
                "合众平台登录返回 HTTP {}（已尝试 {attempt} 次）",
                response.status()
            ));
        }
        match response.json::<LoginResponse>().await {
            Ok(payload) => return Ok(payload),
            Err(_) if attempt < config.retry_attempts => {
                wait_before_retry(config, attempt).await;
            }
            Err(error) => {
                return Err(format!(
                    "解析合众平台登录响应失败（已尝试 {attempt} 次）：{error}"
                ));
            }
        }
    }
    Err("合众平台登录重试耗尽".into())
}

fn is_retryable_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout() || error.is_request() || error.is_body()
}

fn is_retryable_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

fn request_error_detail(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "请求超时"
    } else if error.is_connect() {
        "无法建立网络连接"
    } else if error.is_body() {
        "响应传输中断"
    } else {
        "请求发送失败"
    }
}

async fn wait_before_retry(config: &HezhongConfig, attempt: usize) {
    tokio::time::sleep(Duration::from_millis(backoff_delay_ms(
        config.retry_delay_ms,
        attempt,
    )))
    .await;
}

fn backoff_delay_ms(base_delay_ms: u64, attempt: usize) -> u64 {
    let exponent = attempt.saturating_sub(1).min(4) as u32;
    base_delay_ms.saturating_mul(2u64.pow(exponent)).min(5_000)
}

fn cached_provider_token() -> Option<String> {
    let now = unix_seconds();
    TOKEN_CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|cache| cache.clone())
        .filter(|token| token.expires_at.saturating_sub(now) > 120)
        .map(|token| token.value)
}

fn invalidate_provider_token() {
    if let Ok(mut cache) = TOKEN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = None;
    }
}

fn ensure_success(payload: Value) -> Result<Value, String> {
    let code = payload.get("code").map(value_text).unwrap_or_default();
    if code.is_empty() || matches!(code.as_str(), "1" | "200" | "success" | "true") {
        Ok(payload)
    } else {
        let message = payload
            .get("msg")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let detail = if message.is_empty() {
            String::new()
        } else {
            format!(" · {message}")
        };
        Err(format!("合众平台拒绝读取：{code}{detail}"))
    }
}

fn is_auth_failure(payload: &Value) -> bool {
    matches!(
        payload.get("code").map(value_text).as_deref(),
        Some("401" | "403")
    )
}

fn parse_records<T>(payload: Value, label: &str) -> Result<Vec<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    let records = payload
        .pointer("/data/records")
        .or_else(|| payload.get("records"))
        .or_else(|| payload.pointer("/data/items"))
        .or_else(|| payload.get("items"))
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let array = match records {
        Value::Array(items) => Value::Array(items),
        Value::Null => Value::Array(Vec::new()),
        item => Value::Array(vec![item]),
    };
    serde_json::from_value(array).map_err(|error| format!("解析{label}失败：{error}"))
}

fn normalize_snapshot(
    date: &str,
    config: &HezhongConfig,
    devices: Vec<ProviderDevice>,
    readings: Vec<ProviderReading>,
) -> SmartMeterSnapshot {
    let mut normalized_devices = devices
        .into_iter()
        .filter(|device| {
            !device.com_address.trim().is_empty()
                || !device.pipeline_name.trim().is_empty()
                || !device.address.trim().is_empty()
        })
        .map(|device| normalize_device(config, device))
        .collect::<Vec<_>>();
    normalized_devices.sort_by(|left, right| {
        left.park_name
            .cmp(&right.park_name)
            .then(left.building_name.cmp(&right.building_name))
            .then(left.floor_name.cmp(&right.floor_name))
            .then(left.room_name.cmp(&right.room_name))
    });
    let device_by_address = normalized_devices
        .iter()
        .map(|device| (device.factory_no.clone(), device))
        .collect::<BTreeMap<_, _>>();
    let mut normalized_readings = readings
        .into_iter()
        .map(|reading| {
            let device = device_by_address.get(reading.com_address.trim());
            SmartMeterReading {
                room_id: device
                    .map(|value| value.room_id.clone())
                    .unwrap_or_else(|| reading.com_address.clone()),
                room_name: device
                    .map(|value| value.room_name.clone())
                    .unwrap_or_else(|| reading.com_address.clone()),
                device_id: device
                    .map(|value| value.device_id.clone())
                    .unwrap_or_default(),
                com_address: reading.com_address.trim().to_string(),
                data_item_name: reading.data_item_name.trim().to_string(),
                data_value: value_text(&reading.total),
                data_value_tip: value_text(&reading.tip),
                data_value_peak: value_text(&reading.peak),
                data_value_flat: value_text(&reading.flat),
                data_value_valley: value_text(&reading.valley),
                freeze_time: reading.freeze_time.trim().to_string(),
                write_time: reading.write_time.trim().to_string(),
            }
        })
        .collect::<Vec<_>>();
    normalized_readings.sort_by(|left, right| {
        right
            .freeze_time
            .cmp(&left.freeze_time)
            .then(left.room_name.cmp(&right.room_name))
    });
    let park_name = normalized_devices
        .first()
        .map(|device| device.park_name.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("项目 {}", config.project_code));
    SmartMeterSnapshot {
        kind: MeterKind::Electric,
        requested_date: date.into(),
        provider_name: "合众".into(),
        park_name,
        protocol: config.electric_com_type.clone(),
        devices: normalized_devices,
        readings: normalized_readings,
    }
}

fn normalize_device(config: &HezhongConfig, device: ProviderDevice) -> SmartMeterDevice {
    let (park_name, building_name, floor_name, room_name) = extract_location(&device);
    let current_ratio = value_text(&device.current_ratio);
    let multiplier = value_text(&device.ratio)
        .parse::<f64>()
        .or_else(|_| current_ratio.parse::<f64>())
        .unwrap_or(1.0);
    SmartMeterDevice {
        park_id: config.project_code.clone(),
        park_name,
        building_name,
        floor_name,
        room_id: room_name.clone(),
        room_name,
        device_id: value_text(&device.meter_id),
        factory_no: device.com_address.trim().to_string(),
        protocol: if device.product_model_name.trim().is_empty() {
            config.electric_com_type.clone()
        } else {
            device.product_model_name.trim().to_string()
        },
        current_ratio,
        multiplier,
    }
}

/// 与原 Node 接口保持一致：去掉地址末尾“户内”，房号取设备名称末尾数字。
fn extract_location(device: &ProviderDevice) -> (String, String, String, String) {
    let mut segments = device
        .address
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if !segments.is_empty() {
        segments.pop();
    }
    let room_name = trailing_room_number(&device.pipeline_name)
        .or_else(|| segments.last().cloned())
        .unwrap_or_else(|| device.pipeline_name.trim().to_string());
    let park_name = segments.first().cloned().unwrap_or_default();
    let building_name = segments.get(1).cloned().unwrap_or_default();
    let floor_name = segments.get(2..).unwrap_or_default().join("/");
    (park_name, building_name, floor_name, room_name)
}

fn trailing_room_number(value: &str) -> Option<String> {
    let digits = value
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    (digits.len() >= 3).then_some(digits)
}

fn provider_config() -> HezhongConfig {
    HezhongConfig {
        base_url: env_or("YIZU_HEZHONG_BASE_URL", DEFAULT_BASE_URL),
        username: env_or("YIZU_HEZHONG_USERNAME", DEFAULT_USERNAME),
        login_key: env_or("YIZU_HEZHONG_LOGIN_KEY", DEFAULT_LOGIN_KEY),
        project_code: env_or("YIZU_HEZHONG_PROJECT_CODE", DEFAULT_PROJECT_CODE),
        electric_com_type: env_or("YIZU_HEZHONG_ELECTRIC_COM_TYPE", DEFAULT_ELECTRIC_COM_TYPE),
        timeout_seconds: env_or("YIZU_HEZHONG_TIMEOUT_SECONDS", "15")
            .parse()
            .unwrap_or(15),
        connect_timeout_seconds: env_or("YIZU_HEZHONG_CONNECT_TIMEOUT_SECONDS", "6")
            .parse()
            .unwrap_or(6),
        retry_attempts: env_or("YIZU_HEZHONG_RETRY_ATTEMPTS", "3")
            .parse::<usize>()
            .unwrap_or(3)
            .clamp(1, 5),
        retry_delay_ms: env_or("YIZU_HEZHONG_RETRY_DELAY_MS", "300")
            .parse()
            .unwrap_or(300),
        operation_timeout_seconds: env_or("YIZU_HEZHONG_OPERATION_TIMEOUT_SECONDS", "20")
            .parse()
            .unwrap_or(20),
        cache_seconds: env_or("YIZU_HEZHONG_CACHE_SECONDS", "300")
            .parse()
            .unwrap_or(300),
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.trim().to_string(),
        other => other.to_string().trim_matches('"').trim().to_string(),
    }
}

fn signature_nonce() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device() -> ProviderDevice {
        serde_json::from_value(serde_json::json!({
            "address": "十一高埗园区/A栋/3楼/户内",
            "piplineName": "电表-十一高埗园区A栋3楼8303",
            "comAddress": "250603033969",
            "meterId": 1831655,
            "productModelName": "D.ZDG.FIWBM-GD04",
            "currentRatio": "1",
            "ratio": 1
        }))
        .unwrap()
    }

    #[test]
    fn 正式设备地址可拆成园区楼栋楼层和房号() {
        assert_eq!(
            extract_location(&device()),
            (
                "十一高埗园区".into(),
                "A栋".into(),
                "3楼".into(),
                "8303".into()
            )
        );
    }

    #[test]
    fn 数值字段可以稳定转换为页面文本() {
        assert_eq!(value_text(&serde_json::json!(2040.07)), "2040.07");
        assert_eq!(value_text(&Value::Null), "");
    }

    #[test]
    fn 重试等待时间按指数增长并限制上限() {
        assert_eq!(backoff_delay_ms(300, 1), 300);
        assert_eq!(backoff_delay_ms(300, 2), 600);
        assert_eq!(backoff_delay_ms(300, 3), 1_200);
        assert_eq!(backoff_delay_ms(2_000, 5), 5_000);
    }

    #[test]
    fn 临时网关状态会触发重试() {
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
    }
}
