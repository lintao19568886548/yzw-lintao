//! 合同表单的三个路由入口：新增、编辑、查看。
//!
//! 三个都落到同一个 [`ContractForm`]，差别只有「有没有现存合同」和「能不能改」。
//! 拆成三条路由而不是一条带查询参数的，是为了让权限、菜单和浏览器前进后退都按
//! 常规方式工作——「编辑合同」和「查看合同」在权限上本来就该是两件事。
//!
//! 表单原来是半屏弹窗。合同要谈的东西一路长出来（楼层、宿舍、逐块表报价、基本
//! 电费、约定费用、递增档位、合同原件），弹窗里每一项只有一列宽，四段电价和
//! 「比例＋基数」这种并排字段被挤成竖排。改成整页之后横向铺得开，而且刷新、
//! 分享链接、后退都能正常用。

use dioxus::prelude::*;

use super::form::ContractForm;
use crate::{
    components::card::{Card, CardContent, CardHeader, CardTitle},
    router::Route,
    spacetime_bindings::tenant_image_preview_type::TenantImagePreview,
    state::{ConnectionPhase, WorkspaceState},
};

/// 新增合同。
#[component]
pub fn ContractCreatePage() -> Element {
    let state = use_context::<WorkspaceState>();
    let navigator = use_navigator();
    rsx! {
        ContractForm {
            contract: None,
            parks: (state.parks)(),
            image_previews: Vec::<TenantImagePreview>::new(),
            readonly: false,
            on_close: move |_| {
                navigator.push(Route::ContractManagementPage {});
            },
            on_saved: move |_| {
                navigator.push(Route::ContractManagementPage {});
            },
        }
    }
}

/// 编辑合同。
#[component]
pub fn ContractEditPage(id: u64) -> Element {
    ContractFormRoute(ContractFormRouteProps {
        id,
        readonly: false,
    })
}

/// 查看合同。
#[component]
pub fn ContractDetailPage(id: u64) -> Element {
    ContractFormRoute(ContractFormRouteProps { id, readonly: true })
}

/// 按 id 取出合同再渲染表单；取不到时区分「还没同步完」和「真的没有」。
///
/// 两种情况给的话术不一样：连接还没建好时说"正在同步"，已连接却查不到才说
/// "不存在或没有权限"——否则用户在刚打开页面的一瞬间会看到一句吓人的错误。
#[component]
fn ContractFormRoute(id: u64, readonly: bool) -> Element {
    let state = use_context::<WorkspaceState>();
    let navigator = use_navigator();
    let contract = (state.rental_tenants)()
        .into_iter()
        .find(|row| row.rental_tenant_id == id && !row.is_deleted);

    let Some(contract) = contract else {
        let connected = (state.phase)() == ConnectionPhase::Connected;
        return rsx! {
            main { class: "page",
                Card {
                    CardHeader {
                        CardTitle {
                            if connected { "未找到合同" } else { "正在同步合同" }
                        }
                    }
                    CardContent {
                        p {
                            if connected {
                                "该合同不存在、已被删除，或当前账号没有查看权限。"
                            } else {
                                "正在等待 SpacetimeDB 推送合同数据，请稍候。"
                            }
                        }
                        Link { to: Route::ContractManagementPage {}, "返回合同管理" }
                    }
                }
            }
        };
    };

    let image_previews = (state.tenant_image_previews)()
        .into_iter()
        .filter(|image| image.rental_tenant_id == id)
        .collect::<Vec<_>>();

    rsx! {
        ContractForm {
            contract: Some(contract),
            parks: (state.parks)(),
            image_previews,
            readonly,
            on_close: move |_| {
                navigator.push(Route::ContractManagementPage {});
            },
            on_saved: move |_| {
                navigator.push(Route::ContractManagementPage {});
            },
        }
    }
}
