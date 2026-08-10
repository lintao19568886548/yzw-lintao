# Phase 1 复核问题整改前后对照

| 复核问题 | 整改前 | v2 整改后 | 证据 |
| --- | --- | --- | --- |
| 镇街不完整 | 首页 10 项、确认页 14 项，各自硬编码 | Rust metadata 权威；两页共用 Store；33 项完整降级且失败保留选择 | `metadata.spec.ts`、Rust metadata/非法镇街测试 |
| 预算单位混乱 | 首页元、确认页直接编辑分，使用浮点乘法 | UI 只显示元；字符串整数安全转换；API/Rust 保留整数分 | `money.ts`、`money.spec.ts`、parser 小数测试 |
| 硬条件可绕过 | 依赖自由文本中文关键词 | `ConstraintKey` + `hard/preference`；结果分类；提交服务端重算；未满足/不可验证分别拒绝 | matching/service/E2E 测试 |
| 固定 2026 日期 | 匹配代码写死 3 个截止日期 | 注入 `Clock`；Asia/Shanghai；动态 immediate/30/90/明确日期 | `clock.rs`、日期边界测试、源码扫描 |
| hydrate 陈旧对象 | 确认页保存 hydrate 前对象；首页表单不重同步 | `storeToRefs` + 共用 composable；统一恢复顺序；v2 缓存与损坏清理 | `flow-restore.spec.ts` |
| 没有真实 HTTP E2E | 只有领域与前端契约单测 | 127.0.0.1 随机端口、5 路由、同一 Service、真实 JSON、负向场景和端口释放 | `http_e2e.rs`、2 项 E2E |

以上 6 项已整改，不再列为接受的已知限制。仍保留的真实限制只包括本地演示数据/内存持久化、规则解析能力和明确排除的后续生产能力。
