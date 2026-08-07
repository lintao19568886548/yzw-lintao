#!/usr/bin/env bash
# Web 端开发服务器：先停掉上一次的 dx serve，再启动新的。
#
# IDE 里点「运行」重复触发时，旧进程往往还占着端口，dx 会直接报
# Address already in use。这里统一先收尾再启动，做到一键重启。
set -uo pipefail

cd "$(dirname "$0")/../.."

# Homebrew 的 deno 也提供了一个叫 dx 的可执行文件，PATH 里排在前面。
# 固定走 cargo 安装的那个。
DX="${DX:-$HOME/.cargo/bin/dx}"
PORT="${PORT:-8080}"

if [ ! -x "$DX" ]; then
  echo "找不到 dx：$DX" >&2
  echo "先执行 cargo binstall dioxus-cli 或 cargo install dioxus-cli" >&2
  exit 1
fi

# 只取处于 LISTEN 状态的进程。不加 -sTCP:LISTEN 的话，lsof 会把所有
# 连到这个端口的**客户端**也算进来——浏览器开着页面时，返回的就是浏览器的
# 网络进程，一 kill -9 直接把浏览器打断。
listeners() {
  lsof -ti "tcp:${PORT}" -sTCP:LISTEN 2>/dev/null
}

stop() {
  # 先按进程名收，再按端口兜底——dx 崩溃时留下的子进程不一定还挂在 dx 名下。
  pkill -f "$DX serve" 2>/dev/null
  pkill -f "dx serve" 2>/dev/null

  local pids
  pids="$(listeners)"
  if [ -n "$pids" ]; then
    echo "释放端口 ${PORT}：$pids"
    # shellcheck disable=SC2086
    kill $pids 2>/dev/null
    sleep 1
    pids="$(listeners)"
    # shellcheck disable=SC2086
    [ -n "$pids" ] && kill -9 $pids 2>/dev/null
  fi
}

stop

# 只停不启：scripts/dev/serve-web.sh stop
if [ "${1:-}" = "stop" ]; then
  echo "已停止 dx serve"
  exit 0
fi

# dx 默认开交互式 TUI，它要一个真终端。在 IDE 的运行窗口里没有 tty，
# TUI 会画出一屏乱码般的控制字符，日志反而看不见——此时退回纯日志输出。
extra=()
[ -t 1 ] || extra+=(--interactive false)

echo "启动 dx serve（端口 ${PORT}）"
"$DX" serve --port "$PORT" "${extra[@]}" "$@" &
child=$!

# 这里不能用 exec：exec 之后 trap 就没了，IDE 点停止只会杀掉 dx 本身，
# 它派生的构建进程和端口占用留在原地，下次启动照样撞端口。
trap 'kill "$child" 2>/dev/null; wait "$child" 2>/dev/null; stop; exit 0' INT TERM

wait "$child"
