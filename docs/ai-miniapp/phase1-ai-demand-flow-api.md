# Phase 1 AI 找房 BFF API

## 范围与传输

本切片复用现有 Dioxus Fullstack Server Functions。客户端只访问 Rust BFF，不直接访问 SpacetimeDB。所有字段使用 snake_case JSON，路由前缀为 `/api/miniapp/v1`。完整机器可读契约见 `phase1-ai-demand-flow.openapi.yaml`。

统一响应：

```json
{
  "code": "OK",
  "message": "业务结果说明",
  "request_id": "miniapp-...",
  "data": {},
  "errors": []
}
```

失败时 `data` 为 `null`，`errors` 为 `{ "field", "message" }` 数组。服务端生成 `request_id`；面向客户端的错误不包含密钥、内部 URL 或堆栈。请求序列化后上限为 64 KiB，原始需求最多 1000 个字符，补充字段另有长度和枚举校验。

## 路由

| Method | Path | Request | `data` | 说明 |
| --- | --- | --- | --- | --- |
| POST | `/auth/dev-session` | `DevSessionRequest` | `DevSessionResponse` | 仅 debug 构建且 `YIZU_MINIAPP_DEV_AUTH_ENABLED=true` 可用；release 始终拒绝。 |
| POST | `/demands/interpret` | `InterpretDemandRequest` | `DemandInterpretation` | 要求有效会话；按 `local`/`bailian` 解析。 |
| POST | `/matches` | `MatchRequest` | `MatchResponse` | 服务端硬过滤和七维评分，返回最多 10 条。 |
| POST | `/leads` | `SubmitLeadRequest` | `LeadRecord` | 要求登录、已确认联系方式、完整需求、推荐房源和幂等键。 |
| GET | `/metadata/options` | 无 | `MetadataOptions` | 返回东莞镇街、类型、租金单位、核验等级及演示标识。 |

Dioxus Server Function 会以 JSON 反序列化单一请求对象；响应体为统一 envelope。

## 核心枚举

- `space_type`: `factory | warehouse | office`
- `rent_unit`: `yuan_per_month | yuan_per_square_metre_month`
- `verification_level`: `l0 | l1 | l2 | l3`
- `status`: `pending_assignment`
- `source_channel`: 当前只接受 `miniapp_ai_demand`

## DemandDraft

```json
{
  "raw_text": "想在松山湖附近找1500平方米左右的厂房，需要货梯和较大用电容量。",
  "constraints": {
    "space_type": "factory",
    "target_towns": ["松山湖"],
    "area_min_sqm": 1350,
    "area_max_sqm": 1650,
    "rent_min_cents": null,
    "rent_max_cents": null,
    "rent_unit": null,
    "move_in_time": null,
    "floor_preference": null,
    "needs_freight_elevator": true,
    "elevator_min_tons": null,
    "power_capacity_kva": null,
    "fire_requirement": null,
    "logistics_requirement": null,
    "loading_requirement": null,
    "accepts_sublease": null,
    "other_notes": null
  },
  "hard_conditions": [],
  "preference_conditions": ["需要货梯"],
  "missing_fields": [],
  "ai_confidence": 0.78
}
```

生成线索前必须有明确的 `space_type`、至少一个 `target_towns`、合法的面积上下限，并由用户点击“联系顾问”。空值用 JSON `null`，不要省略必备结构。

## 认证与隐私

开发会话请求包含手机号和 `contact_confirmed`。响应只返回掩码号码、随机会话标识、过期时间和 `local_demo=true`。会话与线索都只存在内存中；服务端日志脱敏函数会移除连续联系方式和认证凭据。

百炼适配器只在 Rust `server` feature 下编译调用；发送前再次移除联系方式。Key 只读取 `ALIYUN_BAILIAN_KEY`，客户端契约中没有 Key、模型 URL 或透传认证字段。

## 匹配语义

硬过滤顺序为出租状态 → AI 推荐资格 → 空间类型 → 镇街 → 面积。严格面积无结果时仅放宽到需求区间的 ±20%；仍为空时返回是否接受相邻镇街的建议，但服务端不会自动扩镇街。

七维分数及权重固定为：位置 0.20、空间 0.20、成本 0.20、生产 0.15、物流 0.10、合规 0.10、入驻 0.05。核验、自营、更新时间和稳定 ID 只用于同分排序。每条结果提供 `overall_score`、7 个 `dimension_scores`、3 条主要理由、未满足条件、数据缺口及面积放宽标识。

## 线索幂等

`idempotency_key` 长度 16～128，只允许 ASCII 字母、数字、下划线和短横线。同一内存进程内，相同用户会话和幂等键返回第一次创建的同一条 `LeadRecord`，不重复创建。提交的房源 ID 必须仍位于服务端针对需求计算出的推荐集合中，且不得违反硬条件。

成功记录包含需求快照、推荐房源 ID、来源渠道、`pending_assignment`、`sla_minutes=15` 与 `temporary_storage=true`。本轮不执行真实分配或任何外部通知。

## 典型错误码

| Code | 含义 |
| --- | --- |
| `VALIDATION_ERROR` | 字段缺失、非法枚举、范围或文本校验失败。 |
| `REQUEST_TOO_LARGE` | 请求超过 64 KiB。 |
| `DEV_AUTH_DISABLED` | 模拟登录未显式开启或处于 release 构建。 |
| `UNAUTHENTICATED` | 会话不存在。 |
| `SESSION_EXPIRED` | 会话已过期。 |
| `CONTACT_NOT_CONFIRMED` | 联系方式未确认。 |
| `AI_PROVIDER_INVALID` | Provider 不是 `local` 或 `bailian`。 |
| `AI_INTERPRETATION_ERROR` | AI 配置、网络、非法 JSON 或 Schema 校验失败。 |
| `LISTING_NOT_RECOMMENDED` | 提交的房源不属于服务端本次推荐。 |
| `HARD_CONDITION_NOT_MET` | 选中房源违反硬条件。 |

HTTP 传输失败与业务 `code` 分开处理；客户端还映射超时、网络断开和无效响应格式。
