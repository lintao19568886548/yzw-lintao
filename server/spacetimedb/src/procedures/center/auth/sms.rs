//! Module 内部的短信发送与验证码登录流程。

use std::time::Duration;

use spacetimedb::{Identity, ProcedureContext, SpacetimeType, Table};

use crate::{
    reducers::platform::center::auth::{legacy_sms::establish_phone_session, sms::LOGIN_SMS_CONFIG_KEY},
    sms::{generate_verification_code, hash_verification_code, send_verification_code},
    tables::{SmsLoginChallenge, sms_login_challenge, sms_provider_config},
};

#[derive(SpacetimeType)]
pub struct SmsSendResult {
    pub success: bool,
    pub message: String,
    pub expires_in: u32,
    pub retry_after: u32,
}

#[derive(SpacetimeType)]
pub struct SmsLoginResult {
    pub success: bool,
    pub message: String,
}

enum LoginOutcome {
    Success,
    Rejected(String),
}

/// 直接从 SpacetimeDB Module 调用联麓短信接口。
#[spacetimedb::procedure]
pub fn send_login_sms_code(ctx: &mut ProcedureContext, phone_number: String) -> SmsSendResult {
    let phone_number = phone_number.trim().to_string();
    if !valid_phone(&phone_number) {
        return send_failure("请输入11位手机号码", 0);
    }

    let sender = ctx.sender();
    let timestamp = ctx.timestamp;
    let nonce: u64 = ctx.random();
    let reserved = ctx.try_with_tx(|tx| -> Result<_, String> {
        let config = tx
            .db
            .sms_provider_config()
            .config_key()
            .find(LOGIN_SMS_CONFIG_KEY.to_string())
            .ok_or("短信服务尚未配置")?;
        if let Some(existing) = tx
            .db
            .sms_login_challenge()
            .phone_number()
            .find(&phone_number)
        {
            let resend_at =
                existing.sent_at + Duration::from_secs(u64::from(config.resend_interval_seconds));
            if timestamp < resend_at {
                let remaining = resend_at
                    .duration_since(timestamp)
                    .map(|duration| duration.as_secs().max(1) as u32)
                    .unwrap_or(1);
                return Err(format!("验证码发送过于频繁，请{remaining}秒后再试"));
            }
        }
        let code = generate_verification_code(
            &config.code_pepper,
            timestamp.to_micros_since_unix_epoch(),
            sender,
            nonce,
        );
        let challenge = SmsLoginChallenge {
            phone_number: phone_number.clone(),
            requested_by: sender,
            code_hash: hash_verification_code(&config.code_pepper, &phone_number, &code),
            sending: true,
            attempts: 0,
            sent_at: timestamp,
            expires_at: timestamp + Duration::from_secs(30),
        };
        if tx
            .db
            .sms_login_challenge()
            .phone_number()
            .find(&phone_number)
            .is_some()
        {
            tx.db.sms_login_challenge().phone_number().update(challenge);
        } else {
            tx.db.sms_login_challenge().insert(challenge);
        }
        Ok((config, code))
    });
    let (config, code) = match reserved {
        Ok(value) => value,
        Err(message) => {
            let retry_after = retry_seconds(&message);
            return send_failure(message, retry_after);
        }
    };

    if let Err(error) = send_verification_code(ctx, &config, &phone_number, &code) {
        clear_reserved_challenge(ctx, &phone_number, sender, timestamp);
        log::error!(
            "联麓短信发送失败，手机号={}****{}：{error}",
            &phone_number[..3],
            &phone_number[7..]
        );
        return send_failure("验证码发送失败，请稍后重试", 0);
    }

    let expires_in = config.code_ttl_seconds;
    ctx.with_tx(|tx| {
        if let Some(mut challenge) = tx
            .db
            .sms_login_challenge()
            .phone_number()
            .find(&phone_number)
            && challenge.requested_by == sender
            && challenge.sent_at == timestamp
        {
            challenge.sending = false;
            challenge.expires_at = timestamp + Duration::from_secs(u64::from(expires_in));
            tx.db.sms_login_challenge().phone_number().update(challenge);
        }
    });
    SmsSendResult {
        success: true,
        message: "验证码发送成功".into(),
        expires_in,
        retry_after: 0,
    }
}

/// 校验私有表中的验证码，并在同一事务中建立用户会话。
#[spacetimedb::procedure]
pub fn login_with_sms_code(
    ctx: &mut ProcedureContext,
    phone_number: String,
    code: String,
    remember_me: bool,
) -> SmsLoginResult {
    let phone_number = phone_number.trim().to_string();
    let code = code.trim().to_string();
    if !valid_phone(&phone_number) || code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return login_failure("手机号或验证码格式不正确");
    }

    let sender = ctx.sender();
    let timestamp = ctx.timestamp;
    let outcome = ctx.try_with_tx(|tx| -> Result<LoginOutcome, String> {
        let config = tx
            .db
            .sms_provider_config()
            .config_key()
            .find(LOGIN_SMS_CONFIG_KEY.to_string())
            .ok_or("短信服务尚未配置")?;
        let Some(mut challenge) = tx
            .db
            .sms_login_challenge()
            .phone_number()
            .find(&phone_number)
        else {
            return Ok(LoginOutcome::Rejected("验证码不存在或已失效".into()));
        };
        if challenge.requested_by != sender || challenge.sending {
            return Ok(LoginOutcome::Rejected("验证码不存在或已失效".into()));
        }
        if challenge.expires_at <= timestamp {
            tx.db
                .sms_login_challenge()
                .phone_number()
                .delete(&phone_number);
            return Ok(LoginOutcome::Rejected("验证码已经过期".into()));
        }
        let submitted_hash = hash_verification_code(&config.code_pepper, &phone_number, &code);
        if submitted_hash != challenge.code_hash {
            challenge.attempts += 1;
            if challenge.attempts >= config.max_verify_attempts {
                tx.db
                    .sms_login_challenge()
                    .phone_number()
                    .delete(&phone_number);
            } else {
                tx.db.sms_login_challenge().phone_number().update(challenge);
            }
            return Ok(LoginOutcome::Rejected("验证码不正确".into()));
        }

        tx.db
            .sms_login_challenge()
            .phone_number()
            .delete(&phone_number);
        establish_phone_session(tx, &phone_number, remember_me)?;
        Ok(LoginOutcome::Success)
    });
    match outcome {
        Ok(LoginOutcome::Success) => SmsLoginResult {
            success: true,
            message: "登录成功".into(),
        },
        Ok(LoginOutcome::Rejected(message)) | Err(message) => login_failure(message),
    }
}

fn clear_reserved_challenge(
    ctx: &mut ProcedureContext,
    phone_number: &str,
    sender: Identity,
    sent_at: spacetimedb::Timestamp,
) {
    ctx.with_tx(|tx| {
        if let Some(challenge) = tx
            .db
            .sms_login_challenge()
            .phone_number()
            .find(phone_number.to_string())
            && challenge.requested_by == sender
            && challenge.sent_at == sent_at
        {
            tx.db
                .sms_login_challenge()
                .phone_number()
                .delete(phone_number.to_string());
        }
    });
}

fn valid_phone(phone_number: &str) -> bool {
    phone_number.len() == 11 && phone_number.bytes().all(|byte| byte.is_ascii_digit())
}

fn retry_seconds(message: &str) -> u32 {
    message
        .split('请')
        .nth(1)
        .and_then(|part| part.split('秒').next())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0)
}

fn send_failure(message: impl Into<String>, retry_after: u32) -> SmsSendResult {
    SmsSendResult {
        success: false,
        message: message.into(),
        expires_in: 0,
        retry_after,
    }
}

fn login_failure(message: impl Into<String>) -> SmsLoginResult {
    SmsLoginResult {
        success: false,
        message: message.into(),
    }
}
