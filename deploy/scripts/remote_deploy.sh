#!/usr/bin/env bash
set -euo pipefail

APP_ROOT="${YIZU_APP_ROOT:-/home/ubuntu/yizu}"
RELEASE_TARBALL="${YIZU_RELEASE_TARBALL:-/tmp/yizu-deploy.tar.gz}"
RUNTIME_ENV="${YIZU_RUNTIME_ENV:-/tmp/yizu-runtime.env}"
SERVICE_NAME="${YIZU_SERVER_NAME:-yizu-app}"
APP_PORT="${YIZU_SERVER_PORT:-8081}"
APP_IP="${YIZU_SERVER_IP:-127.0.0.1}"
WORK_DIR="${YIZU_DEPLOY_WORK_DIR:-/tmp/yizu-deploy}"
SPACETIME_WASM="${YIZU_SPACETIME_WASM_NAME:-parkwise_server.wasm}"
SPACETIME_DB_NAME="${YIZU_SPACETIMEDB_DB_NAME:-yizu-server-yz18m}"
SPACETIME_SERVER="${YIZU_SPACETIMEDB_SERVER:-local}"
SPACETIME_CLI="${YIZU_SPACETIMEDB_CLI:-/stdb/spacetime}"
SPACETIME_PUBLISH_YES="${YIZU_SPACETIMEDB_PUBLISH_YES:-all}"
SPACETIME_PUBLISH_FORCE="${YIZU_SPACETIMEDB_FORCE_PUBLISH:-false}"
# 发布时先销毁全部数据，用于 SpacetimeDB 拒绝的表结构变更（改列、删列一律
# 被拒，只能重建）。默认关闭，且必须显式写成 true 才生效——任何别的值都
# 按关闭处理，避免手滑写个 1 或 yes 就把生产数据清了。
SPACETIME_DELETE_DATA="${YIZU_SPACETIMEDB_DELETE_DATA:-false}"
PUBLISH_CMD="${YIZU_SPACETIMEDB_PUBLISH_CMD:-}"
ENABLE_NGINX="${YIZU_ENABLE_NGINX_CONF:-true}"
NGINX_CONF_NAME="${YIZU_NGINX_CONF_NAME:-yizu-furong.org.conf}"

RELEASES_DIR="$APP_ROOT/releases"
CURRENT_DIR="$APP_ROOT/current"
SPACETIME_DIR="$APP_ROOT/spacetime"
PRIVILEGED_PREFIX=""

if [ "$(id -u)" -ne 0 ]; then
  PRIVILEGED_PREFIX="sudo"
fi

if [ ! -f "$RELEASE_TARBALL" ]; then
  echo "部署失败：未找到部署包 $RELEASE_TARBALL"
  exit 1
fi

if [ ! -f "$RUNTIME_ENV" ]; then
  echo "部署失败：未找到运行时环境文件 $RUNTIME_ENV"
  exit 1
fi

echo "[$(date '+%F %T')] 开始部署：$RELEASE_TARBALL"
mkdir -p "$WORK_DIR" "$RELEASES_DIR" "$SPACETIME_DIR"
rm -rf "$WORK_DIR"/*

tar -xzf "$RELEASE_TARBALL" -C "$WORK_DIR"

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

if [ "$SPACETIME_PUBLISH_FORCE" = "true" ] || [ -n "$PUBLISH_CMD" ]; then
  if [ -x "$SPACETIME_CLI" ]; then
    echo "开始发布 SpacetimeDB..."
    if [ -f /etc/systemd/system/spacetimedb.service ]; then
      $PRIVILEGED_PREFIX systemctl enable --now spacetimedb.service
    fi
    if [ -n "$PUBLISH_CMD" ]; then
      bash -lc "$PUBLISH_CMD"
    else
      $PRIVILEGED_PREFIX install -d -m 0750 -o spacetimedb -g spacetimedb /stdb/modules
      $PRIVILEGED_PREFIX install -m 0750 -o spacetimedb -g spacetimedb \
        "$SPACETIME_DIR/$SPACETIME_WASM" "/stdb/modules/$SPACETIME_WASM"
      publish_args=(
        --bin-path "/stdb/modules/$SPACETIME_WASM"
        --server "$SPACETIME_SERVER"
        --yes="${SPACETIME_PUBLISH_YES}"
      )
      if [ "$SPACETIME_DELETE_DATA" = "true" ]; then
        echo "⚠️  YIZU_SPACETIMEDB_DELETE_DATA=true：本次发布将销毁数据库全部数据后重建表结构。"
        publish_args+=(--delete-data)
      fi
      $PRIVILEGED_PREFIX runuser -u spacetimedb -- "$SPACETIME_CLI" --root-dir=/stdb publish "$SPACETIME_DB_NAME" \
        "${publish_args[@]}"
    fi
  else
    echo "跳过 Spacetime 发布：未检测到 spacetime CLI。"
    echo "请在远端安装 CLI，并配置 YIZU_SPACETIMEDB_PUBLISH_CMD 后重试。"
  fi
else
  echo "未开启 Spacetime 发布开关，已跳过 publish。"
fi

echo "部署完成。当前服务状态："
$PRIVILEGED_PREFIX systemctl is-active "$SERVICE_NAME".service
$PRIVILEGED_PREFIX systemctl is-active spacetimedb.service
$PRIVILEGED_PREFIX systemctl is-active nginx.service
