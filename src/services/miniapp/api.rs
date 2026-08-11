use dioxus::prelude::dioxus_fullstack::Json;
use dioxus::prelude::*;

use super::types::{
    ApiResponse, DemandInterpretation, DevSessionRequest, DevSessionResponse,
    InterpretDemandRequest, LeadRecord, MatchRequest, MatchResponse, MetadataOptions,
    SmsSendRequest, SmsSendResponse, SmsVerifyRequest, SubmitLeadRequest, WechatLoginRequest,
};

#[post("/api/miniapp/v1/auth/dev-session")]
async fn create_dev_session(
    Json(request): Json<DevSessionRequest>,
) -> Result<ApiResponse<DevSessionResponse>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        return Ok(api_service().handle_dev_session(request));
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = request;
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[post("/api/miniapp/v1/auth/sms/send")]
async fn send_sms_code(
    Json(request): Json<SmsSendRequest>,
) -> Result<ApiResponse<SmsSendResponse>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        let identity = super::auth::RequestIdentity {
            ip: request_ip(),
            device_id: request.device_id.clone(),
        };
        return Ok(api_service().handle_sms_send(request, identity).await);
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = request;
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[post("/api/miniapp/v1/auth/sms/verify")]
async fn verify_sms_code(
    Json(request): Json<SmsVerifyRequest>,
) -> Result<ApiResponse<DevSessionResponse>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        return Ok(api_service().handle_sms_verify(request));
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = request;
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[post("/api/miniapp/v1/auth/wechat")]
async fn exchange_wechat_code(
    Json(request): Json<WechatLoginRequest>,
) -> Result<ApiResponse<DevSessionResponse>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        return Ok(api_service().handle_wechat_login(request).await);
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = request;
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[post("/api/miniapp/v1/demands/interpret")]
async fn interpret_demand(
    Json(request): Json<InterpretDemandRequest>,
) -> Result<ApiResponse<DemandInterpretation>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        return Ok(api_service().handle_interpret(request).await);
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = request;
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[post("/api/miniapp/v1/matches")]
async fn create_matches(
    Json(request): Json<MatchRequest>,
) -> Result<ApiResponse<MatchResponse>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        return Ok(api_service().handle_matches(request));
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = request;
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[post("/api/miniapp/v1/leads")]
async fn submit_lead(
    Json(request): Json<SubmitLeadRequest>,
) -> Result<ApiResponse<LeadRecord>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        return Ok(api_service().handle_leads(request));
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
        return Ok(api_service().handle_metadata());
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}

#[cfg(all(feature = "server", not(test)))]
fn api_service() -> &'static super::service::MiniappService {
    super::service::production_service()
}

#[cfg(all(feature = "server", test))]
fn api_service() -> std::sync::Arc<super::service::MiniappService> {
    super::service::test_api_service()
}

#[cfg(feature = "server")]
fn request_ip() -> String {
    use std::net::{IpAddr, SocketAddr};

    use dioxus::prelude::dioxus_fullstack::FullstackContext;
    use dioxus::server::axum::extract::ConnectInfo;

    let Some(context) = FullstackContext::current() else {
        return "unavailable".into();
    };
    let parts = context.parts_mut();
    if let Some(ConnectInfo(address)) = parts.extensions.get::<ConnectInfo<SocketAddr>>() {
        return address.ip().to_string();
    }
    let trust_proxy = std::env::var("YIZU_TRUST_PROXY_HEADERS")
        .is_ok_and(|value| value.eq_ignore_ascii_case("true"));
    if trust_proxy {
        for name in ["x-real-ip", "x-forwarded-for"] {
            if let Some(value) = parts
                .headers
                .get(name)
                .and_then(|value| value.to_str().ok())
            {
                if let Some(candidate) = value.split(',').next() {
                    if let Ok(ip) = candidate.trim().parse::<IpAddr>() {
                        return ip.to_string();
                    }
                }
            }
        }
    }
    // A conservative shared bucket still rate-limits direct deployments when
    // peer metadata is unavailable instead of trusting a client-supplied IP.
    "unavailable".into()
}
