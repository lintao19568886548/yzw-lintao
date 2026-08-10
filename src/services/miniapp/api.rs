use dioxus::prelude::*;

use super::types::{
    ApiResponse, DemandInterpretation, DevSessionRequest, DevSessionResponse,
    InterpretDemandRequest, LeadRecord, MatchRequest, MatchResponse, MetadataOptions,
    SubmitLeadRequest,
};

#[post("/api/miniapp/v1/auth/dev-session")]
async fn create_dev_session(
    request: DevSessionRequest,
) -> Result<ApiResponse<DevSessionResponse>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        return Ok(super::service::production_service().handle_dev_session(request));
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
) -> Result<ApiResponse<DemandInterpretation>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        return Ok(super::service::production_service()
            .handle_interpret(request)
            .await);
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
        return Ok(super::service::production_service().handle_matches(request));
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
        return Ok(super::service::production_service().handle_leads(request));
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
        return Ok(super::service::production_service().handle_metadata());
    }
    #[cfg(not(feature = "server"))]
    {
        Err(ServerFnError::new("该接口只能在服务端执行"))
    }
}
