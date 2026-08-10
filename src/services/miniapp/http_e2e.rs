//! Isolated real-HTTP regression test for the Phase 1 miniapp slice.
//!
//! This router deliberately exposes only the five miniapp endpoints and calls
//! the same `MiniappService` used by Dioxus. It cannot initialize the root app,
//! Bailian, SpacetimeDB, SMS, object storage, or any other external adapter.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    extract::{rejection::JsonRejection, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use time::{Date, Month};
use tokio::{net::TcpListener, sync::oneshot};

use super::{
    clock::{Clock, FixedClock},
    repository::{FixtureListingRepository, ListingRepository},
    service::{MiniappService, ServiceConfig},
    types::{
        ApiResponse, DevSessionRequest, InterpretDemandRequest, ListingVerificationLevel,
        MatchRequest, SubmitLeadRequest,
    },
};

fn isolated_router(service: Arc<MiniappService>) -> Router {
    Router::new()
        .route("/api/miniapp/v1/auth/dev-session", post(dev_session))
        .route("/api/miniapp/v1/demands/interpret", post(interpret))
        .route("/api/miniapp/v1/matches", post(matches))
        .route("/api/miniapp/v1/leads", post(leads))
        .route("/api/miniapp/v1/metadata/options", get(metadata))
        .fallback(not_found)
        .with_state(service)
}

async fn dev_session(
    State(service): State<Arc<MiniappService>>,
    request: Result<Json<DevSessionRequest>, JsonRejection>,
) -> Response {
    map_json(request, |request| service.handle_dev_session(request))
}

async fn interpret(
    State(service): State<Arc<MiniappService>>,
    request: Result<Json<InterpretDemandRequest>, JsonRejection>,
) -> Response {
    match request {
        Ok(Json(request)) => Json(service.handle_interpret(request).await).into_response(),
        Err(error) => invalid_json(error),
    }
}

async fn matches(
    State(service): State<Arc<MiniappService>>,
    request: Result<Json<MatchRequest>, JsonRejection>,
) -> Response {
    map_json(request, |request| service.handle_matches(request))
}

async fn leads(
    State(service): State<Arc<MiniappService>>,
    request: Result<Json<SubmitLeadRequest>, JsonRejection>,
) -> Response {
    map_json(request, |request| service.handle_leads(request))
}

async fn metadata(State(service): State<Arc<MiniappService>>) -> Response {
    Json(service.handle_metadata()).into_response()
}

async fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::<Value>::failure(
            "isolated-http-not-found".into(),
            "NOT_FOUND",
            "仅支持隔离的 miniapp v1 接口",
            Vec::new(),
        )),
    )
        .into_response()
}

fn map_json<I, O>(
    request: Result<Json<I>, JsonRejection>,
    handler: impl FnOnce(I) -> ApiResponse<O>,
) -> Response
where
    I: DeserializeOwned,
    O: Serialize,
{
    match request {
        Ok(Json(request)) => Json(handler(request)).into_response(),
        Err(error) => invalid_json(error),
    }
}

fn invalid_json(error: JsonRejection) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(ApiResponse::<Value>::failure(
            "isolated-http-invalid-json".into(),
            "INVALID_JSON",
            "请求体不是有效JSON",
            vec![super::types::ApiFieldError {
                field: "request".into(),
                message: error.body_text(),
            }],
        )),
    )
        .into_response()
}

struct RunningServer {
    address: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), std::io::Error>>,
}

impl RunningServer {
    async fn start(service: Arc<MiniappService>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind isolated loopback listener");
        let address = listener.local_addr().expect("isolated listener address");
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, isolated_router(service))
                .with_graceful_shutdown(async {
                    let _ = shutdown_receiver.await;
                })
                .await
        });
        Self {
            address,
            shutdown: Some(shutdown),
            task,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.address, path)
    }

    async fn stop(mut self) {
        self.shutdown.take().expect("shutdown sender").send(()).ok();
        self.task
            .await
            .expect("server join")
            .expect("server result");
        TcpListener::bind(self.address)
            .await
            .expect("isolated port released after graceful shutdown");
    }
}

async fn post_json<T: Serialize>(
    client: &reqwest::Client,
    server: &RunningServer,
    path: &str,
    body: &T,
) -> (StatusCode, Value) {
    let response = client
        .post(server.url(path))
        .header("content-type", "application/json")
        .body(serde_json::to_vec(body).expect("serialize HTTP body"))
        .send()
        .await
        .expect("HTTP request");
    assert!(response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("application/json")));
    let status = response.status();
    let value = serde_json::from_slice(&response.bytes().await.expect("HTTP bytes"))
        .expect("JSON response");
    (status, value)
}

fn assert_code(value: &Value, expected: &str) {
    assert_eq!(value["code"], expected, "response: {value:#}");
    assert!(value["request_id"]
        .as_str()
        .is_some_and(|id| !id.is_empty()));
}

fn synthetic_phone() -> String {
    ["139", "0000", "0000"].concat()
}

#[tokio::test]
async fn miniapp_http_e2e_full_flow_and_negative_contracts() {
    let clock = Arc::new(FixedClock::new(
        1_800_000_000,
        Date::from_calendar_date(2027, Month::January, 15).expect("date"),
    ));
    let mut listings = FixtureListingRepository::default().list();
    listings
        .iter_mut()
        .find(|listing| listing.listing_id == "demo-warehouse-005")
        .expect("unverified HTTP fixture")
        .verification_level = ListingVerificationLevel::L2;
    let service = Arc::new(MiniappService::with_repository(
        ServiceConfig::isolated_local(true),
        clock.clone(),
        FixtureListingRepository::from_listings(listings),
    ));
    let server = RunningServer::start(service).await;
    let client = reqwest::Client::new();

    let metadata_response = client
        .get(server.url("/api/miniapp/v1/metadata/options"))
        .send()
        .await
        .expect("metadata HTTP");
    assert_eq!(metadata_response.status(), StatusCode::OK);
    assert!(metadata_response.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("application/json"));
    let metadata: Value = metadata_response.json().await.expect("metadata JSON");
    assert_code(&metadata, "OK");
    assert_eq!(metadata["data"]["towns"].as_array().unwrap().len(), 33);
    assert_eq!(metadata["data"]["business_timezone"], "Asia/Shanghai");
    assert_eq!(metadata["data"]["currency_storage_unit"], "cents");

    let (status, login) = post_json(
        &client,
        &server,
        "/api/miniapp/v1/auth/dev-session",
        &json!({"phone":synthetic_phone(), "contact_confirmed":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_code(&login, "OK");
    let token = login["data"]["session_token"].as_str().unwrap().to_owned();

    let draft = json!({
        "raw_text":"想在松山湖找1500平方米左右的厂房，需要3吨货梯和500kVA用电，丙类消防",
        "constraints":{
            "space_type":null,"target_towns":[],"area_min_sqm":null,"area_max_sqm":null,
            "rent_min_cents":null,"rent_max_cents":null,"rent_unit":null,"move_in_time":null,
            "floor_preference":null,"needs_freight_elevator":null,"elevator_min_tons":null,
            "power_capacity_kva":null,"fire_requirement":null,"logistics_requirement":null,
            "loading_requirement":null,"accepts_sublease":null,"other_notes":null
        },
        "hard_conditions":[],"preference_conditions":[],"constraint_priorities":[],
        "missing_fields":[],"ai_confidence":0
    });
    let (_, interpreted) = post_json(
        &client,
        &server,
        "/api/miniapp/v1/demands/interpret",
        &json!({"session_token":token, "draft":draft}),
    )
    .await;
    assert_code(&interpreted, "OK");
    assert_eq!(interpreted["data"]["provider"], "local");
    let demand = interpreted["data"]["demand"].clone();

    let (_, matched) = post_json(
        &client,
        &server,
        "/api/miniapp/v1/matches",
        &json!({"session_token":token, "demand":demand}),
    )
    .await;
    assert_code(&matched, "OK");
    let selected = matched["data"]["matches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| {
            item["unmet_hard_constraints"]
                .as_array()
                .unwrap()
                .is_empty()
                && item["unverified_hard_constraints"]
                    .as_array()
                    .unwrap()
                    .is_empty()
        })
        .expect("at least one fully eligible listing")["listing"]["listing_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let lead_body = json!({
        "session_token":token,
        "submission":{
            "demand":demand,
            "recommended_listing_ids":[selected],
            "source_channel":"miniapp_ai_demand",
            "idempotency_key":"http-e2e-idempotency-001",
            "client_claimed_all_hard_conditions_satisfied":true
        },
        "client_claimed_safe":true
    });
    let (_, first_lead) = post_json(&client, &server, "/api/miniapp/v1/leads", &lead_body).await;
    let (_, duplicate) = post_json(&client, &server, "/api/miniapp/v1/leads", &lead_body).await;
    assert_code(&first_lead, "OK");
    assert_code(&duplicate, "OK");
    assert_eq!(
        first_lead["data"]["lead_number"],
        duplicate["data"]["lead_number"]
    );

    let (_, unauthenticated) = post_json(
        &client,
        &server,
        "/api/miniapp/v1/matches",
        &json!({"session_token":"missing", "demand":lead_body["submission"]["demand"]}),
    )
    .await;
    assert_code(&unauthenticated, "UNAUTHENTICATED");

    let hard_unmet = json!({
        "raw_text":"长安约1000平方米厂房，必须有货梯",
        "constraints": {"space_type":"factory","target_towns":["长安"],"area_min_sqm":900,"area_max_sqm":1100,
            "rent_min_cents":null,"rent_max_cents":null,"rent_unit":null,"move_in_time":null,"floor_preference":null,
            "needs_freight_elevator":true,"elevator_min_tons":null,"power_capacity_kva":null,"fire_requirement":null,
            "logistics_requirement":null,"loading_requirement":null,"accepts_sublease":null,"other_notes":null},
        "hard_conditions":[],"preference_conditions":[],
        "constraint_priorities":[{"key":"freight_elevator","level":"hard"}],"missing_fields":[],"ai_confidence":1
    });
    let (_, hard_unmet_lead) = post_json(&client, &server, "/api/miniapp/v1/leads", &json!({
        "session_token":token,"submission":{"demand":hard_unmet,"recommended_listing_ids":["demo-factory-005"],
        "source_channel":"miniapp_ai_demand","idempotency_key":"http-e2e-hard-unmet-001"},"forged":true
    })).await;
    assert_code(&hard_unmet_lead, "HARD_CONDITION_NOT_MET");

    let unverified = json!({
        "raw_text":"塘厦约1800平方米仓库，用电至少300kVA",
        "constraints":{"space_type":"warehouse","target_towns":["塘厦"],"area_min_sqm":1700,"area_max_sqm":1900,
            "rent_min_cents":null,"rent_max_cents":null,"rent_unit":null,"move_in_time":null,"floor_preference":null,
            "needs_freight_elevator":null,"elevator_min_tons":null,"power_capacity_kva":300,"fire_requirement":null,
            "logistics_requirement":null,"loading_requirement":null,"accepts_sublease":null,"other_notes":null},
        "hard_conditions":[],"preference_conditions":[],
        "constraint_priorities":[{"key":"power_capacity","level":"hard"}],"missing_fields":[],"ai_confidence":1
    });
    let (_, unverified_lead) = post_json(&client, &server, "/api/miniapp/v1/leads", &json!({
        "session_token":token,"submission":{"demand":unverified,"recommended_listing_ids":["demo-warehouse-005"],
        "source_channel":"miniapp_ai_demand","idempotency_key":"http-e2e-unverified-001"},
        "client_claimed_all_hard_conditions_satisfied":true
    })).await;
    assert_code(&unverified_lead, "HARD_CONDITION_UNVERIFIED");

    let (_, fake_listing) = post_json(
        &client,
        &server,
        "/api/miniapp/v1/leads",
        &json!({
            "session_token":token,"submission":{"demand":lead_body["submission"]["demand"],
            "recommended_listing_ids":["forged-listing"],"source_channel":"miniapp_ai_demand",
            "idempotency_key":"http-e2e-fake-listing-001"}
        }),
    )
    .await;
    assert_code(&fake_listing, "LISTING_NOT_RECOMMENDED");

    let malformed = client
        .post(server.url("/api/miniapp/v1/matches"))
        .header("content-type", "application/json")
        .body("{not-json")
        .send()
        .await
        .expect("malformed request");
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    assert!(malformed.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("application/json"));
    let malformed: Value = malformed.json().await.expect("malformed envelope");
    assert_code(&malformed, "INVALID_JSON");

    let missing = client
        .get(server.url("/api/not-part-of-miniapp"))
        .send()
        .await
        .expect("404 request");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert!(missing.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("application/json"));
    assert_code(&missing.json().await.expect("404 envelope"), "NOT_FOUND");

    clock.set_epoch_seconds(clock.now_epoch_seconds() + 2 * 60 * 60);
    let (_, expired) = post_json(
        &client,
        &server,
        "/api/miniapp/v1/matches",
        &json!({"session_token":token,"demand":lead_body["submission"]["demand"]}),
    )
    .await;
    assert_code(&expired, "SESSION_EXPIRED");

    server.stop().await;
}

#[tokio::test]
async fn miniapp_http_e2e_dev_auth_requires_explicit_debug_gate() {
    let service = Arc::new(MiniappService::new(
        ServiceConfig::isolated_local(false),
        Arc::new(FixedClock::new(
            1_800_000_000,
            Date::from_calendar_date(2027, Month::January, 15).expect("date"),
        )),
    ));
    let server = RunningServer::start(service).await;
    let (_, response) = post_json(
        &reqwest::Client::new(),
        &server,
        "/api/miniapp/v1/auth/dev-session",
        &json!({"phone":synthetic_phone(), "contact_confirmed":true}),
    )
    .await;
    assert_code(&response, "DEV_AUTH_DISABLED");
    server.stop().await;
}
