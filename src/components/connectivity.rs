use dioxus::prelude::*;

use crate::state::ConnectionPhase;

#[derive(Clone, PartialEq)]
enum EndpointProbe {
    Checking,
    Reachable(u16),
    BrowserOffline,
    Unreachable(String),
}

#[cfg(target_arch = "wasm32")]
async fn probe_realtime_endpoint() -> EndpointProbe {
    let Some(window) = web_sys::window() else {
        return EndpointProbe::Unreachable("无法读取浏览器网络状态".to_string());
    };

    if !window.navigator().on_line() {
        return EndpointProbe::BrowserOffline;
    }

    let origin = option_env!("YIZU_SPACETIMEDB_URI")
        .unwrap_or("https://yz.furong.org")
        .trim_end_matches('/');
    // 这里只关心「实时服务是否可达」，用 SpacetimeDB 的健康检查端点，它返回 200 空体。
    //
    // 原先探测的是 /v1/identity/websocket-token，且刻意不带凭证，于是每次探测都在
    // 控制台留下一条 401。那正是连接层校验登录令牌用的同一个端点，两者混在一起看，
    // 会让人以为登录出了问题——排查真实的令牌故障时就被它带偏过。
    let endpoint = format!("{origin}/v1/ping");

    match gloo_net::http::Request::get(&endpoint).send().await {
        Ok(response) if response.status() < 500 => EndpointProbe::Reachable(response.status()),
        Ok(response) => {
            EndpointProbe::Unreachable(format!("实时服务返回 HTTP {}", response.status()))
        }
        Err(error) => EndpointProbe::Unreachable(error.to_string()),
    }
}

#[component]
pub fn ConnectivityStatus(phase: ConnectionPhase, compact: bool) -> Element {
    let mut probe = use_signal(|| EndpointProbe::Checking);
    let mut probe_generation = use_signal(|| 0_u32);

    use_effect(move || {
        let _ = probe_generation();

        #[cfg(target_arch = "wasm32")]
        spawn(async move {
            let result = probe_realtime_endpoint().await;
            if let Ok(mut current) = probe.try_write() {
                *current = result;
            }
        });
    });

    let probe_value = probe();
    let (tone, label, detail) = match (&probe_value, phase) {
        (EndpointProbe::BrowserOffline, _) => (
            "offline",
            "设备网络离线".to_string(),
            "请检查 Wi-Fi 或蜂窝网络".to_string(),
        ),
        (EndpointProbe::Unreachable(error), _) => {
            ("error", "实时服务不可达".to_string(), error.clone())
        }
        (EndpointProbe::Reachable(status), ConnectionPhase::Connected) => (
            "online",
            "实时连接".to_string(),
            format!("服务入口正常 · HTTP {status} · WebSocket 已连接"),
        ),
        (EndpointProbe::Reachable(status), ConnectionPhase::Connecting) => (
            "pending",
            "网络正常 · 正在握手".to_string(),
            format!("服务入口正常 · HTTP {status} · 正在建立 WebSocket"),
        ),
        (EndpointProbe::Reachable(status), ConnectionPhase::Syncing) => (
            "pending",
            "网络正常 · 正在同步权限".to_string(),
            format!("服务入口正常 · HTTP {status} · WebSocket 已连接"),
        ),
        (EndpointProbe::Reachable(status), ConnectionPhase::Failed) => (
            "error",
            "网络正常 · WebSocket 异常".to_string(),
            format!("HTTP {status} 可达，但实时连接未建立"),
        ),
        (EndpointProbe::Reachable(status), ConnectionPhase::Disconnected) => (
            "offline",
            "实时连接已断开".to_string(),
            format!("服务入口正常 · HTTP {status}"),
        ),
        (EndpointProbe::Checking, ConnectionPhase::Connected) => (
            "online",
            "实时连接".to_string(),
            "WebSocket 已连接，正在检测服务入口".to_string(),
        ),
        (EndpointProbe::Checking, ConnectionPhase::Failed) => (
            "error",
            "连接失败".to_string(),
            "正在确认设备网络与服务入口".to_string(),
        ),
        (EndpointProbe::Checking, ConnectionPhase::Disconnected) => (
            "offline",
            "连接已断开".to_string(),
            "正在确认设备网络与服务入口".to_string(),
        ),
        (EndpointProbe::Checking, ConnectionPhase::Syncing) => (
            "pending",
            "正在同步权限".to_string(),
            "WebSocket 已连接，正在同步账号与权限".to_string(),
        ),
        (EndpointProbe::Checking, ConnectionPhase::Connecting) => (
            "pending",
            "正在建立连接".to_string(),
            "正在检测网络并建立 WebSocket".to_string(),
        ),
    };

    let density = if compact { "is-compact" } else { "is-expanded" };

    rsx! {
        div {
            class: "connectivity-status is-{tone} {density}",
            title: "{detail}",
            span { class: "connectivity-dot" }
            span { class: "stack-tight",
                strong { "{label}" }
                if !compact {
                    small { "{detail}" }
                }
            }
            button {
                class: "connectivity-recheck",
                r#type: "button",
                aria_label: "重新检测网络",
                onclick: move |_| *probe_generation.write() += 1,
                if compact { "检测" } else { "重新检测" }
            }
        }
    }
}
