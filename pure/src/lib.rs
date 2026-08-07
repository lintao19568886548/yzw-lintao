//! `#[pure]`：纯函数标记与编译期检查。
//!
//! # 原理
//!
//! 两端的副作用都走「能力传递」的门，没有全局句柄，所以「签名与函数体不出现
//! 这些门」在本代码库里几乎等价于纯函数：
//!
//! - **服务端（SpacetimeDB 模块）**：副作用只有一扇门 `ctx`。数据库、时钟、
//!   随机数都必须经由 `ReducerContext` / `ViewContext` 获得。
//! - **前端（Dioxus）**：副作用的门是 Hook（`use_*`）、`Signal` 响应式状态、
//!   `spawn` 异步任务与浏览器 API（`web_sys` / `js_sys`）。页面 `model.rs`
//!   层的函数不接触这些门，才能脱离组件树单独测试。
//!
//! 本宏把 ARCHITECTURE.md §2.2 的分层判据（「它需要 ctx / Hook 吗」）
//! 从人工纪律变成编译错误。
//!
//! # 检查方式
//!
//! 对被标记项的整个 token 流（签名 + 函数体）做递归扫描，出现禁用标识符即在
//! 该 token 的位置产生 `compile_error!`。扫的是原始 token，所以嵌套在
//! `format!` 等宏调用里的标识符同样逃不掉；生命周期 `'static` 中的 `static`
//! 会被正确跳过，不会误伤错误文案里的 `&'static str`。
//!
//! # 局限（如实声明）
//!
//! - **纯度不传递**：本宏不检查被调函数。约定是对被调的纯函数同样标记
//!   `#[pure]`；命名约定由两侧测试兜底——服务端
//!   `所有名为_check_或_normalize_的函数都已标记为纯函数`，前端
//!   `model_rs_的公开函数都已标记为纯函数或按时钟命名`（`src/pages/mod.rs`）。
//! - 经结构体字段走私 `ctx` 引用的病态写法拦不住（本仓库无此形态，靠评审）。
//! - 堆分配与 panic 不视为副作用——与项目对「纯」的定义（可脱离 ctx 单测、
//!   同输入同输出）一致。

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree};
use quote::quote_spanned;

/// 禁用标识符与给违规者看的原因。
const DENIED: &[(&str, &str)] = &[
    ("ctx", "ctx 是副作用的唯一入口，纯函数不能接触它"),
    ("ReducerContext", "纯函数的签名与函数体不能出现 ReducerContext"),
    ("ViewContext", "纯函数的签名与函数体不能出现 ViewContext"),
    ("AnonymousViewContext", "纯函数的签名与函数体不能出现 AnonymousViewContext"),
    ("LocalReadOnly", "读数据库同样是副作用，纯函数不能接触 LocalReadOnly"),
    ("DbContext", "纯函数的签名与函数体不能出现 DbContext"),
    ("unsafe", "纯函数不允许 unsafe"),
    ("static", "纯函数不允许声明 static（`&'static` 生命周期不受影响）"),
    ("thread_local", "纯函数不允许线程局部状态"),
    ("spacetimedb", "纯函数不能调用 spacetimedb API"),
    ("log", "日志输出是可观测副作用，纯函数不能写日志"),
    ("println", "标准输出是副作用"),
    ("eprintln", "标准错误输出是副作用"),
    ("print", "标准输出是副作用"),
    ("eprint", "标准错误输出是副作用"),
    ("dbg", "dbg! 输出是副作用"),
    ("SystemTime", "系统时钟在模块里不可用也不确定，时间请作为参数传入"),
    ("Instant", "系统时钟在模块里不可用也不确定，时间请作为参数传入"),
    // —— 前端（Dioxus）侧的门 ——
    ("Signal", "读写 Signal 是响应式状态操作，属于组件层，纯函数不能接触"),
    ("ReadOnlySignal", "读 Signal 会订阅响应式更新，属于组件层，纯函数不能接触"),
    ("GlobalSignal", "全局响应式状态是副作用，纯函数不能接触"),
    ("spawn", "启动异步任务或线程是副作用"),
    ("navigator", "路由跳转是副作用"),
    ("dioxus", "纯函数不应依赖 UI 框架"),
    ("web_sys", "浏览器 API 属于外部世界，纯函数不能调用"),
    ("js_sys", "浏览器 API 属于外部世界，纯函数不能调用"),
    ("wasm_bindgen", "浏览器 API 属于外部世界，纯函数不能调用"),
    ("gloo_net", "网络请求是副作用"),
    ("reqwest", "网络请求是副作用"),
    ("tokio", "异步运行时属于命令式外壳，纯函数不能依赖"),
];

/// 以 `use_` 开头的标识符按 Dioxus 约定是 Hook，一律禁用。
const HOOK_PREFIX: &str = "use_";
const HOOK_REASON: &str = "以 use_ 开头的是 Dioxus Hook，Hook 只能在组件里调用，纯函数不能使用";

/// 标记一个函数为纯函数，并在编译期做禁用标识符检查。
///
/// ```ignore
/// #[pure_function::pure]
/// fn check_issued_amount(issued: Option<bool>, amount: Option<i64>) -> Result<(), String> { … }
/// ```
#[proc_macro_attribute]
pub fn pure(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item: TokenStream2 = item.into();
    let attr: TokenStream2 = attr.into();
    let mut errors = TokenStream2::new();
    if !attr.is_empty() {
        errors.extend(quote_spanned! {Span::call_site()=>
            compile_error!("#[pure] 不接受参数");
        });
    }
    let mut violations = Vec::new();
    scan(item.clone(), &mut violations);
    for (span, message) in violations {
        errors.extend(quote_spanned! {span=> compile_error!(#message); });
    }
    errors.extend(item);
    errors.into()
}

/// 递归扫描 token 流，收集禁用标识符及其位置。
fn scan(stream: TokenStream2, violations: &mut Vec<(Span, String)>) {
    // 上一个 token 是否为生命周期引号 `'`——若是，紧随的标识符属于生命周期名。
    let mut after_lifetime_quote = false;
    for tree in stream {
        match tree {
            TokenTree::Group(group) => {
                after_lifetime_quote = false;
                scan(group.stream(), violations);
            }
            TokenTree::Punct(punct) => {
                after_lifetime_quote = punct.as_char() == '\'';
            }
            TokenTree::Ident(ident) => {
                if after_lifetime_quote {
                    after_lifetime_quote = false;
                    continue;
                }
                let name = ident.to_string();
                if let Some((_, reason)) = DENIED.iter().find(|(denied, _)| *denied == name) {
                    violations.push((ident.span(), format!("#[pure] 违规：`{name}`——{reason}")));
                } else if name.starts_with(HOOK_PREFIX) {
                    violations.push((ident.span(), format!("#[pure] 违规：`{name}`——{HOOK_REASON}")));
                }
            }
            TokenTree::Literal(_) => {
                after_lifetime_quote = false;
            }
        }
    }
}
