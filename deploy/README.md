# yz.furong.org 生产部署说明（64.110.114.8）

目标：一次构建并同时发布 Dioxus Web（前端+服务）与 SpacetimeDB 模块。

## 1. GitHub Actions 配置

仓库中已有 `.github/workflows/deploy-production.yml`，按以下方式配置。

### 1.1 Repository Secrets

- `DEPLOY_SSH_KEY` 对应的私钥必须在 `64.110.114.8` 上可直接免密登录 `root`（或调整 `DEPLOY_USER` 为实际用户）。
- `DEPLOY_HOST`：`64.110.114.8`
- `DEPLOY_USER`：`root`
- `DEPLOY_SSH_PORT`：`22`（或实际 SSH 端口）
- `DEPLOY_SSH_KEY`：用于无交互登录的私钥内容
- `R2_ACCESS_KEY_ID`、`R2_SECRET_ACCESS_KEY`、`R2_ENDPOINT_URL`、`R2_BUCKET_NAME`、`R2_PUBLIC_BASE_URL`
- `YIZU_YMSINO_USERNAME`、`YIZU_YMSINO_PASSWORD`、`YIZU_HEZHONG_USERNAME`、`YIZU_HEZHONG_LOGIN_KEY`
- `ALIYUN_BAILIAN_KEY`
- 生产发布不接受自定义发布命令；模块发布命令固定在受版本管理的远端脚本中。

### 1.2 Repository Variables

- `YIZU_PUBLIC_APP_ORIGIN=https://yz.furong.org`
- `YIZU_SPACETIMEDB_URI=https://yz.furong.org`
- `YIZU_SPACETIMEDB_SERVER_URL=http://127.0.0.1:3000`
- `YIZU_SPACETIMEDB_DATABASE=yizu-server-yz18m`
- `YIZU_SPACETIMEDB_SERVER=self-hosted`
- `YIZU_ENABLE_NGINX_CONF=true`
- `YIZU_YMSINO_BASE_URL`, `YIZU_YMSINO_ORG_ID` 等业务参数按现有环境填写

## 2. Cloudflare 与 443 冲突说明（hy2占用 443）

- 当前 Nginx 已配置只监听 `80` 并把 `/v1/` 统一转发到 `127.0.0.1:3000`，`/` 转发到 `127.0.0.1:8080`。
- 因为域名已走 Cloudflare，`yz.furong.org` 解析到 Cloudflare IP，推荐 Cloudflare SSL 模式为 **Flexible**（或改用 Cloudflare Tunnel）。
- 如果你要求端到端 HTTPS 且不使用 Tunnel，则需要在服务器上空出 443 并新增 HTTPS server block。

## 3. 触发部署

生产工作流不监听分支推送，只能在受保护的 `production` Environment 中手动运行。当前 P0-01 已移除清库、强制发布和任意命令入口；完整的 artifact 选择、审批与发布前置门禁由 P0-16 实施。

## 4. 连接稳定性检查（部署后）

```bash
curl -I https://yz.furong.org
curl -I https://yz.furong.org/v1/identity/websocket-token
```

后者若返回 `401` 通常是身份请求不带 token，是正常；重点看是否能建立 TCP/HTTP 连接不报 `523`。

## 5. 服务器上快速核对

```bash
ssh root@64.110.114.8 "systemctl status yizu-app --no-pager"
ssh root@64.110.114.8 "ss -lntp | sed -n '1,120p'"
ssh root@64.110.114.8 "systemctl status nginx --no-pager"
```

## 6. 注意点（本次配置）

- Nginx 中已将 `/v1/` 全量转给 SpacetimeDB，兼容 `/v1/database/...` 与 `/v1/identity/websocket-token`。
- SpacetimeDB 的域名发布入口与前端公开源站入口已分离：
  - 前端二维码等对外展示使用 `YIZU_PUBLIC_APP_ORIGIN`
  - Spacetime 订阅客户端使用 `YIZU_SPACETIMEDB_URI`
