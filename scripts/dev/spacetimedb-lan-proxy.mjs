#!/usr/bin/env node

/**
 * 将远程 SpacetimeDB 的 TCP 流量透明转发到本机局域网地址。
 *
 * 开发手机只需要与电脑处于同一局域网，不需要安装或连接 Tailscale。
 * TCP 透明转发同时支持普通 HTTP 请求和 WebSocket 升级连接。
 */

import net from 'node:net';

const listenHost = process.env.YIZU_PROXY_HOST ?? '0.0.0.0';
const listenPort = Number(process.env.YIZU_PROXY_PORT ?? '3000');
const targetHost = process.env.YIZU_PROXY_TARGET_HOST ?? '100.108.240.95';
const targetPort = Number(process.env.YIZU_PROXY_TARGET_PORT ?? '3000');

const server = net.createServer((client) => {
  const upstream = net.createConnection({ host: targetHost, port: targetPort });

  client.pipe(upstream);
  upstream.pipe(client);

  upstream.on('error', (error) => {
    console.error(`上游连接失败：${error.message}`);
    client.destroy();
  });

  client.on('error', () => upstream.destroy());
});

server.on('error', (error) => {
  console.error(`局域网转发启动失败：${error.message}`);
  process.exitCode = 1;
});

server.listen(listenPort, listenHost, () => {
  console.log(
    `SpacetimeDB 局域网转发已启动：http://${listenHost}:${listenPort} -> http://${targetHost}:${targetPort}`,
  );
});
