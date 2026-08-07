//! 与实时服务的连接自愈：断线后按退避节奏自动重连，网络恢复时立即重试。
//!
//! SpacetimeDB SDK 断开后不会自己重连，`on_disconnect` 只把状态置成
//! Disconnected 就结束了。没有这层，笔记本合盖、切到 4G、服务端重启之后
//! 页面就一直停在断线状态，只能靠用户手动刷新。

use dioxus::prelude::*;

use crate::state::{ConnectionPhase, WorkspaceState};

/// 退避策略在服务端目标下没有调用方，但仍然由单元测试覆盖。
#[cfg(not(target_arch = "wasm32"))]
macro_rules! tested_on_host {
    ($item:item) => {
        #[allow(dead_code)]
        $item
    };
}

#[cfg(target_arch = "wasm32")]
macro_rules! tested_on_host {
    ($item:item) => {
        $item
    };
}

tested_on_host! {
    /// 退避序列（毫秒）。
    ///
    /// 前几档要快——绝大多数断线是服务端重启或网络瞬断，一两秒后就能连上。
    /// 封顶 30 秒是为了长时间离线时别把电池和服务端都耗着；网络真正恢复时由
    /// `online` / `visibilitychange` 事件立刻插队，不必等满这一档。
    const RECONNECT_DELAYS_MS: [u32; 6] = [1_000, 2_000, 4_000, 8_000, 15_000, 30_000];
}

tested_on_host! {
    /// 第 `attempt` 次重试（从 0 开始）之前要等待的毫秒数。
    fn reconnect_delay_ms(attempt: u32) -> u32 {
        let last = RECONNECT_DELAYS_MS.len() - 1;
        RECONNECT_DELAYS_MS[(attempt as usize).min(last)]
    }
}

tested_on_host! {
    /// 断线状态：这两个值意味着不会再有数据进来，必须重连。
    fn needs_reconnect(phase: ConnectionPhase) -> bool {
        matches!(
            phase,
            ConnectionPhase::Disconnected | ConnectionPhase::Failed
        )
    }
}

#[cfg(target_arch = "wasm32")]
async fn sleep_ms(milliseconds: u32) {
    use wasm_bindgen::JsCast;

    let (sender, receiver) = futures_channel::oneshot::channel::<()>();
    let callback = wasm_bindgen::closure::Closure::once_into_js(move || {
        let _ = sender.send(());
    });
    let scheduled = web_sys::window().is_some_and(|window| {
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                callback.unchecked_ref::<js_sys::Function>(),
                milliseconds as i32,
            )
            .is_ok()
    });
    if scheduled {
        let _ = receiver.await;
    }
    // 定时器没排上就直接返回：这一轮退避失效，但重连本身照常进行。
}

/// 挂上自动重连。只需在应用根组件调用一次。
///
/// 服务端和测试目标下是空实现，自动重连仅由浏览器客户端负责。
#[cfg(target_arch = "wasm32")]
pub fn use_auto_reconnect(state: WorkspaceState) {
    use super::{connect_workspace, ConnectionConfig};

    // 连续失败次数，决定下一次等多久；连上就清零。
    let mut attempt = use_signal(|| 0_u32);
    // 网络恢复事件通过它插队：值一变就重新触发下面的 effect。
    let wake = use_signal(|| 0_u32);

    install_network_wakeups(wake);

    use_effect(move || {
        // 订阅这两个信号。attempt 只读 peek，否则在 effect 里写它会自激。
        let phase = (state.phase)();
        let _ = wake();

        if !needs_reconnect(phase) {
            if phase == ConnectionPhase::Connected && *attempt.peek() != 0 {
                attempt.set(0);
            }
            return;
        }

        let failures = *attempt.peek();
        let delay = reconnect_delay_ms(failures);
        attempt.set(failures.saturating_add(1));
        spawn(async move {
            sleep_ms(delay).await;
            // 等待期间可能已经连上了（比如用户手动登录触发了连接）。
            if !needs_reconnect(*state.phase.peek()) {
                return;
            }
            // 并发的重复尝试由 connect_workspace 自己的进行中标记挡掉。
            let _ = connect_workspace(ConnectionConfig::default(), state).await;
        });
    });
}

#[cfg(not(target_arch = "wasm32"))]
pub fn use_auto_reconnect(_state: WorkspaceState) {}

/// 浏览器恢复网络、或标签页重新可见时，立刻叫醒重连循环。
///
/// 只靠退避的话，合盖一晚上再打开要等满 30 秒才恢复；这两个事件让它变成秒回。
#[cfg(target_arch = "wasm32")]
fn install_network_wakeups(wake: Signal<u32>) {
    use wasm_bindgen::JsCast;

    use_hook(move || {
        let Some(window) = web_sys::window() else {
            return;
        };
        for event in ["online", "visibilitychange", "focus"] {
            let mut wake = wake;
            let callback = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
                let ticks = *wake.peek();
                wake.set(ticks.saturating_add(1));
            });
            let _ = window.add_event_listener_with_callback(
                event,
                callback.as_ref().unchecked_ref::<js_sys::Function>(),
            );
            // 监听器与页面同生命周期，交给 JS 持有，不再回收。
            callback.forget();
        }
    });
}

/// 服务端目标不安装浏览器事件监听器。
#[cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
fn install_network_wakeups(_wake: Signal<u32>) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 退避时长逐次拉长并封顶() {
        assert_eq!(reconnect_delay_ms(0), 1_000);
        assert_eq!(reconnect_delay_ms(1), 2_000);
        assert_eq!(reconnect_delay_ms(3), 8_000);
        // 超出序列长度后停在最后一档，不会越界也不会无限增长。
        assert_eq!(reconnect_delay_ms(5), 30_000);
        assert_eq!(reconnect_delay_ms(50), 30_000);
        assert_eq!(reconnect_delay_ms(u32::MAX), 30_000);
    }

    #[test]
    fn 只有断开和失败才触发重连() {
        assert!(needs_reconnect(ConnectionPhase::Disconnected));
        assert!(needs_reconnect(ConnectionPhase::Failed));
        // 连接中已经有尝试在跑，再排一次只会互相抢。
        assert!(!needs_reconnect(ConnectionPhase::Connecting));
        assert!(!needs_reconnect(ConnectionPhase::Syncing));
        assert!(!needs_reconnect(ConnectionPhase::Connected));
    }
}
