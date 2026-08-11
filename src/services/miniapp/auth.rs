use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;

use super::{
    clock::Clock,
    config::{MiniappServerConfig, SmsProviderConfig, TokenSecrets, WechatProviderConfig},
    types::{
        DevSessionResponse, SmsSendRequest, SmsSendResponse, SmsVerifyRequest, WechatLoginRequest,
    },
    validation::{mask_phone, validate_phone},
};

type HmacSha256 = Hmac<Sha256>;
type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ProviderError>> + Send + 'a>>;

const PROVIDER_CONNECT_TIMEOUT_SECONDS: u64 = 5;
const PROVIDER_TOTAL_TIMEOUT_SECONDS: u64 = 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ProviderError;

pub(super) trait SmsSender: Send + Sync {
    fn send_code<'a>(&'a self, phone: &'a str, code: &'a str) -> ProviderFuture<'a, ()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WechatIdentity {
    pub openid: String,
    pub unionid: Option<String>,
}

pub(super) trait WechatCodeExchanger: Send + Sync {
    fn exchange<'a>(&'a self, code: &'a str) -> ProviderFuture<'a, WechatIdentity>;
}

struct DisabledSmsSender;

impl SmsSender for DisabledSmsSender {
    fn send_code<'a>(&'a self, _phone: &'a str, _code: &'a str) -> ProviderFuture<'a, ()> {
        Box::pin(async { Err(ProviderError) })
    }
}

struct DisabledWechatCodeExchanger;

impl WechatCodeExchanger for DisabledWechatCodeExchanger {
    fn exchange<'a>(&'a self, _code: &'a str) -> ProviderFuture<'a, WechatIdentity> {
        Box::pin(async { Err(ProviderError) })
    }
}

struct LianluSmsSender {
    config: SmsProviderConfig,
    client: reqwest::Client,
}

impl LianluSmsSender {
    fn new(config: SmsProviderConfig) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(PROVIDER_CONNECT_TIMEOUT_SECONDS))
            .timeout(Duration::from_secs(PROVIDER_TOTAL_TIMEOUT_SECONDS))
            .build()
            .map_err(|_| "SMS_PROVIDER=INVALID".to_string())?;
        Ok(Self { config, client })
    }

    fn signature(&self, timestamp: &str) -> Result<String, ProviderError> {
        let mut fields = BTreeMap::new();
        fields.insert("AppId", self.config.app_id.as_str());
        fields.insert("MchId", self.config.merchant_id.as_str());
        fields.insert("SignName", self.config.sign_name.as_str());
        fields.insert("SignType", self.config.sign_type.as_str());
        fields.insert("TemplateId", self.config.template_id.as_str());
        fields.insert("TimeStamp", timestamp);
        fields.insert("Type", self.config.message_type.as_str());
        fields.insert("Version", self.config.version.as_str());
        let parameters = fields
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        let raw = format!("{parameters}&key={}", self.config.secret_key);
        let mut signer = HmacSha256::new_from_slice(self.config.secret_key.as_bytes())
            .map_err(|_| ProviderError)?;
        signer.update(raw.as_bytes());
        Ok(signer
            .finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect())
    }
}

impl SmsSender for LianluSmsSender {
    fn send_code<'a>(&'a self, phone: &'a str, code: &'a str) -> ProviderFuture<'a, ()> {
        Box::pin(async move {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| ProviderError)?
                .as_millis()
                .to_string();
            let payload = serde_json::json!({
                "AppId": self.config.app_id,
                "MchId": self.config.merchant_id,
                "SignName": self.config.sign_name,
                "Version": self.config.version,
                "Type": self.config.message_type,
                "TemplateId": self.config.template_id,
                "PhoneNumberSet": [phone],
                "TemplateParamSet": [code],
                "TimeStamp": timestamp,
                "SignType": self.config.sign_type,
                "Signature": self.signature(&timestamp)?,
            });
            let response = self
                .client
                .post(&self.config.api_url)
                .header(
                    reqwest::header::CONTENT_TYPE,
                    "application/json;charset=utf-8",
                )
                .body(payload.to_string())
                .send()
                .await
                .map_err(|_| ProviderError)?;
            let status = response.status();
            let body = response.text().await.map_err(|_| ProviderError)?;
            let response: Value = serde_json::from_str(&body).map_err(|_| ProviderError)?;
            if !status.is_success() || response.get("status").and_then(Value::as_str) != Some("00")
            {
                return Err(ProviderError);
            }
            Ok(())
        })
    }
}

struct HttpWechatCodeExchanger {
    config: WechatProviderConfig,
    client: reqwest::Client,
}

impl HttpWechatCodeExchanger {
    fn new(config: WechatProviderConfig) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(PROVIDER_CONNECT_TIMEOUT_SECONDS))
            .timeout(Duration::from_secs(PROVIDER_TOTAL_TIMEOUT_SECONDS))
            .build()
            .map_err(|_| "WECHAT_PROVIDER=INVALID".to_string())?;
        Ok(Self { config, client })
    }
}

impl WechatCodeExchanger for HttpWechatCodeExchanger {
    fn exchange<'a>(&'a self, code: &'a str) -> ProviderFuture<'a, WechatIdentity> {
        Box::pin(async move {
            // The secret is intentionally carried in the HTTPS request body instead of
            // query parameters so reverse-proxy access logs cannot capture it.
            let payload = serde_json::json!({
                "appid": self.config.app_id,
                "secret": self.config.app_secret,
                "js_code": code,
                "grant_type": "authorization_code",
            });
            let response = self
                .client
                .post(&self.config.exchange_url)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(payload.to_string())
                .send()
                .await
                .map_err(|_| ProviderError)?;
            let status = response.status();
            let body = response.text().await.map_err(|_| ProviderError)?;
            let value: Value = serde_json::from_str(&body).map_err(|_| ProviderError)?;
            if !status.is_success()
                || value
                    .get("errcode")
                    .and_then(Value::as_i64)
                    .is_some_and(|code| code != 0)
            {
                return Err(ProviderError);
            }
            let openid = value
                .get("openid")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .ok_or(ProviderError)?
                .to_string();
            let unionid = value
                .get("unionid")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            Ok(WechatIdentity { openid, unionid })
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthError {
    pub code: String,
    pub message: String,
}

impl AuthError {
    fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AuthPolicy {
    pub code_length: usize,
    pub code_ttl_seconds: u64,
    pub resend_interval_seconds: u64,
    pub max_verify_attempts: u32,
    pub phone_hourly_limit: u32,
    pub phone_daily_limit: u32,
    pub ip_hourly_limit: u32,
    pub ip_daily_limit: u32,
    pub device_hourly_limit: u32,
    pub device_daily_limit: u32,
}

impl Default for AuthPolicy {
    fn default() -> Self {
        Self {
            code_length: 6,
            code_ttl_seconds: 300,
            resend_interval_seconds: 60,
            max_verify_attempts: 5,
            phone_hourly_limit: 5,
            phone_daily_limit: 10,
            ip_hourly_limit: 20,
            ip_daily_limit: 100,
            device_hourly_limit: 10,
            device_daily_limit: 30,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RequestIdentity {
    pub ip: String,
    pub device_id: String,
}

#[derive(Clone)]
struct SmsChallenge {
    digest: [u8; 32],
    device_key: String,
    sent_at: u64,
    expires_at: u64,
    attempts: u32,
    sending: bool,
    sequence: u64,
}

#[derive(Default)]
struct AuthRuntime {
    challenges: BTreeMap<String, SmsChallenge>,
    rate_counters: BTreeMap<(String, &'static str, u64), u32>,
    used_wechat_codes: BTreeMap<String, u64>,
    users: BTreeMap<String, String>,
    sequence: u64,
}

pub struct MiniappAuthService {
    clock: Arc<dyn Clock>,
    sms_sender: Arc<dyn SmsSender>,
    wechat_exchanger: Arc<dyn WechatCodeExchanger>,
    sms_enabled: bool,
    wechat_enabled: bool,
    digest_secret: Option<String>,
    token_secrets: Option<TokenSecrets>,
    policy: AuthPolicy,
    runtime: Mutex<AuthRuntime>,
}

impl MiniappAuthService {
    pub fn disabled(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            sms_sender: Arc::new(DisabledSmsSender),
            wechat_exchanger: Arc::new(DisabledWechatCodeExchanger),
            sms_enabled: false,
            wechat_enabled: false,
            digest_secret: None,
            token_secrets: None,
            policy: AuthPolicy::default(),
            runtime: Mutex::new(AuthRuntime::default()),
        }
    }

    pub fn from_server_config(
        config: &MiniappServerConfig,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, String> {
        let sms_sender: Arc<dyn SmsSender> = match (config.sms_enabled, config.sms.clone()) {
            (true, Some(sms)) => Arc::new(LianluSmsSender::new(sms)?),
            _ => Arc::new(DisabledSmsSender),
        };
        let wechat_exchanger: Arc<dyn WechatCodeExchanger> =
            match (config.wechat_enabled, config.wechat.clone()) {
                (true, Some(wechat)) => Arc::new(HttpWechatCodeExchanger::new(wechat)?),
                _ => Arc::new(DisabledWechatCodeExchanger),
            };
        let policy = config
            .sms
            .as_ref()
            .map_or_else(AuthPolicy::default, |sms| AuthPolicy {
                code_length: sms.code_length,
                code_ttl_seconds: sms.code_ttl_seconds,
                resend_interval_seconds: sms.resend_interval_seconds,
                max_verify_attempts: sms.max_verify_attempts,
                ..AuthPolicy::default()
            });
        let digest_secret = config
            .sms
            .as_ref()
            .map(|sms| sms.secret_key.clone())
            .or_else(|| {
                config
                    .token_secrets
                    .as_ref()
                    .map(|tokens| tokens.access.clone())
            });
        Ok(Self {
            clock,
            sms_sender,
            wechat_exchanger,
            sms_enabled: config.sms_enabled,
            wechat_enabled: config.wechat_enabled,
            digest_secret,
            token_secrets: config.token_secrets.clone(),
            policy,
            runtime: Mutex::new(AuthRuntime::default()),
        })
    }

    pub async fn send_sms_code(
        &self,
        request: SmsSendRequest,
        identity: RequestIdentity,
    ) -> Result<SmsSendResponse, AuthError> {
        if !self.sms_enabled {
            return Err(AuthError::new("SMS_NOT_CONFIGURED", "短信登录尚未启用"));
        }
        validate_auth_request(&request.phone, &identity.device_id)?;
        let secret = self
            .digest_secret
            .as_deref()
            .ok_or_else(|| AuthError::new("SMS_NOT_CONFIGURED", "短信登录尚未启用"))?;
        let now = self.clock.now_epoch_seconds();
        let phone_key = keyed_identifier(secret, "phone", &request.phone)?;
        let device_key = keyed_identifier(secret, "device", &identity.device_id)?;
        let ip_key = keyed_identifier(secret, "ip", &identity.ip)?;

        let code = generate_numeric_code(self.policy.code_length)?;
        let digest = verification_digest(secret, &request.phone, &identity.device_id, &code)?;
        let sequence = {
            let mut runtime = self.runtime.lock().map_err(internal_error)?;
            if let Some(existing) = runtime.challenges.get(&phone_key) {
                let resend_at = existing
                    .sent_at
                    .saturating_add(self.policy.resend_interval_seconds);
                if now < resend_at {
                    return Err(AuthError::new(
                        "SMS_RESEND_TOO_SOON",
                        "验证码发送过于频繁，请稍后重试",
                    ));
                }
            }
            enforce_rate_limits(
                &mut runtime,
                now,
                &phone_key,
                &ip_key,
                &device_key,
                &self.policy,
            )?;
            runtime.sequence = runtime.sequence.saturating_add(1);
            let sequence = runtime.sequence;
            runtime.challenges.insert(
                phone_key.clone(),
                SmsChallenge {
                    digest,
                    device_key: device_key.clone(),
                    sent_at: now,
                    expires_at: now.saturating_add(self.policy.code_ttl_seconds),
                    attempts: 0,
                    sending: true,
                    sequence,
                },
            );
            sequence
        };

        if self
            .sms_sender
            .send_code(&request.phone, &code)
            .await
            .is_err()
        {
            if let Ok(mut runtime) = self.runtime.lock() {
                if runtime
                    .challenges
                    .get(&phone_key)
                    .is_some_and(|challenge| challenge.sequence == sequence)
                {
                    runtime.challenges.remove(&phone_key);
                }
            }
            return Err(AuthError::new(
                "SMS_SEND_FAILED",
                "验证码发送失败，请稍后重试",
            ));
        }
        if let Ok(mut runtime) = self.runtime.lock() {
            if let Some(challenge) = runtime.challenges.get_mut(&phone_key) {
                if challenge.sequence == sequence {
                    challenge.sending = false;
                }
            }
        }
        Ok(SmsSendResponse {
            expires_in_seconds: self.policy.code_ttl_seconds,
            retry_after_seconds: self.policy.resend_interval_seconds,
        })
    }

    pub fn verify_sms_code(
        &self,
        request: SmsVerifyRequest,
    ) -> Result<DevSessionResponse, AuthError> {
        if !self.sms_enabled {
            return Err(AuthError::new("SMS_NOT_CONFIGURED", "短信登录尚未启用"));
        }
        if !request.agreements_accepted {
            return Err(AuthError::new(
                "AGREEMENT_REQUIRED",
                "请阅读并同意用户协议与隐私政策",
            ));
        }
        validate_auth_request(&request.phone, &request.device_id)?;
        if request.code.len() != self.policy.code_length
            || !request.code.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(AuthError::new(
                "VALIDATION_ERROR",
                "手机号或验证码格式不正确",
            ));
        }
        let secret = self
            .digest_secret
            .as_deref()
            .ok_or_else(|| AuthError::new("SMS_NOT_CONFIGURED", "短信登录尚未启用"))?;
        let now = self.clock.now_epoch_seconds();
        let phone_key = keyed_identifier(secret, "phone", &request.phone)?;
        let device_key = keyed_identifier(secret, "device", &request.device_id)?;
        let subject = {
            let mut runtime = self.runtime.lock().map_err(internal_error)?;
            let Some(challenge) = runtime.challenges.get_mut(&phone_key) else {
                return Err(AuthError::new("SMS_CODE_INVALID", "验证码不存在或已失效"));
            };
            if challenge.sending || challenge.expires_at <= now {
                runtime.challenges.remove(&phone_key);
                return Err(AuthError::new("SMS_CODE_EXPIRED", "验证码已经过期"));
            }
            let valid = challenge.device_key == device_key
                && verify_verification_digest(
                    secret,
                    &request.phone,
                    &request.device_id,
                    &request.code,
                    &challenge.digest,
                );
            if !valid {
                challenge.attempts = challenge.attempts.saturating_add(1);
                if challenge.attempts >= self.policy.max_verify_attempts {
                    runtime.challenges.remove(&phone_key);
                }
                return Err(AuthError::new("SMS_CODE_INVALID", "验证码不正确"));
            }
            runtime.challenges.remove(&phone_key);
            upsert_user(&mut runtime, format!("phone:{phone_key}"))
        };
        self.issue_session(
            &subject,
            &request.phone,
            Some(&request.phone),
            mask_phone(&request.phone),
            now,
        )
    }

    pub async fn exchange_wechat_code(
        &self,
        request: WechatLoginRequest,
    ) -> Result<DevSessionResponse, AuthError> {
        if !self.wechat_enabled {
            return Err(AuthError::new("WECHAT_NOT_CONFIGURED", "微信登录尚未启用"));
        }
        if !request.agreements_accepted {
            return Err(AuthError::new(
                "AGREEMENT_REQUIRED",
                "请阅读并同意用户协议与隐私政策",
            ));
        }
        validate_device_id(&request.device_id)?;
        let code = request.code.trim();
        if code.is_empty() || code.len() > 128 {
            return Err(AuthError::new("VALIDATION_ERROR", "微信登录凭证格式无效"));
        }
        let secret = self
            .digest_secret
            .as_deref()
            .ok_or_else(|| AuthError::new("WECHAT_NOT_CONFIGURED", "微信登录尚未启用"))?;
        let now = self.clock.now_epoch_seconds();
        let code_key = keyed_identifier(secret, "wechat-code", code)?;
        {
            let mut runtime = self.runtime.lock().map_err(internal_error)?;
            runtime
                .used_wechat_codes
                .retain(|_, expires| *expires > now);
            if runtime.used_wechat_codes.contains_key(&code_key) {
                return Err(AuthError::new(
                    "WECHAT_CODE_USED",
                    "微信登录凭证已使用，请重试",
                ));
            }
        }
        let identity = self
            .wechat_exchanger
            .exchange(code)
            .await
            .map_err(|_| AuthError::new("WECHAT_LOGIN_FAILED", "微信登录失败，请重试"))?;
        let identity_key = keyed_identifier(secret, "openid", &identity.openid)?;
        let subject = {
            let mut runtime = self.runtime.lock().map_err(internal_error)?;
            if runtime.used_wechat_codes.contains_key(&code_key) {
                return Err(AuthError::new(
                    "WECHAT_CODE_USED",
                    "微信登录凭证已使用，请重试",
                ));
            }
            runtime
                .used_wechat_codes
                .insert(code_key, now.saturating_add(600));
            upsert_user(&mut runtime, format!("wechat:{identity_key}"))
        };
        self.issue_session(
            &subject,
            &format!("wx_{identity_key}"),
            None,
            "微信用户".into(),
            now,
        )
    }

    fn issue_session(
        &self,
        user_id: &str,
        username: &str,
        phone: Option<&str>,
        masked_phone: String,
        now: u64,
    ) -> Result<DevSessionResponse, AuthError> {
        let tokens = self
            .token_secrets
            .as_ref()
            .ok_or_else(|| AuthError::new("AUTH_NOT_CONFIGURED", "认证服务尚未配置"))?;
        let access_expires = now.saturating_add(2 * 60 * 60);
        let refresh_expires = now.saturating_add(30 * 24 * 60 * 60);
        Ok(DevSessionResponse {
            // Keep the existing SpacetimeDB `legacy_sms` bridge usable: the
            // access token is a standard HS256 JWT with the claims that
            // reducer already validates. The miniapp still sends it only to
            // this BFF; it never connects to SpacetimeDB directly.
            session_token: issue_access_token(
                &tokens.access,
                user_id,
                username,
                phone,
                now,
                access_expires,
            )?,
            refresh_token: Some(issue_opaque_token(
                &tokens.refresh,
                "refresh",
                user_id,
                refresh_expires,
            )?),
            masked_phone,
            expires_at_epoch_seconds: access_expires,
            local_demo: false,
        })
    }

    #[cfg(test)]
    fn for_tests(
        clock: Arc<dyn Clock>,
        sms_sender: Arc<dyn SmsSender>,
        wechat_exchanger: Arc<dyn WechatCodeExchanger>,
        policy: AuthPolicy,
    ) -> Self {
        Self {
            clock,
            sms_sender,
            wechat_exchanger,
            sms_enabled: true,
            wechat_enabled: true,
            digest_secret: Some("synthetic-unit-test-digest-material".into()),
            token_secrets: Some(TokenSecrets {
                access: "synthetic-unit-test-access-material-0001".into(),
                refresh: "synthetic-unit-test-refresh-material-0002".into(),
            }),
            policy,
            runtime: Mutex::new(AuthRuntime::default()),
        }
    }
}

fn validate_auth_request(phone: &str, device_id: &str) -> Result<(), AuthError> {
    if !validate_phone(phone) {
        return Err(AuthError::new(
            "VALIDATION_ERROR",
            "请输入有效的11位中国大陆手机号",
        ));
    }
    validate_device_id(device_id)
}

fn validate_device_id(device_id: &str) -> Result<(), AuthError> {
    if !(16..=80).contains(&device_id.len())
        || !device_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(AuthError::new("VALIDATION_ERROR", "设备标识格式无效"));
    }
    Ok(())
}

fn enforce_rate_limits(
    runtime: &mut AuthRuntime,
    now: u64,
    phone: &str,
    ip: &str,
    device: &str,
    policy: &AuthPolicy,
) -> Result<(), AuthError> {
    let checks = [
        (phone, "hour", now / 3600, policy.phone_hourly_limit),
        (phone, "day", now / 86400, policy.phone_daily_limit),
        (ip, "hour", now / 3600, policy.ip_hourly_limit),
        (ip, "day", now / 86400, policy.ip_daily_limit),
        (device, "hour", now / 3600, policy.device_hourly_limit),
        (device, "day", now / 86400, policy.device_daily_limit),
    ];
    if checks.iter().any(|(key, window, bucket, limit)| {
        runtime
            .rate_counters
            .get(&(key.to_string(), *window, *bucket))
            .copied()
            .unwrap_or_default()
            >= *limit
    }) {
        return Err(AuthError::new(
            "SMS_RATE_LIMITED",
            "验证码请求过于频繁，请稍后重试",
        ));
    }
    for (key, window, bucket, _) in checks {
        *runtime
            .rate_counters
            .entry((key.to_string(), window, bucket))
            .or_default() += 1;
    }
    Ok(())
}

fn upsert_user(runtime: &mut AuthRuntime, subject_key: String) -> String {
    if let Some(user) = runtime.users.get(&subject_key) {
        return user.clone();
    }
    runtime.sequence = runtime.sequence.saturating_add(1);
    let user = format!("miniapp-user-{:x}", runtime.sequence);
    runtime.users.insert(subject_key, user.clone());
    user
}

fn generate_numeric_code(length: usize) -> Result<String, AuthError> {
    let mut code = String::with_capacity(length);
    while code.len() < length {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|_| AuthError::new("INTERNAL_ERROR", "认证服务暂时不可用"))?;
        for byte in bytes {
            // Rejection sampling avoids the modulo bias of mapping all 256
            // byte values directly into ten decimal digits.
            if byte < 250 {
                code.push(char::from(b'0' + byte % 10));
                if code.len() == length {
                    break;
                }
            }
        }
    }
    Ok(code)
}

fn verification_digest(
    secret: &str,
    phone: &str,
    device_id: &str,
    code: &str,
) -> Result<[u8; 32], AuthError> {
    let mut signer = HmacSha256::new_from_slice(secret.as_bytes()).map_err(internal_error)?;
    signer.update(b"sms-code-v1\0");
    signer.update(phone.as_bytes());
    signer.update(b"\0");
    signer.update(device_id.as_bytes());
    signer.update(b"\0");
    signer.update(code.as_bytes());
    Ok(signer.finalize().into_bytes().into())
}

fn verify_verification_digest(
    secret: &str,
    phone: &str,
    device_id: &str,
    code: &str,
    expected: &[u8; 32],
) -> bool {
    let Ok(mut verifier) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    verifier.update(b"sms-code-v1\0");
    verifier.update(phone.as_bytes());
    verifier.update(b"\0");
    verifier.update(device_id.as_bytes());
    verifier.update(b"\0");
    verifier.update(code.as_bytes());
    verifier.verify_slice(expected).is_ok()
}

fn keyed_identifier(secret: &str, namespace: &str, value: &str) -> Result<String, AuthError> {
    let mut signer = HmacSha256::new_from_slice(secret.as_bytes()).map_err(internal_error)?;
    signer.update(namespace.as_bytes());
    signer.update(b"\0");
    signer.update(value.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(signer.finalize().into_bytes()))
}

fn issue_access_token(
    secret: &str,
    user_id: &str,
    username: &str,
    phone: Option<&str>,
    now: u64,
    expires: u64,
) -> Result<String, AuthError> {
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce)
        .map_err(|_| AuthError::new("INTERNAL_ERROR", "认证服务暂时不可用"))?;
    let header = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({ "alg": "HS256", "typ": "JWT" }))
            .map_err(internal_error)?,
    );
    let mut claims = serde_json::json!({
        "sub": user_id,
        "username": username,
        "realName": username,
        "customerId": "public",
        "roles": ["User"],
        "tokenVersion": 1,
        "iat": now,
        "nbf": now,
        "exp": expires,
        "jti": URL_SAFE_NO_PAD.encode(nonce),
    });
    if let Some(phone) = phone {
        claims["phone"] = Value::String(phone.into());
    }
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).map_err(internal_error)?);
    let signing_input = format!("{header}.{payload}");
    let mut signer = HmacSha256::new_from_slice(secret.as_bytes()).map_err(internal_error)?;
    signer.update(signing_input.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(signer.finalize().into_bytes());
    Ok(format!("{signing_input}.{signature}"))
}

fn issue_opaque_token(
    secret: &str,
    kind: &str,
    user_id: &str,
    expires: u64,
) -> Result<String, AuthError> {
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce)
        .map_err(|_| AuthError::new("INTERNAL_ERROR", "认证服务暂时不可用"))?;
    let payload = URL_SAFE_NO_PAD.encode(format!(
        "{kind}.{user_id}.{expires}.{}",
        URL_SAFE_NO_PAD.encode(nonce)
    ));
    let mut signer = HmacSha256::new_from_slice(secret.as_bytes()).map_err(internal_error)?;
    signer.update(payload.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(signer.finalize().into_bytes());
    Ok(format!("{payload}.{signature}"))
}

fn internal_error<T>(_error: T) -> AuthError {
    AuthError::new("INTERNAL_ERROR", "认证服务暂时不可用")
}

#[cfg(test)]
#[derive(Default)]
pub struct MockSmsSender {
    sent: Mutex<Vec<(String, String)>>,
}

#[cfg(test)]
impl MockSmsSender {
    fn last_code(&self) -> String {
        self.sent
            .lock()
            .expect("mock SMS lock")
            .last()
            .expect("mock SMS sent")
            .1
            .clone()
    }
}

#[cfg(test)]
impl SmsSender for MockSmsSender {
    fn send_code<'a>(&'a self, phone: &'a str, code: &'a str) -> ProviderFuture<'a, ()> {
        Box::pin(async move {
            self.sent
                .lock()
                .map_err(|_| ProviderError)?
                .push((phone.into(), code.into()));
            Ok(())
        })
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct MockWechatCodeExchanger {
    calls: Mutex<Vec<String>>,
}

#[cfg(test)]
impl WechatCodeExchanger for MockWechatCodeExchanger {
    fn exchange<'a>(&'a self, code: &'a str) -> ProviderFuture<'a, WechatIdentity> {
        Box::pin(async move {
            self.calls
                .lock()
                .map_err(|_| ProviderError)?
                .push(code.into());
            Ok(WechatIdentity {
                openid: "synthetic-openid".into(),
                unionid: Some("synthetic-unionid".into()),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use time::{Date, Month};

    use super::*;
    use crate::services::miniapp::clock::FixedClock;

    fn phone() -> String {
        ["139", "0000", "0000"].concat()
    }

    fn identity() -> RequestIdentity {
        RequestIdentity {
            ip: "192.0.2.10".into(),
            device_id: "device_test_abcdefghijkl".into(),
        }
    }

    fn clock() -> Arc<FixedClock> {
        Arc::new(FixedClock::new(
            1_800_000_000,
            Date::from_calendar_date(2027, Month::January, 15).expect("date"),
        ))
    }

    fn service(policy: AuthPolicy) -> (MiniappAuthService, Arc<MockSmsSender>, Arc<FixedClock>) {
        let clock = clock();
        let sender = Arc::new(MockSmsSender::default());
        let wechat = Arc::new(MockWechatCodeExchanger::default());
        (
            MiniappAuthService::for_tests(clock.clone(), sender.clone(), wechat, policy),
            sender,
            clock,
        )
    }

    #[tokio::test]
    async fn mock短信发送并一次性验证且只存摘要() {
        let (service, sender, _) = service(AuthPolicy::default());
        service
            .send_sms_code(
                SmsSendRequest {
                    phone: phone(),
                    device_id: identity().device_id.clone(),
                },
                identity(),
            )
            .await
            .expect("send");
        let code = sender.last_code();
        let runtime = service.runtime.lock().expect("runtime");
        let challenge = runtime.challenges.values().next().expect("challenge");
        assert_ne!(challenge.digest.as_slice(), code.as_bytes());
        drop(runtime);
        let session = service
            .verify_sms_code(SmsVerifyRequest {
                phone: phone(),
                code,
                device_id: identity().device_id,
                agreements_accepted: true,
            })
            .expect("verify");
        assert!(!session.local_demo);
        assert!(session.refresh_token.is_some());
        let token_parts = session.session_token.split('.').collect::<Vec<_>>();
        assert_eq!(token_parts.len(), 3);
        let claims: Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(token_parts[1]).expect("JWT payload"))
                .expect("JWT claims");
        assert_eq!(
            claims.get("username").and_then(Value::as_str),
            Some(phone().as_str())
        );
        assert_eq!(
            claims.get("customerId").and_then(Value::as_str),
            Some("public")
        );
        assert!(service
            .runtime
            .lock()
            .expect("runtime")
            .challenges
            .is_empty());
    }

    #[tokio::test]
    async fn 重发间隔和每手机号频率限制生效() {
        let (service, _, clock) = service(AuthPolicy {
            phone_hourly_limit: 2,
            ..AuthPolicy::default()
        });
        let request = SmsSendRequest {
            phone: phone(),
            device_id: identity().device_id.clone(),
        };
        service
            .send_sms_code(request.clone(), identity())
            .await
            .expect("first");
        assert_eq!(
            service
                .send_sms_code(request.clone(), identity())
                .await
                .expect_err("resend")
                .code,
            "SMS_RESEND_TOO_SOON"
        );
        clock.set_epoch_seconds(clock.now_epoch_seconds() + 61);
        service
            .send_sms_code(request.clone(), identity())
            .await
            .expect("second");
        clock.set_epoch_seconds(clock.now_epoch_seconds() + 61);
        assert_eq!(
            service
                .send_sms_code(request, identity())
                .await
                .expect_err("rate")
                .code,
            "SMS_RATE_LIMITED"
        );
    }

    #[tokio::test]
    async fn ip和设备维度频率限制生效() {
        let ip_policy = AuthPolicy {
            resend_interval_seconds: 0,
            ip_hourly_limit: 1,
            ..AuthPolicy::default()
        };
        let (ip_service, _, _) = service(ip_policy);
        ip_service
            .send_sms_code(
                SmsSendRequest {
                    phone: phone(),
                    device_id: identity().device_id.clone(),
                },
                identity(),
            )
            .await
            .expect("first ip send");
        assert_eq!(
            ip_service
                .send_sms_code(
                    SmsSendRequest {
                        phone: ["138", "0000", "0000"].concat(),
                        device_id: "device_test_second_value".into(),
                    },
                    RequestIdentity {
                        ip: identity().ip,
                        device_id: "device_test_second_value".into()
                    },
                )
                .await
                .expect_err("ip rate")
                .code,
            "SMS_RATE_LIMITED"
        );

        let device_policy = AuthPolicy {
            resend_interval_seconds: 0,
            device_hourly_limit: 1,
            ..AuthPolicy::default()
        };
        let (device_service, _, _) = service(device_policy);
        device_service
            .send_sms_code(
                SmsSendRequest {
                    phone: phone(),
                    device_id: identity().device_id.clone(),
                },
                identity(),
            )
            .await
            .expect("first device send");
        assert_eq!(
            device_service
                .send_sms_code(
                    SmsSendRequest {
                        phone: ["138", "0000", "0000"].concat(),
                        device_id: identity().device_id.clone(),
                    },
                    RequestIdentity {
                        ip: "192.0.2.11".into(),
                        device_id: identity().device_id
                    },
                )
                .await
                .expect_err("device rate")
                .code,
            "SMS_RATE_LIMITED"
        );
    }

    #[tokio::test]
    async fn 验证码过期和最大失败次数生效() {
        let (service, sender, clock) = service(AuthPolicy {
            max_verify_attempts: 3,
            ..AuthPolicy::default()
        });
        let request = SmsSendRequest {
            phone: phone(),
            device_id: identity().device_id.clone(),
        };
        service
            .send_sms_code(request.clone(), identity())
            .await
            .expect("send");
        clock.set_epoch_seconds(clock.now_epoch_seconds() + 301);
        assert_eq!(
            service
                .verify_sms_code(SmsVerifyRequest {
                    phone: phone(),
                    code: sender.last_code(),
                    device_id: identity().device_id.clone(),
                    agreements_accepted: true,
                })
                .expect_err("expired")
                .code,
            "SMS_CODE_EXPIRED"
        );

        clock.set_epoch_seconds(clock.now_epoch_seconds() + 61);
        service
            .send_sms_code(request, identity())
            .await
            .expect("send again");
        let wrong_code = if sender.last_code() == "000000" {
            "111111"
        } else {
            "000000"
        };
        for _ in 0..3 {
            let _ = service.verify_sms_code(SmsVerifyRequest {
                phone: phone(),
                code: wrong_code.into(),
                device_id: identity().device_id.clone(),
                agreements_accepted: true,
            });
        }
        assert!(service
            .runtime
            .lock()
            .expect("runtime")
            .challenges
            .is_empty());
    }

    #[tokio::test]
    async fn mock微信交换且code只使用一次() {
        let clock = clock();
        let service = MiniappAuthService::for_tests(
            clock,
            Arc::new(MockSmsSender::default()),
            Arc::new(MockWechatCodeExchanger::default()),
            AuthPolicy::default(),
        );
        let request = WechatLoginRequest {
            code: "synthetic-login-code".into(),
            device_id: identity().device_id,
            agreements_accepted: true,
        };
        assert!(service.exchange_wechat_code(request.clone()).await.is_ok());
        assert_eq!(
            service
                .exchange_wechat_code(request)
                .await
                .expect_err("used")
                .code,
            "WECHAT_CODE_USED"
        );
    }

    #[test]
    fn 外部提供方均设置连接和总超时() {
        assert!(PROVIDER_CONNECT_TIMEOUT_SECONDS < PROVIDER_TOTAL_TIMEOUT_SECONDS);
        assert!(PROVIDER_TOTAL_TIMEOUT_SECONDS <= 15);
    }
}
