# Phase 1 v3 真实 Dioxus Server Function HTTP E2E

## 注册方式

`src/services/miniapp/api.rs` 中 4 个 `#[post]` 和 1 个 `#[get]` 宏把实际函数提交到 Dioxus `ServerFunction` inventory。测试从该 inventory 核对路径和方法，然后在 `Router<FullstackState>` 上调用 `register_server_functions()`，最后使用 `FullstackState::headless()` 提供无需根组件的运行状态。

测试没有为 5 个业务路径编写任何替代 Axum handler。唯一自定义 fallback 只用于验证未注册路径 404。

## 请求体兼容

4 个 POST 函数使用 Dioxus fullstack 的 `Json<Request>` extractor，所以真实 HTTP 请求体是 request 对象本身。真实 `reqwest` 测试与 `apps/yizu-client/src/api/miniapp.ts` 都发送这一裸 JSON；OpenAPI 同样直接引用对应 request schema。

## 隔离与覆盖

- 监听 `127.0.0.1:0`；使用 `reqwest` 真实发包。
- 注入 `FixedClock`、本地固定解析器、内存会话/线索仓库、脱敏房源 fixture 与隔离开发认证配置。
- 不初始化根应用、SpacetimeDB、百炼、短信、对象存储、生产数据库或生产环境变量。
- 覆盖 5 条路径、方法、200/400/404/405 状态、JSON Content-Type、完整信封、登录→解析→匹配→线索、非法 JSON、缺失/无效/过期 session、未核验房源过滤、硬条件不满足/无法核验、偏好不阻断、unspecified 不阻断、幂等重复提交及开发认证双门禁。
- graceful shutdown 后在原端口重新绑定，证明监听端口已释放。

执行命令：

```powershell
cargo test --features server dioxus_server_function_http_e2e
```

真实结果：运行 1 项，1 passed，0 failed，退出码 0。

```text
DIOXUS_SERVER_FUNCTION_HTTP_E2E=PASS
```
