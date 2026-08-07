# Module 短信认证说明

本目录保存 SpacetimeDB Module 内部使用的短信协议与验证码密码学逻辑。
短信登录不再依赖 Node.js、MySQL、Redis或独立短信服务。

## 目录职责

```text
server/spacetimedb/src/
├── sms/
│   ├── mod.rs       # 声明并导出短信内部能力
│   ├── crypto.rs    # 生成六位验证码并计算 HMAC 摘要
│   └── provider.rs  # 生成联麓签名并发送 HTTP 请求
├── procedures/center/auth/sms.rs
│                    # 发送验证码和验证码登录 Procedure
├── tables/center/auth/sms.rs
│                    # 短信配置和验证码私有表
└── reducers/center/auth/sms.rs
                     # 管理员配置联麓短信参数
```

`sms` 目录只处理密码学和联麓短信协议，不直接负责用户、租户、角色或登录
会话。业务流程由 `procedures/center/auth/sms.rs` 负责组织。

## 发送验证码流程

客户端通过自动生成的绑定调用：

```rust
connection.procedures.send_login_sms_code_then(
    phone_number,
    move |_, result| {
        // 根据 SmsSendResult 更新页面状态
    },
);
```

Module 执行以下操作：

1. 校验手机号是否为 11 位数字。
2. 从私有表 `sms_provider_config` 读取联麓配置。
3. 检查同一手机号是否处于 60 秒重发限制内。
4. 使用 Module 私有 `code_pepper` 派生六位验证码。
5. 计算验证码 HMAC 摘要，不保存验证码明文。
6. 在私有表 `sms_login_challenge` 中预留发送记录。
7. 结束数据库事务后调用联麓短信 HTTPS 接口。
8. 发送成功后把验证码设置为 5 分钟有效。
9. 发送失败时删除本次预留记录。

HTTP 请求期间不能保持数据库事务开启。Procedure 会在发送前后分别开启短事务，
避免第三方接口阻塞数据库事务。

## 验证码登录流程

客户端调用：

```rust
connection.procedures.login_with_sms_code_then(
    phone_number,
    code,
    remember_me,
    move |_, result| {
        // 根据 SmsLoginResult 更新页面状态
    },
);
```

Module 在事务中执行：

1. 根据手机号读取 `sms_login_challenge`。
2. 检查验证码是否属于当前调用者的 SpacetimeDB Identity。
3. 检查短信发送状态和 5 分钟有效期。
4. 使用用户输入重新计算 HMAC 摘要。
5. 摘要不同则增加错误次数，累计 5 次后删除验证码。
6. 摘要相同则立即删除验证码，防止重复使用。
7. 查找或创建中心用户和 `public` 租户业务用户。
8. 建立中心用户、租户用户和普通 `User` 角色关系。
9. 为当前 SpacetimeDB Identity 创建登录会话。

手机号不存在时会自动创建账号，因此短信入口同时承担登录和注册功能。

## 私有表

### `sms_provider_config`

保存 Module 调用联麓短信需要的私有配置：

- API 地址
- AppId
- 商户号
- 接口版本
- MD5 或 HMACSHA256 签名算法
- 联麓短信密钥
- 模板编号和短信类型
- 验证码 `code_pepper`
- 有效期、重发间隔和最大错误次数

该表不是 `public` 表，不会发送到客户端订阅。

### `sms_login_challenge`

保存一次性登录挑战：

- 手机号
- 发起请求的 SpacetimeDB Identity
- 验证码 HMAC 摘要
- 是否仍在发送
- 已输入错误次数
- 发送时间和过期时间

该表同样是私有表，不保存验证码明文。

## 验证码为什么不可直接使用 Procedure 随机数

SpacetimeDB 2.6.1 的 `ProcedureContext::rng()` 由调用时间戳播种。时间戳是可观察
信息，因此该随机源本身不适合直接生成安全验证码。

当前实现把时间戳、调用者 Identity 和随机值放入 HMAC，并使用只有 Module 私有表
能够读取的 `code_pepper` 作为密钥：

```text
验证码 = HMAC(code_pepper, 时间戳 + Identity + nonce) 的六位数字映射
```

即使调用者知道时间戳和自己的 Identity，没有 `code_pepper` 也无法预测验证码。

## 配置方式

管理员通过 Reducer 写入配置：

```text
upsert_sms_provider_config(
    api_host,
    app_id,
    merchant_id,
    version,
    sign_type,
    secret_key,
    template_id,
    message_type,
    code_pepper
)
```

配置要求：

- `api_host` 必须使用 HTTPS。
- `sign_type` 只能是 `MD5` 或 `HMACSHA256`。
- `code_pepper` 至少 32 个字符，并且不能与前端共享。
- 不要把生产密钥写入 Git、README、客户端代码或公开表。

可以只查询非敏感字段确认配置是否存在：

```bash
spacetime sql --server self-hosted yizu-server-yz18m \
  "SELECT config_key, api_host, version, sign_type, template_id, updated_at FROM sms_provider_config"
```

不要执行 `SELECT * FROM sms_provider_config`，避免密钥出现在终端记录中。

## SpacetimeDB HTTP 限制

SpacetimeDB 2.6.1 对 Procedure 发出的单次 HTTP 请求设置了 500ms 最大超时。
如果联麓短信接口、DNS或网络连接在 500ms 内没有完成，发送 Procedure 会失败，
并清除本次验证码预留记录。

常见错误：

- `验证码发送失败，请稍后重试`：检查 Module 日志中的联麓或超时错误。
- `验证码发送过于频繁`：等待返回的秒数后重试。
- `验证码不存在或已失效`：重新获取验证码。
- `验证码不正确`：检查输入；累计 5 次错误后必须重新获取。
- `账号已被禁用`：验证码正确，但中心用户或租户业务用户已被停用。

查看远程 Module 日志：

```bash
spacetime logs --server self-hosted yizu-server-yz18m -n 100
```

## 修改后的验证步骤

```bash
# Module 单元测试
cargo test -p parkwise_server

# 整个 Workspace 类型检查
cargo check --workspace

# 重新生成 Dioxus Rust 客户端绑定
cd server
spacetime generate --lang rust \
  --out-dir ../src/spacetime_bindings \
  --module-path spacetimedb

# 构建 Dioxus Web 客户端
cd ..
dx build --platform web
```

生成后的 `src/spacetime_bindings` 文件不能手动修改。需要先修改 Module 源码，再通过
`spacetime generate` 重新生成。
