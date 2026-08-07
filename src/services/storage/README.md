# Cloudflare R2 图片存储说明

当前工资凭证复用 `MyIndex005` 的 Cloudflare R2 存储桶，采用“服务端签名、浏览器直传、SpacetimeDB 保存关系”的方式。

## 上传流程

1. 用户选择图片时，浏览器只做格式、大小校验并创建本地预览，不上传 R2。
2. 用户点击“确认保存”后，浏览器读取图片并在 WASM 内计算 SHA-256。
3. Dioxus 服务端校验 SpacetimeDB Token 及 `Super/system` 角色。
4. 服务端生成有效期 10 分钟的 R2 预签名 PUT 地址。
5. 浏览器直接把图片上传到 R2，图片字节不经过 SpacetimeDB。
6. 图片全部上传成功后，Reducer 在同一事务中保存工资、登记 `image` 并写入 `salary_image` 关系。

用户在确认前取消或移除图片时，只会释放浏览器本地预览，不会在 R2 留下未关联对象。上传期间弹窗关闭和取消操作会暂时锁定，避免上传一半退出。

## 删除流程

1. 编辑工资时点击既有凭证的“移除”，只会在表单中标记“确认后删除”，可以撤销。
2. 确认保存或永久删除工资后，Reducer 先物理删除工资图片关系。
3. 图片未被任何其他业务关系引用时，Reducer 物理删除共享 `image` 元数据。
4. 数据库事务成功后，Dioxus 服务端再次确认 `my_images` 中已不存在该图片，再删除对应 R2 对象。
5. 图片仍被其他业务引用时保留元数据和 R2 对象，避免其他页面出现裂图。

`salary.is_deleted` 字段仅为兼容 MySQL 历史结构保留；当前工资删除操作会真正删除 `salary` 主记录，不再写入新的逻辑删除数据。

对象路径使用内容哈希：

```text
yizu/salary-images/{sha256}.{扩展名}
```

相同文件会获得相同对象地址，并由 SpacetimeDB 按当前租户和哈希复用图片记录。
如果 MySQL 迁移记录仍保存 `/uploads/...` 旧地址，首次确认保存同一文件时会复用原 `img_id`，并把旧地址更新成内容哈希对应的 R2 地址。

## 配置

复制 `.env.example` 为 `.env`，配置以下变量：

```text
R2_ACCESS_KEY_ID
R2_SECRET_ACCESS_KEY
R2_ENDPOINT_URL
R2_BUCKET_NAME
R2_PUBLIC_BASE_URL
YIZU_SPACETIMEDB_SERVER_URL
YIZU_SPACETIMEDB_DATABASE
```

`.env` 已加入 Git 忽略规则，禁止把 R2 密钥写入 Rust 源码或提交到仓库。

## 历史图片

MySQL 的 `image` 表只包含 URL 和哈希。旧 `/uploads/...` 文件仍需要从原 Node.js 服务器的 Docker `uploads_data` 卷复制到 R2，再更新对应的 `image.img_url`。
