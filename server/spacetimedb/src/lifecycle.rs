//! 连接生命周期钩子。
//!
//! # 为什么是一张常量表
//!
//! `client_connected` 里堆的是各域的**自愈初始化**：账号一连上来就为它的租户
//! 补齐该有的菜单、权限码。这样发布流水线不需要任何人工迁移步骤，补齐之后
//! 每次连接只剩一次索引查找。
//!
//! 问题是**每加一个域就要在这里多写一行，而没有任何东西提醒你该加**——漏了
//! 不会报错，只会让那个租户少一项授权，等用户报「我这里怎么没有这个菜单」
//! 才被发现。所以做成 [`CONNECT_HOOKS`] 一张常量表加一条完备性测试，形状与
//! [`crate::reducers::rental::park::deletion::PARK_CHILDREN`] 一致。
//!
//! # 为什么不做成事件总线
//!
//! 总线换来的是「运行时能挂上编译期未知的消费者」，而这个能力用不上：交付
//! 模型是一客户一容器、同一份二进制（`docs/交付与部署方案.md`），消费者永远
//! 在编译期已知。静态表在扩展性上不输，还额外拿到编译期检查、零运行时开销
//! 和直白的栈追踪。
//!
//! 真正需要解耦的两类场景平台已经给了：**订阅**负责「写入 → 谁关心谁自己收」，
//! **调度表**（`user_session_sweep` 等）负责「现在提交 → 稍后异步反应」。

use spacetimedb::{DbContext, ReducerContext};

use crate::access;
use crate::reducers::device::gateway::set_gateway_online;
use crate::reducers::platform::center::auth::password::ensure_session_sweeper;
use crate::reducers::platform::navigation::catalog::ensure_default_permission_catalog;
use crate::reducers::platform::permissions::hr_manage::ensure_hr_manage_grants;
use crate::reducers::platform::permissions::maintenance_inspect::ensure_maintenance_inspect_code;

/// 一项按租户执行的自愈初始化。必须幂等——每次连接都会跑一遍。
type ConnectHook = fn(&ReducerContext, &str);

/// 账号连上来时为其租户补齐的初始化动作。
///
/// **新增一个域的自愈初始化 = 在这里加一行。** 名字只用于本文件的完备性测试
/// 与排查，不参与任何逻辑。
pub(crate) const CONNECT_HOOKS: &[(&str, ConnectHook)] = &[
    ("菜单目录", ensure_default_permission_catalog),
    ("人事权限码", ensure_hr_manage_grants),
    ("巡检权限码", ensure_maintenance_inspect_code),
];

/// Module 首次发布时执行。
///
/// 会话清理器挂在这里而不进 [`CONNECT_HOOKS`]：它是全局的定时任务，不按租户
/// 分，签名也不同（不需要 `customer_id`）。
#[spacetimedb::reducer(init)]
pub fn init(ctx: &ReducerContext) {
    ensure_session_sweeper(ctx);
    log::info!("易租服务端 Module 已初始化");
}

/// 客户端建立连接时按租户补齐初始化；这里不自动创建业务用户。
#[spacetimedb::reducer(client_connected)]
pub fn client_connected(ctx: &ReducerContext) {
    log::info!("客户端已连接：{}", ctx.sender());
    // 边缘计算设备的在线状态就是连接生死，不设心跳表：心跳每 30 秒写一次库，
    // 每次写入都会推给所有订阅者、触发一轮界面重渲染，而这期间什么都没发生。
    // 连接钩子天然只在状态真的翻转时触发，一天几次而不是几万次。
    //
    // 设备不是人，没有租户级的菜单与权限码要补，所以认出来就直接返回。
    if set_gateway_online(ctx, true) {
        return;
    }
    // 租户解析只做一次。`current_customer_id` 内部已经是「会话优先、退回身份
    // 绑定」的兜底（见 `access::current_center_user_id`），原先菜单目录那段手写的
    // `user_identity → center_user` 链是它的真子集——合并之后，持有有效会话但
    // 没有常驻身份绑定的账号也能被补齐，覆盖面只增不减。
    let Some(customer_id) = access::current_customer_id(ctx.db_read_only(), ctx.sender(), Some(ctx.timestamp))
    else {
        return;
    };
    for (_, hook) in CONNECT_HOOKS {
        hook(ctx, &customer_id);
    }
}

/// 客户端断开连接时把边缘计算设备标成离线。
#[spacetimedb::reducer(client_disconnected)]
pub fn client_disconnected(ctx: &ReducerContext) {
    log::info!("客户端已断开：{}", ctx.sender());
    set_gateway_online(ctx, false);
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::Path};

    /// 两条测试都只读源码，**不在运行期引用 `CONNECT_HOOKS`**。
    ///
    /// 引用那个常量会把三个 `ensure_*` 的函数指针拉进测试二进制，连带要求
    /// `datastore_insert_bsatn` 这些宿主符号——它们只在 WASM 运行时存在，宿主
    /// 目标下直接链接失败。读源码既绕开了这一点，也让断言能报出「谁漏了」。
    #[test]
    fn 钩子名字不重复也不为空() {
        let hooks = parse_hooks();
        assert!(!hooks.is_empty(), "没解析到任何钩子，提取规则可能失效了");
        let names = hooks
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), hooks.len(), "钩子名字有重复：{names:?}");
        assert!(
            names.iter().all(|name| !name.trim().is_empty()),
            "钩子名字不能为空，排查时靠它认人"
        );
    }

    /// 接住编译器接不住的那一步：写了一个按租户自愈的 `ensure_*`，却忘了挂到
    /// [`CONNECT_HOOKS`] 上。
    ///
    /// 漏挂不会报错，只会让那个租户少一项授权，要等用户报「我这里怎么没有这个
    /// 菜单」才被发现——这正是当初把它做成常量表要防的事。
    ///
    /// 判据是**可见性加签名**：`pub(crate) fn ensure_xxx(ctx: &ReducerContext,
    /// customer_id: &str)`。两个条件都必要：
    ///
    /// - **签名**排除了 `ensure_session_sweeper(ctx)`，它是全局定时任务，不按租户分；
    /// - **`pub(crate)`** 排除了各模块自用的私有 `ensure_*`（如 `legacy_sms.rs` 里
    ///   登录时补建租户的那个）。私有函数 `lifecycle.rs` 根本引用不到，本来就挂
    ///   不上去；反过来说，把一个按租户自愈的函数暴露成 `pub(crate)`，就意味着
    ///   它要被别处调用，而这个「别处」在本代码库里只有连接钩子这一处。
    #[test]
    fn 所有按租户自愈的初始化都已挂上() {
        let registered = parse_hooks()
            .into_iter()
            .map(|(_, func)| func)
            .collect::<BTreeSet<_>>();
        let actual = tenant_seeding_fns();
        assert!(
            !actual.is_empty(),
            "没扫到任何按租户自愈的函数，提取规则可能失效了"
        );
        let missing = actual.difference(&registered).collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "这些函数按租户自愈却没挂到 CONNECT_HOOKS，对应租户会静默少一项授权：{missing:?}"
        );
    }

    /// 从本文件的 `CONNECT_HOOKS` 字面量里解析出 `(名字, 函数名)`。
    fn parse_hooks() -> Vec<(String, String)> {
        let source = include_str!("lifecycle.rs");
        let start = source
            .find("CONNECT_HOOKS: &[(&str, ConnectHook)] = &[")
            .expect("找不到 CONNECT_HOOKS 定义");
        let body = &source[start..];
        let end = body.find("];").expect("CONNECT_HOOKS 定义没有结束");
        body[..end]
            .lines()
            .filter_map(|line| line.trim().strip_prefix('('))
            .filter_map(|line| line.split_once(", "))
            .map(|(name, func)| {
                (
                    name.trim().trim_matches('"').to_string(),
                    func.trim_end_matches("),").trim().to_string(),
                )
            })
            .collect()
    }

    /// 扫 `src/reducers`，取出所有
    /// `pub(crate) fn ensure_*(ctx: &ReducerContext, customer_id: &str)`。
    fn tenant_seeding_fns() -> BTreeSet<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/reducers");
        // 在运行期拼接，避免匹配到本测试自身的源码。
        let prefix = format!("fn {}", "ensure_");
        let mut found = BTreeSet::new();
        for path in rust_files(&root) {
            let source = fs::read_to_string(&path).expect("源码文件应当可读");
            // 签名可能跨行，去掉空白后再匹配。
            let compact = source
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>();
            for line in source.lines() {
                let trimmed = line.trim();
                let Some(at) = trimmed.find(&prefix) else {
                    continue;
                };
                let name = trimmed[at + 3..]
                    .split('(')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                let signature =
                    format!("pub(crate)fn{name}(ctx:&ReducerContext,customer_id:&str)");
                if compact.contains(&signature) {
                    found.insert(name);
                }
            }
        }
        found
    }

    fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir).expect("目录应当可读") {
            let path = entry.expect("目录项应当可读").path();
            if path.is_dir() {
                files.extend(rust_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
        files
    }
}
