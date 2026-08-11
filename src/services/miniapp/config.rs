use std::{collections::BTreeMap, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateStatus {
    Set,
    NotSet,
    Invalid,
    RotationRequired,
}

impl fmt::Display for GateStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Set => "SET",
            Self::NotSet => "NOT_SET",
            Self::Invalid => "INVALID",
            Self::RotationRequired => "ROTATION_REQUIRED",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerEnvironment {
    Development,
    Test,
    Production,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigGateReport {
    pub miniapp_app_id: GateStatus,
    pub miniapp_app_secret: GateStatus,
    pub sms_config: GateStatus,
    pub bailian_config: GateStatus,
    pub token_secrets: GateStatus,
}

impl ConfigGateReport {
    pub fn status_lines(&self) -> Vec<String> {
        vec![
            format!("MINIAPP_APP_ID={}", self.miniapp_app_id),
            format!("MINIAPP_APP_SECRET={}", self.miniapp_app_secret),
            format!("SMS_CONFIG={}", self.sms_config),
            format!("BAILIAN_CONFIG={}", self.bailian_config),
            format!("TOKEN_SECRETS={}", self.token_secrets),
        ]
    }
}

#[derive(Clone)]
pub struct WechatProviderConfig {
    pub app_id: String,
    pub app_secret: String,
    pub exchange_url: String,
}

#[derive(Clone)]
pub struct SmsProviderConfig {
    pub merchant_id: String,
    pub app_id: String,
    pub sign_name: String,
    pub version: String,
    pub sign_type: String,
    pub message_type: String,
    pub secret_key: String,
    pub api_url: String,
    pub template_id: String,
    pub code_length: usize,
    pub code_ttl_seconds: u64,
    pub resend_interval_seconds: u64,
    pub max_verify_attempts: u32,
}

#[derive(Clone)]
pub struct TokenSecrets {
    pub access: String,
    pub refresh: String,
}

#[derive(Clone)]
pub struct MiniappServerConfig {
    pub environment: ServerEnvironment,
    pub sms_enabled: bool,
    pub wechat_enabled: bool,
    pub ai_provider: String,
    pub wechat: Option<WechatProviderConfig>,
    pub sms: Option<SmsProviderConfig>,
    pub bailian_api_key: Option<String>,
    pub token_secrets: Option<TokenSecrets>,
    pub gate: ConfigGateReport,
}

impl MiniappServerConfig {
    pub fn from_environment() -> Result<Self, String> {
        let values = std::env::vars().collect::<BTreeMap<_, _>>();
        Self::from_values(&values)
    }

    pub fn from_values(values: &BTreeMap<String, String>) -> Result<Self, String> {
        let environment = match value(values, "YIZU_ENV").as_deref() {
            Some("production") => ServerEnvironment::Production,
            Some("test") => ServerEnvironment::Test,
            Some("development") | None => ServerEnvironment::Development,
            Some(_) => return Err("YIZU_ENV=INVALID".into()),
        };
        let rotation_confirmed = bool_value(values, "YIZU_SECRETS_ROTATED");
        let sms_enabled = bool_value(values, "YIZU_MINIAPP_SMS_ENABLED");
        let wechat_enabled = bool_value(values, "YIZU_MINIAPP_WECHAT_ENABLED");
        let ai_provider =
            value(values, "YIZU_MINIAPP_AI_PROVIDER").unwrap_or_else(|| "local".into());
        if ai_provider != "local" && ai_provider != "bailian" {
            return Err("YIZU_MINIAPP_AI_PROVIDER=INVALID".into());
        }

        let app_id = value(values, "WECHAT_MINIPROGRAM_APP_ID");
        let app_secret = value(values, "WECHAT_MINIPROGRAM_APP_SECRET");
        let miniapp_app_id = match app_id.as_deref() {
            None => GateStatus::NotSet,
            Some(value) if valid_wechat_app_id(value) => GateStatus::Set,
            Some(_) => GateStatus::Invalid,
        };
        let miniapp_app_secret = secret_status(app_secret.as_deref(), rotation_confirmed, 16);
        let wechat_exchange_url = value(values, "WECHAT_CODE_EXCHANGE_URL")
            .unwrap_or_else(|| "https://api.weixin.qq.com/sns/jscode2session".into());
        let wechat = match (&app_id, &app_secret) {
            (Some(app_id), Some(app_secret))
                if miniapp_app_id == GateStatus::Set
                    && miniapp_app_secret == GateStatus::Set
                    && valid_https_endpoint(&wechat_exchange_url) =>
            {
                Some(WechatProviderConfig {
                    app_id: app_id.clone(),
                    app_secret: app_secret.clone(),
                    exchange_url: wechat_exchange_url,
                })
            }
            _ => None,
        };

        let sms_fields = [
            "SMS_MCH_ID",
            "SMS_APP_ID",
            "SMS_SIGN_NAME",
            "SMS_VERSION",
            "SMS_MESSAGE_TYPE",
            "SMS_SECRET_KEY",
            "SMS_API_URL",
            "SMS_TEMPLATE_ID",
        ];
        let sms_values = sms_fields
            .iter()
            .map(|name| value(values, name))
            .collect::<Vec<_>>();
        let sms_policy = SmsPolicyValues::parse(values);
        let sms_config = if sms_values.iter().all(Option::is_none) {
            GateStatus::NotSet
        } else if sms_values.iter().any(Option::is_none) || sms_policy.is_err() {
            GateStatus::Invalid
        } else if !rotation_confirmed {
            GateStatus::RotationRequired
        } else if sms_values[5]
            .as_deref()
            .is_none_or(|secret| !valid_secret(secret, 16))
            || sms_values[6]
                .as_deref()
                .is_none_or(|url| !valid_https_endpoint(url))
            || value(values, "SMS_SIGN_TYPE").is_some_and(|sign_type| sign_type != "HMACSHA256")
        {
            GateStatus::Invalid
        } else {
            GateStatus::Set
        };
        let sms = if sms_config == GateStatus::Set {
            let policy = sms_policy.expect("validated SMS policy");
            Some(SmsProviderConfig {
                merchant_id: sms_values[0].clone().expect("validated SMS_MCH_ID"),
                app_id: sms_values[1].clone().expect("validated SMS_APP_ID"),
                sign_name: sms_values[2].clone().expect("validated SMS_SIGN_NAME"),
                version: sms_values[3].clone().expect("validated SMS_VERSION"),
                sign_type: value(values, "SMS_SIGN_TYPE").unwrap_or_else(|| "HMACSHA256".into()),
                message_type: sms_values[4].clone().expect("validated SMS_MESSAGE_TYPE"),
                secret_key: sms_values[5].clone().expect("validated SMS_SECRET_KEY"),
                api_url: sms_values[6].clone().expect("validated SMS_API_URL"),
                template_id: sms_values[7].clone().expect("validated SMS_TEMPLATE_ID"),
                code_length: policy.code_length,
                code_ttl_seconds: policy.code_ttl_seconds,
                resend_interval_seconds: policy.resend_interval_seconds,
                max_verify_attempts: policy.max_verify_attempts,
            })
        } else {
            None
        };

        let (bailian_api_key, bailian_alias_conflict) =
            alias_value(values, "BAILIAN_API_KEY", "ALIYUN_BAILIAN_KEY");
        let bailian_config = if bailian_alias_conflict {
            GateStatus::Invalid
        } else {
            secret_status(bailian_api_key.as_deref(), rotation_confirmed, 20)
        };

        let access = value(values, "ACCESS_TOKEN_SECRET");
        let refresh = value(values, "REFRESH_TOKEN_SECRET");
        let token_secrets_status = match (&access, &refresh) {
            (None, None) => GateStatus::NotSet,
            (Some(_), None) | (None, Some(_)) => GateStatus::Invalid,
            (Some(access), Some(refresh)) if !rotation_confirmed => GateStatus::RotationRequired,
            (Some(access), Some(refresh))
                if valid_secret(access, 32) && valid_secret(refresh, 32) && access != refresh =>
            {
                GateStatus::Set
            }
            (Some(_), Some(_)) => GateStatus::Invalid,
        };
        let token_secrets = if token_secrets_status == GateStatus::Set {
            Some(TokenSecrets {
                access: access.expect("validated access secret"),
                refresh: refresh.expect("validated refresh secret"),
            })
        } else {
            None
        };

        let gate = ConfigGateReport {
            miniapp_app_id,
            miniapp_app_secret,
            sms_config,
            bailian_config,
            token_secrets: token_secrets_status,
        };
        let config = Self {
            environment,
            sms_enabled,
            wechat_enabled,
            ai_provider,
            wechat,
            sms,
            bailian_api_key,
            token_secrets,
            gate,
        };
        config.validate_enabled_providers()?;
        Ok(config)
    }

    fn validate_enabled_providers(&self) -> Result<(), String> {
        if self.environment != ServerEnvironment::Production {
            return Ok(());
        }
        if self.gate.token_secrets != GateStatus::Set {
            return Err(format!("TOKEN_SECRETS={}", self.gate.token_secrets));
        }
        if self.sms_enabled && self.gate.sms_config != GateStatus::Set {
            return Err(format!("SMS_CONFIG={}", self.gate.sms_config));
        }
        if self.wechat_enabled
            && (self.gate.miniapp_app_id != GateStatus::Set
                || self.gate.miniapp_app_secret != GateStatus::Set)
        {
            return Err(format!(
                "MINIAPP_APP_SECRET={}",
                self.gate.miniapp_app_secret
            ));
        }
        if self.wechat_enabled && self.wechat.is_none() {
            return Err("MINIAPP_APP_SECRET=INVALID".into());
        }
        if self.ai_provider == "bailian" && self.gate.bailian_config != GateStatus::Set {
            return Err(format!("BAILIAN_CONFIG={}", self.gate.bailian_config));
        }
        Ok(())
    }
}

struct SmsPolicyValues {
    code_length: usize,
    code_ttl_seconds: u64,
    resend_interval_seconds: u64,
    max_verify_attempts: u32,
}

impl SmsPolicyValues {
    fn parse(values: &BTreeMap<String, String>) -> Result<Self, ()> {
        let code_length = integer(values, "LOGIN_SMS_CODE_LENGTH", 6usize)?;
        let code_ttl_seconds = integer(values, "LOGIN_SMS_CODE_TTL", 300u64)?;
        let resend_interval_seconds = integer(values, "LOGIN_SMS_RESEND_INTERVAL", 60u64)?;
        let max_verify_attempts = integer(values, "LOGIN_SMS_MAX_VERIFY_ATTEMPTS", 5u32)?;
        if !(4..=8).contains(&code_length)
            || !(60..=900).contains(&code_ttl_seconds)
            || !(30..=600).contains(&resend_interval_seconds)
            || !(3..=10).contains(&max_verify_attempts)
        {
            return Err(());
        }
        Ok(Self {
            code_length,
            code_ttl_seconds,
            resend_interval_seconds,
            max_verify_attempts,
        })
    }
}

fn value(values: &BTreeMap<String, String>, name: &str) -> Option<String> {
    values
        .get(name)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn bool_value(values: &BTreeMap<String, String>, name: &str) -> bool {
    value(values, name).is_some_and(|value| value.eq_ignore_ascii_case("true"))
}

fn integer<T>(values: &BTreeMap<String, String>, name: &str, default: T) -> Result<T, ()>
where
    T: std::str::FromStr,
{
    match value(values, name) {
        Some(value) => value.parse().map_err(|_| ()),
        None => Ok(default),
    }
}

fn alias_value(
    values: &BTreeMap<String, String>,
    primary: &str,
    legacy: &str,
) -> (Option<String>, bool) {
    let primary = value(values, primary);
    let legacy = value(values, legacy);
    let conflict = matches!((&primary, &legacy), (Some(left), Some(right)) if left != right);
    (primary.or(legacy), conflict)
}

fn secret_status(value: Option<&str>, rotation_confirmed: bool, minimum: usize) -> GateStatus {
    match value {
        None => GateStatus::NotSet,
        Some(_) if !rotation_confirmed => GateStatus::RotationRequired,
        Some(value) if valid_secret(value, minimum) => GateStatus::Set,
        Some(_) => GateStatus::Invalid,
    }
}

fn valid_secret(value: &str, minimum: usize) -> bool {
    value.len() >= minimum
        && !value.starts_with('<')
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "secret" | "change-me" | "changeme" | "default" | "example"
        )
}

fn valid_wechat_app_id(value: &str) -> bool {
    value.len() == 18
        && value.starts_with("wx")
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_https_endpoint(value: &str) -> bool {
    value.starts_with("https://")
        && value.len() > "https://".len()
        && !value.bytes().any(|byte| byte.is_ascii_whitespace())
        && !value.contains('?')
        && !value.contains('#')
        && !value.contains('@')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| ((*key).into(), (*value).into()))
            .collect()
    }

    #[test]
    fn 缺少私密配置只返回状态() {
        let config = MiniappServerConfig::from_values(&values(&[])).expect("development config");
        assert_eq!(
            config.gate.status_lines(),
            [
                "MINIAPP_APP_ID=NOT_SET",
                "MINIAPP_APP_SECRET=NOT_SET",
                "SMS_CONFIG=NOT_SET",
                "BAILIAN_CONFIG=NOT_SET",
                "TOKEN_SECRETS=NOT_SET",
            ]
        );
    }

    #[test]
    fn 已填写但未确认轮换的秘密被拦截() {
        let config = MiniappServerConfig::from_values(&values(&[
            (
                "WECHAT_MINIPROGRAM_APP_SECRET",
                "a-previously-exposed-value",
            ),
            ("BAILIAN_API_KEY", "a-previously-exposed-bailian-value"),
        ]))
        .expect("development config");
        assert_eq!(config.gate.miniapp_app_secret, GateStatus::RotationRequired);
        assert_eq!(config.gate.bailian_config, GateStatus::RotationRequired);
    }

    #[test]
    fn 生产启用提供方时缺配置失败关闭() {
        let error = MiniappServerConfig::from_values(&values(&[
            ("YIZU_ENV", "production"),
            ("YIZU_MINIAPP_SMS_ENABLED", "true"),
        ]))
        .err()
        .expect("production must fail closed");
        assert!(error.contains("TOKEN_SECRETS=NOT_SET"));
    }

    #[test]
    fn access和refresh必须独立且足够长() {
        let same = "x".repeat(40);
        let config = MiniappServerConfig::from_values(&values(&[
            ("YIZU_SECRETS_ROTATED", "true"),
            ("ACCESS_TOKEN_SECRET", &same),
            ("REFRESH_TOKEN_SECRET", &same),
        ]))
        .expect("development config");
        assert_eq!(config.gate.token_secrets, GateStatus::Invalid);
    }

    #[test]
    fn 百炼新旧别名冲突被拒绝且不回显值() {
        let config = MiniappServerConfig::from_values(&values(&[
            ("YIZU_SECRETS_ROTATED", "true"),
            ("BAILIAN_API_KEY", "first-value-long-enough"),
            ("ALIYUN_BAILIAN_KEY", "second-value-long-enough"),
        ]))
        .expect("development config");
        assert_eq!(config.gate.bailian_config, GateStatus::Invalid);
    }

    #[test]
    fn 短信配置复用现有协议字段且只允许https() {
        let base = [
            ("YIZU_SECRETS_ROTATED", "true"),
            ("SMS_MCH_ID", "synthetic-merchant"),
            ("SMS_APP_ID", "synthetic-app"),
            ("SMS_SIGN_NAME", "synthetic-sign"),
            ("SMS_VERSION", "synthetic-version"),
            ("SMS_MESSAGE_TYPE", "synthetic-type"),
            ("SMS_SECRET_KEY", "synthetic-secret-material"),
            ("SMS_TEMPLATE_ID", "synthetic-template"),
        ];
        let mut secure = values(&base);
        secure.insert("SMS_API_URL".into(), "https://sms.invalid/send".into());
        let config = MiniappServerConfig::from_values(&secure).expect("secure SMS config");
        assert_eq!(config.gate.sms_config, GateStatus::Set);

        let mut insecure = values(&base);
        insecure.insert("SMS_API_URL".into(), "http://sms.invalid/send".into());
        let config = MiniappServerConfig::from_values(&insecure).expect("development config");
        assert_eq!(config.gate.sms_config, GateStatus::Invalid);
    }

    #[test]
    fn 微信交换端点不允许查询串或非https() {
        for endpoint in [
            "http://wechat.invalid/exchange",
            "https://wechat.invalid/exchange?secret=forbidden",
        ] {
            let config = MiniappServerConfig::from_values(&values(&[
                ("YIZU_SECRETS_ROTATED", "true"),
                ("WECHAT_MINIPROGRAM_APP_ID", "wx1234567890abcdef"),
                ("WECHAT_MINIPROGRAM_APP_SECRET", "synthetic-wechat-secret"),
                ("WECHAT_CODE_EXCHANGE_URL", endpoint),
            ]))
            .expect("development config");
            assert!(config.wechat.is_none());
        }
    }
}
