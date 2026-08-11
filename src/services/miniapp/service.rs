use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use super::{
    auth::{MiniappAuthService, RequestIdentity},
    clock::{Clock, SystemClock, BUSINESS_TIMEZONE},
    interpreter::{DemandInterpreter, LocalDemandInterpreter},
    matching::match_listings_with_clock,
    repository::{FixtureListingRepository, InMemoryLeadRepository, LeadRepository},
    types::{
        AdvisorAssignmentStatus, ApiFieldError, ApiResponse, DevSessionRequest, DevSessionResponse,
        InterpretDemandRequest, LeadRecord, MatchRequest, MatchResponse, MetadataOptions,
        SmsSendRequest, SmsSendResponse, SmsVerifyRequest, SubmitLeadRequest, WechatLoginRequest,
    },
    validation::{
        mask_phone, normalize_constraint_priorities, validate_demand, validate_idempotency_key,
        validate_phone, validate_request_size, DONGGUAN_TOWNS,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InterpreterMode {
    Environment,
    LocalOnly,
}

#[derive(Clone, Copy, Debug)]
pub struct ServiceConfig {
    dev_auth_enabled: bool,
    interpreter_mode: InterpreterMode,
}

impl ServiceConfig {
    pub fn from_environment() -> Self {
        let explicit_auth = std::env::var("YIZU_MINIAPP_DEV_AUTH_ENABLED")
            .is_ok_and(|value| value.eq_ignore_ascii_case("true"));
        Self {
            dev_auth_enabled: dev_auth_allowed(cfg!(debug_assertions), explicit_auth),
            interpreter_mode: InterpreterMode::Environment,
        }
    }

    #[cfg(test)]
    pub fn isolated_local(explicit_auth: bool) -> Self {
        Self {
            dev_auth_enabled: dev_auth_allowed(cfg!(debug_assertions), explicit_auth),
            interpreter_mode: InterpreterMode::LocalOnly,
        }
    }
}

pub fn dev_auth_allowed(debug_build: bool, explicit_enabled: bool) -> bool {
    debug_build && explicit_enabled
}

#[derive(Default)]
struct MiniappRuntime {
    sessions: BTreeMap<String, DemoSession>,
    leads: InMemoryLeadRepository,
    session_sequence: u64,
    lead_sequence: u64,
}

impl MiniappRuntime {
    fn create_dev_session_at(
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
            refresh_token: None,
            masked_phone: mask_phone(&request.phone),
            expires_at_epoch_seconds: expires,
            local_demo: true,
        })
    }

    fn insert_authenticated_session(
        &mut self,
        token: String,
        identity: String,
        expires_at_epoch_seconds: u64,
        contact_confirmed: bool,
    ) {
        self.sessions.insert(
            token,
            DemoSession {
                phone: identity,
                contact_confirmed,
                expires_at_epoch_seconds,
            },
        );
    }

    fn ensure_authenticated(&self, token: &str, now: u64) -> Result<(), ServiceError> {
        self.session(token, now).map(|_| ())
    }

    fn submit_lead_at(
        &mut self,
        mut request: SubmitLeadRequest,
        now: u64,
        repository: &FixtureListingRepository,
        clock: &dyn Clock,
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
        normalize_constraint_priorities(&mut request.submission.demand);
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

        let matches = match_listings_with_clock(&request.submission.demand, repository, clock);
        for selected in &request.submission.recommended_listing_ids {
            let Some(result) = matches
                .matches
                .iter()
                .find(|item| &item.listing.listing_id == selected)
            else {
                return Err(ServiceError::new(
                    "LISTING_NOT_RECOMMENDED",
                    "提交的房源不在本次服务端重新计算的推荐结果中",
                ));
            };
            if !result.unmet_hard_constraints.is_empty() {
                return Err(ServiceError::new(
                    "HARD_CONDITION_NOT_MET",
                    format!(
                        "所选房源未满足硬条件：{}",
                        result
                            .unmet_hard_constraints
                            .iter()
                            .map(|item| item.detail.as_str())
                            .collect::<Vec<_>>()
                            .join("、")
                    ),
                ));
            }
            if !result.unverified_hard_constraints.is_empty() {
                return Err(ServiceError::new(
                    "HARD_CONDITION_UNVERIFIED",
                    format!(
                        "所选房源存在无法自动验证的硬条件：{}",
                        result
                            .unverified_hard_constraints
                            .iter()
                            .map(|item| item.detail.as_str())
                            .collect::<Vec<_>>()
                            .join("、")
                    ),
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

pub struct MiniappService {
    config: ServiceConfig,
    clock: Arc<dyn Clock>,
    repository: FixtureListingRepository,
    runtime: Mutex<MiniappRuntime>,
    auth: MiniappAuthService,
    request_sequence: std::sync::atomic::AtomicU64,
}

impl MiniappService {
    pub fn new(config: ServiceConfig, clock: Arc<dyn Clock>) -> Self {
        let auth = MiniappAuthService::disabled(clock.clone());
        Self::with_repository_and_auth(config, clock, FixtureListingRepository::default(), auth)
    }

    pub fn with_repository(
        config: ServiceConfig,
        clock: Arc<dyn Clock>,
        repository: FixtureListingRepository,
    ) -> Self {
        let auth = MiniappAuthService::disabled(clock.clone());
        Self::with_repository_and_auth(config, clock, repository, auth)
    }

    pub fn with_repository_and_auth(
        config: ServiceConfig,
        clock: Arc<dyn Clock>,
        repository: FixtureListingRepository,
        auth: MiniappAuthService,
    ) -> Self {
        Self {
            config,
            clock,
            repository,
            runtime: Mutex::new(MiniappRuntime::default()),
            auth,
            request_sequence: std::sync::atomic::AtomicU64::new(1),
        }
    }

    pub async fn handle_sms_send(
        &self,
        request: SmsSendRequest,
        identity: RequestIdentity,
    ) -> ApiResponse<SmsSendResponse> {
        let request_id = self.request_id();
        match self.auth.send_sms_code(request, identity).await {
            Ok(data) => ApiResponse::success(request_id, "验证码发送成功", data),
            Err(error) => ApiResponse::failure(request_id, error.code, error.message, Vec::new()),
        }
    }

    pub fn handle_sms_verify(&self, request: SmsVerifyRequest) -> ApiResponse<DevSessionResponse> {
        let request_id = self.request_id();
        let phone = request.phone.clone();
        match self.auth.verify_sms_code(request) {
            Ok(data) => {
                if self
                    .runtime
                    .lock()
                    .map(|mut runtime| {
                        runtime.insert_authenticated_session(
                            data.session_token.clone(),
                            phone,
                            data.expires_at_epoch_seconds,
                            true,
                        );
                    })
                    .is_err()
                {
                    return ApiResponse::failure(
                        request_id,
                        "INTERNAL_ERROR",
                        "认证服务暂时不可用",
                        Vec::new(),
                    );
                }
                ApiResponse::success(request_id, "登录成功", data)
            }
            Err(error) => ApiResponse::failure(request_id, error.code, error.message, Vec::new()),
        }
    }

    pub async fn handle_wechat_login(
        &self,
        request: WechatLoginRequest,
    ) -> ApiResponse<DevSessionResponse> {
        let request_id = self.request_id();
        match self.auth.exchange_wechat_code(request).await {
            Ok(data) => {
                if self
                    .runtime
                    .lock()
                    .map(|mut runtime| {
                        runtime.insert_authenticated_session(
                            data.session_token.clone(),
                            "wechat-user".into(),
                            data.expires_at_epoch_seconds,
                            false,
                        );
                    })
                    .is_err()
                {
                    return ApiResponse::failure(
                        request_id,
                        "INTERNAL_ERROR",
                        "认证服务暂时不可用",
                        Vec::new(),
                    );
                }
                ApiResponse::success(request_id, "微信登录成功", data)
            }
            Err(error) => ApiResponse::failure(request_id, error.code, error.message, Vec::new()),
        }
    }

    pub fn handle_dev_session(
        &self,
        request: DevSessionRequest,
    ) -> ApiResponse<DevSessionResponse> {
        let request_id = self.request_id();
        if !self.config.dev_auth_enabled {
            return ApiResponse::failure(
                request_id,
                "DEV_AUTH_DISABLED",
                "本地模拟登录未启用；release构建始终禁用该能力",
                Vec::new(),
            );
        }
        if let Err(error) = validate_request_size(&request) {
            return ApiResponse::failure(
                request_id,
                "REQUEST_TOO_LARGE",
                error.message.clone(),
                vec![error],
            );
        }
        let result = self
            .runtime
            .lock()
            .map_err(|_| ServiceError::new("INTERNAL_ERROR", "本地服务暂时不可用"))
            .and_then(|mut runtime| {
                runtime.create_dev_session_at(request, self.clock.now_epoch_seconds())
            });
        match result {
            Ok(data) => ApiResponse::success(request_id, "本地演示会话已创建", data),
            Err(error) => failure(request_id, error),
        }
    }

    pub async fn handle_interpret(
        &self,
        request: InterpretDemandRequest,
    ) -> ApiResponse<super::types::DemandInterpretation> {
        let request_id = self.request_id();
        if let Err(error) = validate_request_size(&request) {
            return ApiResponse::failure(
                request_id,
                "REQUEST_TOO_LARGE",
                error.message.clone(),
                vec![error],
            );
        }
        let authentication = self
            .runtime
            .lock()
            .map_err(|_| ServiceError::new("INTERNAL_ERROR", "本地服务暂时不可用"))
            .and_then(|runtime| {
                runtime.ensure_authenticated(&request.session_token, self.clock.now_epoch_seconds())
            });
        if let Err(error) = authentication {
            return failure(request_id, error);
        }
        let initial_errors = validate_demand(&request.draft, false);
        if !initial_errors.is_empty() {
            return failure(request_id, ServiceError::validation(initial_errors));
        }

        let result = match self.config.interpreter_mode {
            InterpreterMode::LocalOnly => LocalDemandInterpreter.interpret(request.draft).await,
            InterpreterMode::Environment => self.interpret_from_environment(request.draft).await,
        };
        match result {
            Ok(data) => ApiResponse::success(request_id, "需求已结构化，请确认并补充", data),
            Err(error) if error.starts_with("AI_PROVIDER_INVALID:") => ApiResponse::failure(
                request_id,
                "AI_PROVIDER_INVALID",
                error.trim_start_matches("AI_PROVIDER_INVALID:"),
                Vec::new(),
            ),
            Err(message) => {
                ApiResponse::failure(request_id, "AI_INTERPRETATION_ERROR", message, Vec::new())
            }
        }
    }

    pub fn handle_matches(&self, mut request: MatchRequest) -> ApiResponse<MatchResponse> {
        let request_id = self.request_id();
        if let Err(error) = validate_request_size(&request) {
            return ApiResponse::failure(
                request_id,
                "REQUEST_TOO_LARGE",
                error.message.clone(),
                vec![error],
            );
        }
        let authentication = self
            .runtime
            .lock()
            .map_err(|_| ServiceError::new("INTERNAL_ERROR", "本地服务暂时不可用"))
            .and_then(|runtime| {
                runtime.ensure_authenticated(&request.session_token, self.clock.now_epoch_seconds())
            });
        if let Err(error) = authentication {
            return failure(request_id, error);
        }
        let errors = validate_demand(&request.demand, true);
        if !errors.is_empty() {
            return failure(request_id, ServiceError::validation(errors));
        }
        normalize_constraint_priorities(&mut request.demand);
        ApiResponse::success(
            request_id,
            "匹配完成，结果仅为脱敏演示数据",
            match_listings_with_clock(&request.demand, &self.repository, self.clock.as_ref()),
        )
    }

    pub fn handle_leads(&self, request: SubmitLeadRequest) -> ApiResponse<LeadRecord> {
        let request_id = self.request_id();
        if let Err(error) = validate_request_size(&request) {
            return ApiResponse::failure(
                request_id,
                "REQUEST_TOO_LARGE",
                error.message.clone(),
                vec![error],
            );
        }
        let result = self
            .runtime
            .lock()
            .map_err(|_| ServiceError::new("INTERNAL_ERROR", "本地服务暂时不可用"))
            .and_then(|mut runtime| {
                runtime.submit_lead_at(
                    request,
                    self.clock.now_epoch_seconds(),
                    &self.repository,
                    self.clock.as_ref(),
                )
            });
        match result {
            Ok(data) => {
                ApiResponse::success(request_id, "本地演示线索已生成；未通知真实顾问", data)
            }
            Err(error) => failure(request_id, error),
        }
    }

    pub fn handle_metadata(&self) -> ApiResponse<MetadataOptions> {
        ApiResponse::success(self.request_id(), "选项加载成功", metadata())
    }

    async fn interpret_from_environment(
        &self,
        draft: super::types::DemandDraft,
    ) -> Result<super::types::DemandInterpretation, String> {
        let provider = std::env::var("YIZU_MINIAPP_AI_PROVIDER").unwrap_or_else(|_| "local".into());
        match provider.as_str() {
            "local" => LocalDemandInterpreter.interpret(draft).await,
            "bailian" => {
                use super::interpreter::bailian::{BailianConfig, BailianDemandInterpreter};
                match BailianConfig::from_env().and_then(BailianDemandInterpreter::new) {
                    Ok(interpreter) => interpret_with_local_fallback(&interpreter, draft).await,
                    Err(_) => local_fallback(draft).await,
                }
            }
            _ => Err("AI_PROVIDER_INVALID:YIZU_MINIAPP_AI_PROVIDER 只能是 local 或 bailian".into()),
        }
    }

    fn request_id(&self) -> String {
        use std::sync::atomic::Ordering;
        format!(
            "miniapp-{:x}-{:x}",
            self.clock.now_epoch_seconds(),
            self.request_sequence.fetch_add(1, Ordering::Relaxed)
        )
    }
}

async fn interpret_with_local_fallback(
    interpreter: &impl DemandInterpreter,
    draft: super::types::DemandDraft,
) -> Result<super::types::DemandInterpretation, String> {
    match interpreter.interpret(draft.clone()).await {
        Ok(output) => Ok(output),
        Err(_) => local_fallback(draft).await,
    }
}

async fn local_fallback(
    draft: super::types::DemandDraft,
) -> Result<super::types::DemandInterpretation, String> {
    LocalDemandInterpreter
        .interpret(draft)
        .await
        .map(|mut output| {
            output.fallback_reason = Some("bailian_unavailable".into());
            output
        })
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
        constraint_keys: vec![
            "budget".into(),
            "freight_elevator".into(),
            "elevator_capacity".into(),
            "power_capacity".into(),
            "fire_safety".into(),
            "truck_access".into(),
            "loading_dock".into(),
            "sublease".into(),
            "floor".into(),
            "move_in".into(),
        ],
        business_timezone: BUSINESS_TIMEZONE.into(),
        currency_storage_unit: "cents".into(),
        currency_display_unit: "yuan".into(),
        demo_data: true,
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

fn failure<T>(request_id: String, error: ServiceError) -> ApiResponse<T> {
    ApiResponse::failure(request_id, error.code, error.message, error.errors)
}

pub fn production_service() -> &'static MiniappService {
    static SERVICE: std::sync::OnceLock<MiniappService> = std::sync::OnceLock::new();
    SERVICE.get_or_init(|| {
        dotenvy::dotenv().ok();
        let server_config = super::config::MiniappServerConfig::from_environment()
            .unwrap_or_else(|status| panic!("{status}"));
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let auth = MiniappAuthService::from_server_config(&server_config, clock.clone())
            .unwrap_or_else(|status| panic!("{status}"));
        MiniappService::with_repository_and_auth(
            ServiceConfig::from_environment(),
            clock,
            FixtureListingRepository::default(),
            auth,
        )
    })
}

#[cfg(test)]
fn test_api_service_slot() -> &'static std::sync::RwLock<Option<Arc<MiniappService>>> {
    static SERVICE: std::sync::OnceLock<std::sync::RwLock<Option<Arc<MiniappService>>>> =
        std::sync::OnceLock::new();
    SERVICE.get_or_init(|| std::sync::RwLock::new(None))
}

#[cfg(test)]
pub fn install_test_api_service(service: Arc<MiniappService>) {
    *test_api_service_slot()
        .write()
        .expect("test API service write lock") = Some(service);
}

#[cfg(test)]
pub fn test_api_service() -> Arc<MiniappService> {
    test_api_service_slot()
        .read()
        .expect("test API service read lock")
        .clone()
        .expect("HTTP E2E must install its isolated API service")
}

#[cfg(test)]
mod tests {
    use time::{Date, Month};

    use super::*;
    use crate::services::miniapp::{
        clock::FixedClock,
        types::{
            ConstraintKey, ConstraintLevel, ConstraintPriority, DemandConstraints, DemandDraft,
            LeadSubmission, SpaceType,
        },
        validation::DONGGUAN_TOWNS,
    };

    struct MockBailianInterpreter {
        fail: bool,
    }

    impl DemandInterpreter for MockBailianInterpreter {
        async fn interpret(
            &self,
            draft: DemandDraft,
        ) -> Result<super::super::types::DemandInterpretation, String> {
            if self.fail {
                Err("synthetic provider failure".into())
            } else {
                Ok(super::super::types::DemandInterpretation {
                    demand: draft,
                    provider: "mock-bailian".into(),
                    fallback_reason: None,
                })
            }
        }
    }

    fn clock() -> Arc<FixedClock> {
        Arc::new(FixedClock::new(
            1_723_305_600,
            Date::from_calendar_date(2026, Month::August, 10).expect("date"),
        ))
    }

    fn service() -> (MiniappService, Arc<FixedClock>) {
        let clock = clock();
        (
            MiniappService::new(ServiceConfig::isolated_local(true), clock.clone()),
            clock,
        )
    }

    fn synthetic_phone() -> String {
        ["139", "0000", "0000"].concat()
    }

    fn session(service: &MiniappService) -> String {
        service
            .handle_dev_session(DevSessionRequest {
                phone: synthetic_phone(),
                contact_confirmed: true,
            })
            .data
            .expect("session")
            .session_token
    }

    #[tokio::test]
    async fn mock百炼成功和失败回退本地解析器() {
        let source = demand("松山湖", 1400, 1600);
        let success =
            interpret_with_local_fallback(&MockBailianInterpreter { fail: false }, source.clone())
                .await
                .expect("mock success");
        assert_eq!(success.provider, "mock-bailian");

        let fallback =
            interpret_with_local_fallback(&MockBailianInterpreter { fail: true }, source)
                .await
                .expect("local fallback");
        assert_eq!(fallback.provider, "local");
        assert_eq!(
            fallback.fallback_reason.as_deref(),
            Some("bailian_unavailable")
        );
    }

    fn demand(town: &str, min: u32, max: u32) -> DemandDraft {
        DemandDraft {
            raw_text: format!("{town}厂房"),
            constraints: DemandConstraints {
                space_type: Some(SpaceType::Factory),
                target_towns: vec![town.into()],
                area_min_sqm: Some(min),
                area_max_sqm: Some(max),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn lead_request(
        token: &str,
        demand: DemandDraft,
        listing: &str,
        key: &str,
    ) -> SubmitLeadRequest {
        SubmitLeadRequest {
            session_token: token.into(),
            submission: LeadSubmission {
                demand,
                recommended_listing_ids: vec![listing.into()],
                source_channel: "miniapp_ai_demand".into(),
                idempotency_key: key.into(),
            },
        }
    }

    #[test]
    fn 元数据返回完整东莞镇街集合() {
        let (service, _) = service();
        let metadata = service.handle_metadata().data.expect("metadata");
        assert_eq!(metadata.towns.len(), DONGGUAN_TOWNS.len());
        for town in ["石龙", "清溪", "凤岗", "桥头"] {
            assert!(metadata.towns.contains(&town.to_string()));
        }
    }

    #[test]
    fn 非法镇街仍被服务端拒绝() {
        let (service, _) = service();
        let token = session(&service);
        let response = service.handle_matches(MatchRequest {
            session_token: token,
            demand: demand("伪造镇街", 1000, 1200),
        });
        assert_eq!(response.code, "VALIDATION_ERROR");
    }

    #[test]
    fn 未登录和未确认联系方式被拒绝() {
        let (service, _) = service();
        let response = service.handle_leads(lead_request(
            "missing",
            demand("松山湖", 1400, 1650),
            "demo-factory-001",
            "idempotency-key-001",
        ));
        assert_eq!(response.code, "UNAUTHENTICATED");

        service
            .runtime
            .lock()
            .expect("runtime")
            .insert_unconfirmed_session("token", &synthetic_phone(), u64::MAX);
        let response = service.handle_leads(lead_request(
            "token",
            demand("松山湖", 1400, 1650),
            "demo-factory-001",
            "idempotency-key-002",
        ));
        assert_eq!(response.code, "CONTACT_NOT_CONFIRMED");
    }

    #[test]
    fn 货梯硬条件无自由文本也由服务端拒绝() {
        let (service, _) = service();
        let token = session(&service);
        let mut request = demand("长安", 900, 1100);
        request.constraints.needs_freight_elevator = Some(true);
        let response = service.handle_leads(lead_request(
            &token,
            request,
            "demo-factory-005",
            "idempotency-key-003",
        ));
        assert_eq!(response.code, "HARD_CONDITION_NOT_MET");
    }

    #[test]
    fn 用电偏好不阻止线索但硬条件阻止() {
        let (service, _) = service();
        let token = session(&service);
        let mut request = demand("松山湖", 1500, 1650);
        request.constraints.power_capacity_kva = Some(700);
        request.constraint_priorities = vec![ConstraintPriority {
            key: ConstraintKey::PowerCapacity,
            level: ConstraintLevel::Preference,
        }];
        let response = service.handle_leads(lead_request(
            &token,
            request.clone(),
            "demo-factory-002",
            "idempotency-key-004",
        ));
        assert_eq!(response.code, "OK");

        request.constraint_priorities[0].level = ConstraintLevel::Hard;
        let response = service.handle_leads(lead_request(
            &token,
            request,
            "demo-factory-002",
            "idempotency-key-005",
        ));
        assert_eq!(response.code, "HARD_CONDITION_NOT_MET");
    }

    #[test]
    fn 未知硬条件被标记为无法验证() {
        let (service, _) = service();
        let token = session(&service);
        let mut request = demand("松山湖", 1400, 1500);
        request.hard_conditions = vec!["必须临近指定供应商".into()];
        let response = service.handle_leads(lead_request(
            &token,
            request,
            "demo-factory-001",
            "idempotency-key-006",
        ));
        assert_eq!(response.code, "HARD_CONDITION_UNVERIFIED");
    }

    #[test]
    fn 满足硬条件可生成且幂等重试返回同一线索() {
        let (service, _) = service();
        let token = session(&service);
        let mut request = demand("松山湖", 1400, 1500);
        request.constraints.needs_freight_elevator = Some(true);
        let first = service.handle_leads(lead_request(
            &token,
            request.clone(),
            "demo-factory-001",
            "idempotency-key-007",
        ));
        let second = service.handle_leads(lead_request(
            &token,
            request,
            "demo-factory-001",
            "idempotency-key-007",
        ));
        assert_eq!(first.code, "OK");
        assert_eq!(
            first.data.expect("first").lead_number,
            second.data.expect("second").lead_number
        );
    }

    #[test]
    fn 过期会话和双重开发认证门禁生效() {
        assert!(!dev_auth_allowed(false, true));
        assert!(!dev_auth_allowed(true, false));
        assert!(dev_auth_allowed(true, true));

        let (service, clock) = service();
        let token = session(&service);
        clock.set_epoch_seconds(clock.now_epoch_seconds() + 7200);
        let response = service.handle_matches(MatchRequest {
            session_token: token,
            demand: demand("松山湖", 1400, 1650),
        });
        assert_eq!(response.code, "SESSION_EXPIRED");
    }
}
