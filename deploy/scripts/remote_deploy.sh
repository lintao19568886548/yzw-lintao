#!/usr/bin/env bash
set -euo pipefail

APP_ROOT="${YIZU_APP_ROOT:-/home/ubuntu/yizu}"
RUNTIME_ENV="${YIZU_RUNTIME_ENV:-/tmp/yizu-runtime.env}"
SERVICE_NAME="${YIZU_SERVER_NAME:-yizu-app}"
APP_PORT="${YIZU_SERVER_PORT:-8081}"
APP_IP="${YIZU_SERVER_IP:-127.0.0.1}"
DEPLOY_ENVIRONMENT="${YIZU_DEPLOY_ENVIRONMENT:-}"
WORK_DIR="${YIZU_DEPLOY_WORK_DIR:-}"
SPACETIME_WASM="${YIZU_SPACETIME_WASM_NAME:-parkwise_server.wasm}"
SPACETIME_DB_NAME="${YIZU_SPACETIMEDB_DB_NAME:-yizu-server-yz18m}"
SPACETIME_SERVER="${YIZU_SPACETIMEDB_SERVER:-local}"
SPACETIME_CLI="${YIZU_SPACETIMEDB_CLI:-/stdb/spacetime}"
SPACETIME_PUBLISH_ENABLED="${YIZU_SPACETIMEDB_PUBLISH_ENABLED:-false}"
ENABLE_NGINX="${YIZU_ENABLE_NGINX_CONF:-true}"
NGINX_CONF_NAME="${YIZU_NGINX_CONF_NAME:-yizu-furong.org.conf}"

RELEASES_DIR="$APP_ROOT/releases"
CURRENT_DIR="$APP_ROOT/current"
SPACETIME_DIR="$APP_ROOT/spacetime"
PRIVILEGED_PREFIX=""

if [ "$(id -u)" -ne 0 ]; then
  PRIVILEGED_PREFIX="sudo"
fi

case "$DEPLOY_ENVIRONMENT" in
  production | test | local) ;;
  *)
    echo "部署失败：部署环境未明确识别，已按失败关闭处理。"
    exit 1
    ;;
esac

# 生产上下文只检查环境变量名称，不读取或输出配置值。即使旧工作流或人工调用
# 再次注入危险开关，脚本也会在任何文件、服务或数据库操作前直接拒绝。
if [ "$DEPLOY_ENVIRONMENT" = "production" ]; then
  while IFS= read -r variable_name; do
    case "$variable_name" in
      YIZU_*CLEAR* | YIZU_*DELETE_DATA* | YIZU_*DESTROY* | YIZU_*DROP* | YIZU_*FORCE* | YIZU_*PURGE* | YIZU_*TRUNCATE* | YIZU_*WIPE*)
        echo "部署失败：生产上下文检测到被禁止的破坏性配置项名称。"
        exit 1
        ;;
    esac
  done < <(compgen -e)
fi

if [[ ! "$WORK_DIR" =~ ^/tmp/yizu-deploy\.[A-Za-z0-9_-]+$ ]] || [ ! -d "$WORK_DIR" ]; then
  echo "部署失败：部署工作目录缺失或不在允许的临时目录范围内。"
  exit 1
fi

if [ ! -f "$RUNTIME_ENV" ]; then
  echo "部署失败：未找到运行时环境文件 $RUNTIME_ENV"
  exit 1
fi

echo "[$(date '+%F %T')] 开始部署已验证的临时工作目录。"
mkdir -p "$RELEASES_DIR" "$SPACETIME_DIR"

if [ ! -f "$WORK_DIR/web/server" ]; then
  echo "部署失败：缺少 web/server 可执行文件"
  exit 1
fi

if [ ! -f "$WORK_DIR/spacetime/$SPACETIME_WASM" ]; then
  echo "部署失败：缺少 SpacetimeDB 模块文件 $SPACETIME_WASM"
  exit 1
fi

RELEASE_DIR="$RELEASES_DIR/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RELEASE_DIR"
cp -R "$WORK_DIR/web/"* "$RELEASE_DIR/"
cp "$WORK_DIR/spacetime/$SPACETIME_WASM" "$SPACETIME_DIR/$SPACETIME_WASM"
$PRIVILEGED_PREFIX cp "$WORK_DIR/yizu-app.service" /etc/systemd/system/"$SERVICE_NAME".service
$PRIVILEGED_PREFIX sed -i "s#/opt/yizu#$APP_ROOT#g" "/etc/systemd/system/$SERVICE_NAME.service"
$PRIVILEGED_PREFIX sed -i "s#^Environment=IP=.*#Environment=IP=${APP_IP}#" "/etc/systemd/system/$SERVICE_NAME.service"
$PRIVILEGED_PREFIX sed -i "s#^Environment=PORT=.*#Environment=PORT=${APP_PORT}#" "/etc/systemd/system/$SERVICE_NAME.service"
ln -sfn "$RELEASE_DIR" "$CURRENT_DIR"

# `.env` 由 root 服务读取，使用提权后的原子安装替代普通用户重定向。
# 这样无论文件此前属于 root 还是部署用户，都不会因 shell 重定向权限失败。
$PRIVILEGED_PREFIX install -m 0600 -o root -g root "$RUNTIME_ENV" "$APP_ROOT/.env"

# 运行服务前确认文件权限
chmod +x "$CURRENT_DIR/server"

if [ "$ENABLE_NGINX" = "true" ] && [ -f "$WORK_DIR/$NGINX_CONF_NAME" ] && command -v nginx >/dev/null 2>&1; then
  echo "更新 Nginx 配置（443/TCP -> Dioxus，1314/TCP -> SpacetimeDB）..."
  if [ -d "/etc/nginx/conf.d" ]; then
    $PRIVILEGED_PREFIX cp "$WORK_DIR/$NGINX_CONF_NAME" /etc/nginx/conf.d/"$NGINX_CONF_NAME"
  elif [ -d "/etc/nginx/sites-available" ]; then
    $PRIVILEGED_PREFIX cp "$WORK_DIR/$NGINX_CONF_NAME" /etc/nginx/sites-available/"$NGINX_CONF_NAME"
    if [ -d "/etc/nginx/sites-enabled" ] && [ ! -L "/etc/nginx/sites-enabled/$NGINX_CONF_NAME" ]; then
      $PRIVILEGED_PREFIX ln -sfn "/etc/nginx/sites-available/$NGINX_CONF_NAME" "/etc/nginx/sites-enabled/$NGINX_CONF_NAME"
    fi
  else
    echo "未检测到 /etc/nginx/conf.d 或 /etc/nginx/sites-available，跳过 Nginx 配置。"
    echo "你可以手动将 $WORK_DIR/$NGINX_CONF_NAME 安装到你的 Nginx 配置目录。"
  fi
  if command -v nginx >/dev/null 2>&1; then
    $PRIVILEGED_PREFIX nginx -t
    $PRIVILEGED_PREFIX systemctl reload nginx || true
  fi
else
  echo "未检测到 Nginx 服务或配置文件，跳过反代配置。"
fi

echo "重启 Dioxus 服务..."
$PRIVILEGED_PREFIX systemctl daemon-reload
$PRIVILEGED_PREFIX systemctl enable "$SERVICE_NAME".service
$PRIVILEGED_PREFIX systemctl restart "$SERVICE_NAME".service

if [ "$SPACETIME_PUBLISH_ENABLED" = "true" ]; then
  if [ -x "$SPACETIME_CLI" ]; then
    echo "开始执行固定的非破坏性 SpacetimeDB 发布..."
    if [ -f /etc/systemd/system/spacetimedb.service ]; then
      $PRIVILEGED_PREFIX systemctl enable --now spacetimedb.service
    fi
    $PRIVILEGED_PREFIX install -d -m 0750 -o spacetimedb -g spacetimedb /stdb/modules
    $PRIVILEGED_PREFIX install -m 0750 -o spacetimedb -g spacetimedb \
      "$SPACETIME_DIR/$SPACETIME_WASM" "/stdb/modules/$SPACETIME_WASM"
    $PRIVILEGED_PREFIX runuser -u spacetimedb -- "$SPACETIME_CLI" --root-dir=/stdb publish "$SPACETIME_DB_NAME" \
      --bin-path "/stdb/modules/$SPACETIME_WASM" \
      --server "$SPACETIME_SERVER" \
      --yes=all
  else
    echo "跳过 Spacetime 发布：未检测到 spacetime CLI。"
  fi
else
  echo "未开启 Spacetime 发布开关，已跳过 publish。"
fi

echo "部署完成。当前服务状态："
$PRIVILEGED_PREFIX systemctl is-active "$SERVICE_NAME".service
$PRIVILEGED_PREFIX systemctl is-active spacetimedb.service
$PRIVILEGED_PREFIX systemctl is-active nginx.service
