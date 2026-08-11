//! Real Dioxus Server Function HTTP regression for the Phase 1 miniapp slice.
//!
//! The router is populated exclusively by Dioxus' inventory-backed
//! `register_server_functions`; this file does not duplicate any miniapp route.

use std::{net::SocketAddr, sync::Arc};

use axum::{
    http::{Method, StatusCode},
    response::IntoResponse,
    Json, Router,
};
use dioxus::server::{DioxusRouterExt, FullstackState, ServerFunction};
use serde::Serialize;
use serde_json::{json, Value};
use time::{Date, Month};
use tokio::{net::TcpListener, sync::oneshot};

use super::{
    clock::{Clock, FixedClock},
    repository::{FixtureListingRepository, ListingRepository},
    service::{install_test_api_service, MiniappService, ServiceConfig},
    types::{ApiResponse, ListingVerificationLevel},
};

const MINIAPP_PATHS: [(&str, Method); 8] = [
    ("/api/miniapp/v1/auth/dev-session", Method::POST),
    ("/api/miniapp/v1/auth/sms/send", Method::POST),
    ("/api/miniapp/v1/auth/sms/verify", Method::POST),
    ("/api/miniapp/v1/auth/wechat", Method::POST),
    ("/api/miniapp/v1/demands/interpret", Method::POST),
    ("/api/miniapp/v1/matches", Method::POST),
    ("/api/miniapp/v1/leads", Method::POST),
    ("/api/miniapp/v1/metadata/options", Method::GET),
];

fn assert_real_dioxus_inventory() {
    let registered = ServerFunction::collect()
        .into_iter()
        .filter(|function| function.path().starts_with("/api/miniapp/v1/"))
        .map(|function| {
            (
                function.path().to_owned(),
                function.method().as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let expected = MINIAPP_PATHS
        .iter()
        .map(|(path, method)| ((*path).to_owned(), method.as_str().to_owned()))
        .collect::<Vec<_>>();
    assert_eq!(
        registered.len(),
        expected.len(),
        "duplicate or missing routes"
    );
    for route in expected {
        assert!(
            registered.contains(&route),
            "missing Dioxus route: {route:?}"
        );
    }
}

fn isolated_router() -> Router {
    assert_real_dioxus_inventory();
    Router::<FullstackState>::new()
        .register_server_functions()
        .fallback(not_found)
        .with_state(FullstackState::headless())
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ApiResponse::<Value>::failure(
            "dioxus-http-not-found".into(),
            "NOT_FOUND",
            "未注册的接口路径",
            Vec::new(),
        )),
    )
}

struct RunningServer {
    address: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<Result<(), std::io::Error>>,
}

impl RunningServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind isolated loopback listener");
        let address = listener.local_addr().expect("isolated listener address");
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, isolated_router())
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

fn assert_envelope(value: &Value, expected: &str) {
    assert_eq!(value["code"], expected, "response: {value:#}");
    assert!(value["message"].is_string());
    assert!(value["request_id"]
        .as_str()
        .is_some_and(|id| !id.is_empty()));
    assert!(value.get("data").is_some());
    assert!(value["errors"].is_array());
}

fn synthetic_phone() -> String {
    ["139", "0000", "0000"].concat()
}

fn demand_json(
    town: &str,
    space_type: &str,
    min: u32,
    max: u32,
    freight: Option<bool>,
    level: Option<&str>,
) -> Value {
    let priorities = level
        .map(|level| vec![json!({"key":"freight_elevator","level":level})])
        .unwrap_or_default();
    json!({
        "raw_text":format!("{town}{min}-{max}平方米{space_type}"),
        "constraints":{
            "space_type":space_type,"target_towns":[town],"area_min_sqm":min,"area_max_sqm":max,
            "rent_min_cents":null,"rent_max_cents":null,"rent_unit":null,"move_in_time":null,
            "floor_preference":null,"needs_freight_elevator":freight,"elevator_min_tons":null,
            "power_capacity_kva":null,"fire_requirement":null,"logistics_requirement":null,
            "loading_requirement":null,"accepts_sublease":null,"other_notes":null
        },
        "hard_conditions":[],"preference_conditions":[],
        "constraint_priorities":priorities,"missing_fields":[],"ai_confidence":1
    })
}

async fn submit_for_listing(
    client: &reqwest::Client,
    server: &RunningServer,
    token: &str,
    demand: Value,
    listing_id: &str,
    key: &str,
) -> Value {
    post_json(
        client,
        server,
        "/api/miniapp/v1/leads",
        &json!({
            "session_token":token,
            "submission":{
                "demand":demand,"recommended_listing_ids":[listing_id],
                "source_channel":"miniapp_ai_demand","idempotency_key":key
            }
        }),
    )
    .await
    .1
}

#[tokio::test]
async fn dioxus_server_function_http_e2e() {
    let disabled = Arc::new(MiniappService::new(
        ServiceConfig::isolated_local(false),
        Arc::new(FixedClock::new(
            1_800_000_000,
            Date::from_calendar_date(2027, Month::January, 15).expect("date"),
        )),
    ));
    install_test_api_service(disabled);
    let disabled_server = RunningServer::start().await;
    let client = reqwest::Client::new();
    let (_, disabled_auth) = post_json(
        &client,
        &disabled_server,
        "/api/miniapp/v1/auth/dev-session",
        &json!({"phone":synthetic_phone(),"contact_confirmed":true}),
    )
    .await;
    assert_envelope(&disabled_auth, "DEV_AUTH_DISABLED");
    disabled_server.stop().await;

    let clock = Arc::new(FixedClock::new(
        1_800_000_000,
        Date::from_calendar_date(2027, Month::January, 15).expect("date"),
    ));
    let mut listings = FixtureListingRepository::default().list();
    listings
        .iter_mut()
        .find(|listing| listing.listing_id == "demo-warehouse-005")
        .expect("unverified hard-condition fixture")
        .verification_level = ListingVerificationLevel::L2;
    install_test_api_service(Arc::new(MiniappService::with_repository(
        ServiceConfig::isolated_local(true),
        clock.clone(),
        FixtureListingRepository::from_listings(listings),
    )));
    let server = RunningServer::start().await;

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
    assert_envelope(&metadata, "OK");
    assert_eq!(metadata["data"]["towns"].as_array().unwrap().len(), 33);

    let (status, login) = post_json(
        &client,
        &server,
        "/api/miniapp/v1/auth/dev-session",
        &json!({"phone":synthetic_phone(),"contact_confirmed":true}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_envelope(&login, "OK");
    let token = login["data"]["session_token"].as_str().unwrap().to_owned();

    let draft = demand_json("松山湖", "factory", 1350, 1650, Some(true), None);
    let (_, interpreted) = post_json(
        &client,
        &server,
        "/api/miniapp/v1/demands/interpret",
        &json!({"session_token":token,"draft":draft}),
    )
    .await;
    assert_envelope(&interpreted, "OK");
    let demand = interpreted["data"]["demand"].clone();

    let (_, matched) = post_json(
        &client,
        &server,
        "/api/miniapp/v1/matches",
        &json!({"session_token":token,"demand":demand}),
    )
    .await;
    assert_envelope(&matched, "OK");
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
        .expect("eligible recommendation")["listing"]["listing_id"]
        .as_str()
        .unwrap();
    let lead_body = json!({
        "session_token":token,
        "submission":{"demand":demand,"recommended_listing_ids":[selected],
        "source_channel":"miniapp_ai_demand","idempotency_key":"dioxus-http-e2e-001"}
    });
    let first = post_json(&client, &server, "/api/miniapp/v1/leads", &lead_body)
        .await
        .1;
    let duplicate = post_json(&client, &server, "/api/miniapp/v1/leads", &lead_body)
        .await
        .1;
    assert_envelope(&first, "OK");
    assert_envelope(&duplicate, "OK");
    assert_eq!(
        first["data"]["lead_number"],
        duplicate["data"]["lead_number"]
    );

    let no_auth = post_json(
        &client,
        &server,
        "/api/miniapp/v1/matches",
        &json!({"session_token":"","demand":lead_body["submission"]["demand"]}),
    )
    .await
    .1;
    assert_envelope(&no_auth, "UNAUTHENTICATED");
    let invalid_session = post_json(
        &client,
        &server,
        "/api/miniapp/v1/matches",
        &json!({"session_token":"invalid-session","demand":lead_body["submission"]["demand"]}),
    )
    .await
    .1;
    assert_envelope(&invalid_session, "UNAUTHENTICATED");

    let unverified_match = post_json(
        &client,
        &server,
        "/api/miniapp/v1/matches",
        &json!({"session_token":token,"demand":demand_json("虎门","factory",2400,2600,None,None)}),
    )
    .await
    .1;
    assert_envelope(&unverified_match, "OK");
    assert!(unverified_match["data"]["matches"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["listing"]["listing_id"] != "demo-factory-006"));

    let hard = submit_for_listing(
        &client,
        &server,
        &token,
        demand_json("长安", "factory", 900, 1100, Some(true), Some("hard")),
        "demo-factory-005",
        "dioxus-http-hard-001",
    )
    .await;
    assert_envelope(&hard, "HARD_CONDITION_NOT_MET");

    let mut power_unverified = demand_json("塘厦", "warehouse", 1700, 1900, None, None);
    power_unverified["constraints"]["power_capacity_kva"] = json!(300);
    power_unverified["constraint_priorities"] = json!([{"key":"power_capacity","level":"hard"}]);
    let unverified = submit_for_listing(
        &client,
        &server,
        &token,
        power_unverified,
        "demo-warehouse-005",
        "dioxus-http-unverified-001",
    )
    .await;
    assert_envelope(&unverified, "HARD_CONDITION_UNVERIFIED");

    let preference = submit_for_listing(
        &client,
        &server,
        &token,
        demand_json("长安", "factory", 900, 1100, Some(true), Some("preference")),
        "demo-factory-005",
        "dioxus-http-preference-001",
    )
    .await;
    assert_envelope(&preference, "OK");
    let unspecified = submit_for_listing(
        &client,
        &server,
        &token,
        demand_json(
            "长安",
            "factory",
            900,
            1100,
            Some(true),
            Some("unspecified"),
        ),
        "demo-factory-005",
        "dioxus-http-unspecified-001",
    )
    .await;
    assert_envelope(&unspecified, "OK");

    let malformed = client
        .post(server.url("/api/miniapp/v1/matches"))
        .header("content-type", "application/json")
        .body("{not-json")
        .send()
        .await
        .expect("malformed request");
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    assert!(!malformed.bytes().await.expect("malformed body").is_empty());

    let wrong_method = client
        .get(server.url("/api/miniapp/v1/matches"))
        .send()
        .await
        .expect("method request");
    assert_eq!(wrong_method.status(), StatusCode::METHOD_NOT_ALLOWED);
    let missing = client
        .get(server.url("/api/not-part-of-miniapp"))
        .send()
        .await
        .expect("404 request");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_envelope(&missing.json().await.expect("404 envelope"), "NOT_FOUND");

    clock.set_epoch_seconds(clock.now_epoch_seconds() + 2 * 60 * 60);
    let expired = post_json(
        &client,
        &server,
        "/api/miniapp/v1/matches",
        &json!({"session_token":token,"demand":lead_body["submission"]["demand"]}),
    )
    .await
    .1;
    assert_envelope(&expired, "SESSION_EXPIRED");

    server.stop().await;
}
