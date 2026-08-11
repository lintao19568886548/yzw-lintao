# Phase 1 AI 找房 API v3

机器可读契约：`docs/phase1-ai-demand-flow.openapi.yaml`。

## 传输契约

基础路径为 `/api/miniapp/v1`。POST 使用 `Content-Type: application/json`，请求体就是下表对应的 request 对象，不得增加 `{ "request": ... }` 包装层。`api.rs` 使用 Dioxus `#[post]` + `Json<T>` extractor，uni-app `miniapp.ts` 发送同一个裸对象。

| 方法 | 路径 | 请求 | 成功 data |
|---|---|---|---|
| POST | `/auth/dev-session` | `DevSessionRequest` | `DevSessionResponse` |
| POST | `/demands/interpret` | `InterpretDemandRequest` | `DemandInterpretation` |
| POST | `/matches` | `MatchRequest` | `MatchResponse` |
| POST | `/leads` | `SubmitLeadRequest` | `LeadRecord` |
| GET | `/metadata/options` | 无 | `MetadataOptions` |

所有正常业务响应都是：

```json
{
  "code": "OK",
  "message": "...",
  "request_id": "miniapp-...",
  "data": {},
  "errors": []
}
```

业务拒绝仍返回该信封，`code` 可为 `UNAUTHENTICATED`、`SESSION_EXPIRED`、`VALIDATION_ERROR`、`HARD_CONDITION_NOT_MET`、`HARD_CONDITION_UNVERIFIED` 或 `LISTING_NOT_RECOMMENDED`。Dioxus JSON extractor 在语法损坏时返回 HTTP 400；错误方法为 405；测试路由未注册路径为 404。

## 条件优先级

`ConstraintLevel` 只允许：

- `hard`：服务端硬阻断；
- `preference`：记录未满足偏好但可生成线索；
- `unspecified`：明确跳过该字段的条件评估。

同一 `ConstraintKey` 不得重复。缺少级别仅用于兼容旧调用：货梯 true 默认为 hard，其他有值字段默认为 preference。显式 unspecified 永远优先于兼容默认。

金额在 API/Rust 中使用整数分；UI 输入和显示为元。业务日期以 `Asia/Shanghai` 解释。Phase 1 数据和认证均是本地演示，不代表生产持久化或真实顾问通知。
