use dioxus::prelude::*;
use std::collections::BTreeMap;

use super::{
    matching::{hard_condition_violations, match_listings},
    repository::{FixtureListingRepository, InMemoryLeadRepository, LeadRepository},
    types::{
        AdvisorAssignmentStatus, ApiFieldError, ApiResponse, DevSessionRequest, DevSessionResponse,
        InterpretDemandRequest, LeadRecord, MatchRequest, MatchResponse, MetadataOptions,
        SubmitLeadRequest,
    },
    validation::{
        mask_phone, validate_demand, validate_idempotency_key, validate_phone,
        validate_request_size, DONGGUAN_TOWNS,
    },
};

#[derive(Clone, Debug)]
struct DemoSession {
    phone: String,
    contact_confirmed: bool,
    expires_at_epoch_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceError {
    pub code: String,
    pub message: String,
    pub errors: Vec<ApiFieldError>,
}

impl ServiceError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            errors: Vec::new(),
        }
    }

    fn validation(errors: Vec<ApiFieldError>) -> Self {
        Self {
            code: "VALIDATION_ERROR".into(),
            message: "请求字段校验失败".into(),
            errors,
        }
    }
}

#[derive(Default)]
pub struct MiniappRuntime {
    sessions: BTreeMap<String, DemoSession>,
    leads: InMemoryLeadRepository,
    session_sequence: u64,
    lead_sequence: u64,
}

impl MiniappRuntime {
    pub fn create_dev_session_at(
        &mut self,
        request: DevSessionRequest,
        now: u64,
    ) -> Result<DevSessionResponse, ServiceError> {
        if !validate_phone(&request.phone) {
            return Err(ServiceError::validation(vec![ApiFieldError {
                field: "phone".into(),
                message: "请输入有效的11位中国大陆手机号".into(),
            }]));
        }
        if !request.contact_confirmed {
            return Err(ServiceError::validation(vec![ApiFieldError {
                field: "contact_confirmed".into(),
                message: "请确认该手机号可用于顾问联系".into(),
            }]));
        }
        self.session_sequence += 1;
        let session_token = session_token(&request.phone, now, self.session_sequence);
        let expires = now + 2 * 60 * 60;
        self.sessions.insert(
            session_token.clone(),
            DemoSession {
                phone: request.phone.clone(),
                contact_confirmed: request.contact_confirmed,
                expires_at_epoch_seconds: expires,
            },
        );
        Ok(DevSessionResponse {
            session_token,
            masked_phone: mask_phone(&request.phone),
            expires_at_epoch_seconds: expires,
            local_demo: true,
        })
    }

    pub fn ensure_authenticated(&self, token: &str, now: u64) -> Result<(), ServiceError> {
        self.session(token, now).map(|_| ())
    }

    pub fn submit_lead_at(
        &mut self,
        request: SubmitLeadRequest,
        now: u64,
    ) -> Result<LeadRecord, ServiceError> {
        let session = self.session(&request.session_token, now)?.clone();
        if !session.contact_confirmed {
            return Err(ServiceError::new(
                "CONTACT_NOT_CONFIRMED",
                "联系方式尚未确认，不能生成招商线索",
            ));
        }
        let errors = validate_demand(&request.submission.demand, true);
        if !errors.is_empty() {
            return Err(ServiceError::validation(errors));
        }
        validate_idempotency_key(&request.submission.idempotency_key)
            .map_err(|error| ServiceError::validation(vec![error]))?;
        if request.submission.recommended_listing_ids.is_empty()
            || request.submission.recommended_listing_ids.len() > 10
        {
            return Err(ServiceError::validation(vec![ApiFieldError {
                field: "submission.recommended_listing_ids".into(),
                message: "必须选择1到10套本次推荐房源".into(),
            }]));
        }
        if request.submission.source_channel != "miniapp_ai_demand" {
            return Err(ServiceError::validation(vec![ApiFieldError {
                field: "submission.source_channel".into(),
                message: "线索来源渠道无效".into(),
            }]));
        }

        let identity_key = format!("{}:{}", session.phone, request.submission.idempotency_key);
        if let Some(existing) = self.leads.find_by_idempotency_key(&identity_key) {
            return Ok(existing);
        }

        let matches = match_listings(
            &request.submission.demand,
            &FixtureListingRepository::default(),
        );
        for selected in &request.submission.recommended_listing_ids {
            let Some(result) = matches
                .matches
                .iter()
                .find(|item| &item.listing.listing_id == selected)
            else {
                return Err(ServiceError::new(
                    "LISTING_NOT_RECOMMENDED",
                    "提交的房源不在本次服务端推荐结果中",
                ));
            };
            let violations = hard_condition_violations(&request.submission.demand, result);
            if !violations.is_empty() {
                return Err(ServiceError::new(
                    "HARD_CONDITION_NOT_MET",
                    format!("所选房源未满足硬条件：{}", violations.join("、")),
                ));
            }
        }

        self.lead_sequence += 1;
        let record = LeadRecord {
            demand_number: format!("DEMO-D-{:06}", self.lead_sequence),
            lead_number: format!("DEMO-L-{:06}", self.lead_sequence),
            demand_snapshot: request.submission.demand,
            recommended_listing_ids: request.submission.recommended_listing_ids,
            source_channel: request.submission.source_channel,
            status: AdvisorAssignmentStatus::PendingAssignment,
            sla_minutes: 15,
            created_at_epoch_seconds: now,
            temporary_storage: true,
        };
        Ok(self.leads.insert(identity_key, record))
    }

    fn session(&self, token: &str, now: u64) -> Result<&DemoSession, ServiceError> {
        let session = self
            .sessions
            .get(token)
            .ok_or_else(|| ServiceError::new("UNAUTHENTICATED", "登录状态不存在，请重新登录"))?;
        if session.expires_at_epoch_seconds <= now {
            return Err(ServiceError::new(
                "SESSION_EXPIRED",
                "登录已过期，请重新登录",
            ));
        }
        Ok(session)
    }

    #[cfg(test)]
    fn insert_unconfirmed_session(&mut self, token: &str, phone: &str, expires: u64) {
        self.sessions.insert(
            token.into(),
            DemoSession {
                phone: phone.into(),
                contact_confirmed: false,
                expires_at_epoch_seconds: expires,
            },
        );
    }
}

fn session_token(phone: &str, now: u64, sequence: u64) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"yizu-miniapp-local-demo-session");
    hasher.update(phone.as_bytes());
    hasher.update(now.to_le_bytes());
    hasher.update(sequence.to_le_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("miniapp-dev-{}", &digest[..32])
}

fn metadata() -> MetadataOptions {
    MetadataOptions {
        towns: DONGGUAN_TOWNS.into_iter().map(str::to_string).collect(),
        space_types: vec!["factory".into(), "warehouse".into(), "office".into()],
        rent_units: vec![
            "yuan_per_month".into(),
            "yuan_per_square_metre_month".into(),
        ],
        verification_levels: vec!["l0".into(), "l1".into(), "l2".into(), "l3".into()],
        demo_data: true,
    }
}

#[cfg(feature = "server")]
fn runtime() -> &'static std::sync::Mutex<MiniappRuntime> {
    static RUNTIME: std::sync::OnceLock<std::sync::Mutex<MiniappRuntime>> =
        std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| std::sync::Mutex::new(MiniappRuntime::default()))
}

#[cfg(feature = "server")]
fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(feature = "server")]
fn request_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQUENCE: AtomicU64 = AtomicU64::new(1);
    format!(
        "miniapp-{:x}-{:x}",
        now_seconds(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(feature = "server")]
fn failure<T>(request_id: String, error: ServiceError) -> ApiResponse<T> {
    ApiResponse::failure(request_id, error.code, error.message, error.errors)
}

#[cfg(feature = "server")]
fn dev_auth_enabled() -> bool {
    cfg!(debug_assertions)
        && std::env::var("YIZU_MINIAPP_DEV_AUTH_ENABLED")
            .is_ok_and(|value| value.eq_ignore_ascii_case("true"))
}

#[cfg(feature = "server")]
fn local_fallback_enabled() -> bool {
    cfg!(debug_assertions)
        && std::env::var("YIZU_MINIAPP_BAILIAN_ALLOW_LOCAL_FALLBACK")
            .is_ok_and(|value| value.eq_ignore_ascii_case("true"))
}

fn validate_ai_provider(provider: &str) -> Result<&str, ServiceError> {
    match provider {
        "local" | "bailian" => Ok(provider),
        _ => Err(ServiceError::new(
            "AI_PROVIDER_INVALID",
            "YIZU_MINIAPP_AI_PROVIDER 只能是 local 或 bailian",
        )),
    }
}

#[post("/api/miniapp/v1/auth/dev-session")]
async fn create_dev_session(
    request: DevSessionRequest,
) -> Result<ApiResponse<DevSessionResponse>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        dotenvy::dotenv().ok();
        let request_id = request_id();
        if !dev_auth_enabled() {
            return Ok(ApiResponse::failure(
                request_id,
                "DEV_AUTH_DISABLED",
                "本地模拟登录未启用；release构建始终禁用该能力",
                Vec::new(),
            ));
        }
        if let Err(error) = validate_request_size(&request) {
            return Ok(ApiResponse::failure(
                request_id,
                "REQUEST_TOO_LARGE",
                error.message.clone(),
                vec![error],
            ));
        }
        let result = runtime()
            .lock()
            .expect("miniapp runtime lock")
            .create_dev_session_at(request, now_seconds());
        return Ok(match result {
            Ok(data) => ApiResponse::success(request_id, "本地演示会话已创建", data),
            Err(error) => failure(request_id, error),
        });
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = request;
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[post("/api/miniapp/v1/demands/interpret")]
async fn interpret_demand(
    request: InterpretDemandRequest,
) -> Result<ApiResponse<super::types::DemandInterpretation>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use super::interpreter::{DemandInterpreter, LocalDemandInterpreter};
        let request_id = request_id();
        if let Err(error) = validate_request_size(&request) {
            return Ok(ApiResponse::failure(
                request_id,
                "REQUEST_TOO_LARGE",
                error.message.clone(),
                vec![error],
            ));
        }
        if let Err(error) = runtime()
            .lock()
            .expect("miniapp runtime lock")
            .ensure_authenticated(&request.session_token, now_seconds())
        {
            return Ok(failure(request_id, error));
        }
        let initial_errors = validate_demand(&request.draft, false);
        if !initial_errors.is_empty() {
            return Ok(failure(
                request_id,
                ServiceError::validation(initial_errors),
            ));
        }
        dotenvy::dotenv().ok();
        let provider = std::env::var("YIZU_MINIAPP_AI_PROVIDER").unwrap_or_else(|_| "local".into());
        let provider = match validate_ai_provider(&provider) {
            Ok(provider) => provider,
            Err(error) => return Ok(failure(request_id, error)),
        };
        let result = match provider {
            "local" => LocalDemandInterpreter.interpret(request.draft).await,
            "bailian" => {
                use super::interpreter::bailian::{BailianConfig, BailianDemandInterpreter};
                match BailianConfig::from_env().and_then(BailianDemandInterpreter::new) {
                    Ok(interpreter) => interpreter.interpret(request.draft.clone()).await,
                    Err(configuration_error) if local_fallback_enabled() => LocalDemandInterpreter
                        .interpret(request.draft)
                        .await
                        .map(|mut output| {
                            output.fallback_reason = Some(configuration_error);
                            output
                        }),
                    Err(configuration_error) => Err(configuration_error),
                }
            }
            _ => unreachable!("provider was validated above"),
        };
        return Ok(match result {
            Ok(data) => ApiResponse::success(request_id, "需求已结构化，请确认并补充", data),
            Err(message) => {
                ApiResponse::failure(request_id, "AI_INTERPRETATION_ERROR", message, Vec::new())
            }
        });
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = request;
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[post("/api/miniapp/v1/matches")]
async fn create_matches(
    request: MatchRequest,
) -> Result<ApiResponse<MatchResponse>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let request_id = request_id();
        if let Err(error) = validate_request_size(&request) {
            return Ok(ApiResponse::failure(
                request_id,
                "REQUEST_TOO_LARGE",
                error.message.clone(),
                vec![error],
            ));
        }
        if let Err(error) = runtime()
            .lock()
            .expect("miniapp runtime lock")
            .ensure_authenticated(&request.session_token, now_seconds())
        {
            return Ok(failure(request_id, error));
        }
        let errors = validate_demand(&request.demand, true);
        if !errors.is_empty() {
            return Ok(failure(request_id, ServiceError::validation(errors)));
        }
        let data = match_listings(&request.demand, &FixtureListingRepository::default());
        return Ok(ApiResponse::success(
            request_id,
            "匹配完成，结果仅为脱敏演示数据",
            data,
        ));
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = request;
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[post("/api/miniapp/v1/leads")]
async fn submit_lead(request: SubmitLeadRequest) -> Result<ApiResponse<LeadRecord>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let request_id = request_id();
        if let Err(error) = validate_request_size(&request) {
            return Ok(ApiResponse::failure(
                request_id,
                "REQUEST_TOO_LARGE",
                error.message.clone(),
                vec![error],
            ));
        }
        let result = runtime()
            .lock()
            .expect("miniapp runtime lock")
            .submit_lead_at(request, now_seconds());
        return Ok(match result {
            Ok(data) => {
                ApiResponse::success(request_id, "本地演示线索已生成；未通知真实顾问", data)
            }
            Err(error) => failure(request_id, error),
        });
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = request;
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[get("/api/miniapp/v1/metadata/options")]
async fn metadata_options() -> Result<ApiResponse<MetadataOptions>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        return Ok(ApiResponse::success(
            request_id(),
            "选项加载成功",
            metadata(),
        ));
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::miniapp::types::{
        DemandConstraints, DemandDraft, LeadSubmission, SpaceType,
    };

    fn complete_demand() -> DemandDraft {
        DemandDraft {
            raw_text: "松山湖1500平方米厂房".into(),
            constraints: DemandConstraints {
                space_type: Some(SpaceType::Factory),
                target_towns: vec!["松山湖".into()],
                area_min_sqm: Some(1400),
                area_max_sqm: Some(1650),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn lead_request(token: &str, key: &str) -> SubmitLeadRequest {
        SubmitLeadRequest {
            session_token: token.into(),
            submission: LeadSubmission {
                demand: complete_demand(),
                recommended_listing_ids: vec!["demo-factory-001".into()],
                source_channel: "miniapp_ai_demand".into(),
                idempotency_key: key.into(),
            },
        }
    }

    fn synthetic_phone() -> String {
        ["139", "0000", "0000"].concat()
    }

    #[test]
    fn 非法ai_provider返回专用错误码() {
        let error = validate_ai_provider("unknown").unwrap_err();
        assert_eq!(error.code, "AI_PROVIDER_INVALID");
    }

    #[test]
    fn 未登录拒绝生成线索() {
        let mut runtime = MiniappRuntime::default();
        let error = runtime
            .submit_lead_at(lead_request("missing", "idempotency-key-001"), 100)
            .unwrap_err();
        assert_eq!(error.code, "UNAUTHENTICATED");
    }

    #[test]
    fn 联系方式未确认拒绝生成线索() {
        let mut runtime = MiniappRuntime::default();
        runtime.insert_unconfirmed_session("token", &synthetic_phone(), 1000);
        let error = runtime
            .submit_lead_at(lead_request("token", "idempotency-key-002"), 100)
            .unwrap_err();
        assert_eq!(error.code, "CONTACT_NOT_CONFIRMED");
    }

    #[test]
    fn 相同幂等键只生成一条线索() {
        let mut runtime = MiniappRuntime::default();
        let session = runtime
            .create_dev_session_at(
                DevSessionRequest {
                    phone: synthetic_phone(),
                    contact_confirmed: true,
                },
                100,
            )
            .expect("session");
        let first = runtime
            .submit_lead_at(
                lead_request(&session.session_token, "idempotency-key-003"),
                200,
            )
            .expect("first");
        let second = runtime
            .submit_lead_at(
                lead_request(&session.session_token, "idempotency-key-003"),
                201,
            )
            .expect("second");
        assert_eq!(first.lead_number, second.lead_number);
        assert_eq!(
            first.created_at_epoch_seconds,
            second.created_at_epoch_seconds
        );
    }

    #[test]
    fn 过期会话被拒绝() {
        let mut runtime = MiniappRuntime::default();
        let session = runtime
            .create_dev_session_at(
                DevSessionRequest {
                    phone: synthetic_phone(),
                    contact_confirmed: true,
                },
                100,
            )
            .expect("session");
        let error = runtime
            .ensure_authenticated(&session.session_token, 100 + 7200)
            .unwrap_err();
        assert_eq!(error.code, "SESSION_EXPIRED");
    }
}
