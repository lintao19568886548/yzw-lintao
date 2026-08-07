//! 短信供应商私有配置管理。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::AdminContext,
        validation::{required_text, validate_max_length},
    },
    tables::{SmsProviderConfig, sms_provider_config},
};

pub(crate) const LOGIN_SMS_CONFIG_KEY: &str = "login";

/// 复用登录短信供应商凭据，配置合同到期提醒模板。
#[spacetimedb::reducer]
pub fn upsert_contract_reminder_sms_template(
    ctx: &ReducerContext,
    template_id: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let template_id = required_text(template_id, "合同提醒短信模板编号不能为空")?;
    validate_max_length(&template_id, 120, "合同提醒短信模板编号过长")?;
    let mut config = ctx
        .db
        .sms_provider_config()
        .config_key()
        .find(LOGIN_SMS_CONFIG_KEY.to_string())
        .ok_or("请先配置登录短信供应商")?;
    config.config_key = "contract:expiry".into();
    config.template_id = template_id;
    config.updated_at = ctx.timestamp;
    if ctx
        .db
        .sms_provider_config()
        .config_key()
        .find(config.config_key.clone())
        .is_some()
    {
        ctx.db.sms_provider_config().config_key().update(config);
    } else {
        ctx.db.sms_provider_config().insert(config);
    }
    Ok(())
}

/// 复用登录短信供应商凭据，为企业主体配置一类催收模板。
#[spacetimedb::reducer]
pub fn upsert_collection_sms_template(
    ctx: &ReducerContext,
    company_name: String,
    collection_type: String,
    template_id: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let company_name = required_text(company_name, "短信企业主体不能为空")?;
    let collection_type = required_text(collection_type, "催收类型不能为空")?;
    if !matches!(
        collection_type.as_str(),
        "payment_reminder" | "overdue_10" | "final_30"
    ) {
        return Err("催收类型无效".into());
    }
    let template_id = required_text(template_id, "催收短信模板编号不能为空")?;
    validate_max_length(&company_name, 120, "短信企业主体过长")?;
    validate_max_length(&template_id, 120, "催收短信模板编号过长")?;
    let mut config = ctx
        .db
        .sms_provider_config()
        .config_key()
        .find(LOGIN_SMS_CONFIG_KEY.to_string())
        .ok_or("请先配置登录短信供应商")?;
    config.config_key = crate::procedures::billing::collection::collection_config_key(
        &company_name,
        &collection_type,
    );
    config.template_id = template_id;
    config.updated_at = ctx.timestamp;
    if ctx
        .db
        .sms_provider_config()
        .config_key()
        .find(config.config_key.clone())
        .is_some()
    {
        ctx.db.sms_provider_config().config_key().update(config);
    } else {
        ctx.db.sms_provider_config().insert(config);
    }
    Ok(())
}

/// 管理员配置 Module 直接调用的联麓短信参数。
#[spacetimedb::reducer]
#[allow(clippy::too_many_arguments)]
pub fn upsert_sms_provider_config(
    ctx: &ReducerContext,
    api_host: String,
    app_id: String,
    merchant_id: String,
    version: String,
    sign_type: String,
    secret_key: String,
    template_id: String,
    message_type: String,
    code_pepper: String,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let api_host = required_text(api_host, "短信接口地址不能为空")?;
    if !api_host.starts_with("https://") {
        return Err("短信接口必须使用 HTTPS".into());
    }
    let app_id = required_text(app_id, "短信 AppId 不能为空")?;
    let merchant_id = required_text(merchant_id, "短信商户号不能为空")?;
    let version = required_text(version, "短信接口版本不能为空")?;
    let sign_type = required_text(sign_type, "短信签名算法不能为空")?.to_uppercase();
    if sign_type != "MD5" && sign_type != "HMACSHA256" {
        return Err("短信签名算法只支持 MD5 或 HMACSHA256".into());
    }
    let secret_key = required_text(secret_key, "短信平台密钥不能为空")?;
    let template_id = required_text(template_id, "短信模板编号不能为空")?;
    let message_type = required_text(message_type, "短信类型不能为空")?;
    let code_pepper = required_text(code_pepper, "验证码私有密钥不能为空")?;
    if code_pepper.len() < 32 {
        return Err("验证码私有密钥至少需要 32 个字符".into());
    }
    validate_max_length(&api_host, 500, "短信接口地址过长")?;
    validate_max_length(&secret_key, 512, "短信平台密钥过长")?;
    validate_max_length(&code_pepper, 512, "验证码私有密钥过长")?;

    let config = SmsProviderConfig {
        config_key: LOGIN_SMS_CONFIG_KEY.into(),
        api_host,
        app_id,
        merchant_id,
        version,
        sign_type,
        secret_key,
        template_id,
        message_type,
        code_pepper,
        code_ttl_seconds: 300,
        resend_interval_seconds: 60,
        max_verify_attempts: 5,
        updated_at: ctx.timestamp,
    };
    if ctx
        .db
        .sms_provider_config()
        .config_key()
        .find(LOGIN_SMS_CONFIG_KEY.to_string())
        .is_some()
    {
        ctx.db.sms_provider_config().config_key().update(config);
    } else {
        ctx.db.sms_provider_config().insert(config);
    }
    Ok(())
}
