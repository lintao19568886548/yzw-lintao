# Phase 1 Miniapp BFF 隔离 HTTP E2E

运行命令：`cargo test --features server miniapp_http_e2e`。

测试只在进程内启动 Axum Router，监听 `127.0.0.1` 的操作系统随机端口。Dioxus Server Function 和测试 Router 调用同一个 `MiniappService`；测试没有复制匹配、校验、认证或线索逻辑。

隔离配置固定使用 `LocalDemandInterpreter`、内嵌虚构 fixture、内存会话和内存线索。它不读取 `YIZU_MINIAPP_AI_PROVIDER` 或百炼 Key，也不初始化根应用、SpacetimeDB、对象存储、短信、设备服务或其他园区模块，因此不存在外部网络适配器路径。

覆盖场景：元数据、开发会话、中文解析、确认需求、推荐、满足硬条件的线索创建、幂等重试、未登录、过期、非法 JSON、硬条件不满足、硬条件数据缺失、客户端伪造满足字段、伪造房源 ID、404、显式开发认证开关，以及 graceful shutdown 后端口重新绑定。
