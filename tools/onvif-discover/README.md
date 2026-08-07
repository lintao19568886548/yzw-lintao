# ONVIF 摄像头发现工具

装机现场跑一次，回答两个问题：**这个网段上有哪些摄像头**、**它们的 RTSP 地址是什么**。

拿到地址后**先用 VLC 验证能不能打开**，通了再往下写取流代码。顺序反了会在「是代码有问题还是地址有问题」之间空耗很久。

## 编译

```bash
cargo build --release --manifest-path tools/onvif-discover/Cargo.toml
```

产物是一个约 370 KB 的单文件，**不依赖任何 DLL**，拷到目标机器上直接跑。

它自成一个工作区，不进主构建——只在装机时用一次，没必要让每次 `cargo test` 都跟着编译它。

给园区那台 Windows 电脑用时交叉编译：

```bash
rustup target add x86_64-pc-windows-msvc   # 或 x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-msvc \
  --manifest-path tools/onvif-discover/Cargo.toml
```

> 也可以在**任何一台和摄像头同网段的电脑**上跑，Mac、Linux 都行，不一定非得是那台 Windows。它只发 UDP 组播、收响应，不改任何东西。

## 用法

```bash
onvif-discover                                        # 只发现，不需要密码
onvif-discover -u admin -p 密码                        # 发现并取出 RTSP 地址
onvif-discover --host 192.168.1.64 -u admin -p 密码    # 跳过发现，直查一台
onvif-discover --timeout 8                            # 网络慢就加长等待
```

不给账号密码也能跑，只是取不到 RTSP 地址——ONVIF 的 `GetStreamUri` 要求认证。这时它会列出常见厂商的固定路径供手工尝试。

## 它做了什么

1. **逐网卡**发 WS-Discovery 探测（UDP 组播 `239.255.255.250:3702`），每张网卡发三遍；
2. 收集响应里的设备服务地址（`XAddrs`）与设备名、型号（藏在 `Scopes` 里）；
3. 给了账号密码时，依次调 `GetCapabilities` → `GetProfiles` → `GetStreamUri`，取出每个码流档案的 RTSP 地址，并把账号密码拼进地址方便直接粘进 VLC。

### 为什么必须逐网卡发

园区那台电脑常常有两张网卡——一张连办公网、一张单独连摄像头网段。只绑通配地址 `0.0.0.0` 时，组播往往只从默认路由那张网卡出去，**摄像头那张网卡上的设备一台也发现不了**，看起来就像「这网段没有摄像头」。

## 发现不到时的排查顺序

程序跑完会打印同样的清单，这里再列一遍：

1. **网段**：这台电脑和摄像头在同一个网段吗？摄像头常单独挂一张网卡或一个 VLAN；
2. **防火墙**：Windows 防火墙是否拦了本程序的 UDP 入站？发现响应是设备主动发回来的，被拦掉就什么都收不到——临时关掉防火墙试一次即可确认；
3. **ONVIF 开关**：不少型号出厂默认关闭 ONVIF，要在摄像头管理页里单独启用，有的还要为它设独立账号；
4. **交换机组播**：企业交换机常关掉 IGMP 转发。

**发现不了不代表 RTSP 不通。** 用 `--host` 直接指定 IP 查询，或者拿下面的固定路径在 VLC 里逐个试。

## 认证失败最常见的原因不是密码错

ONVIF 的摘要认证把**时间戳**算进哈希，相机会拒绝时间偏差过大的请求。而摄像头没接 NTP、时间跑偏几分钟是现场常态。

**先到相机管理页把时间校准**，再试一次。程序在遇到认证类错误时也会提示这一条。

## 常见厂商的固定 RTSP 路径

拿不到 ONVIF 地址时可以逐个试：

| 厂商 | 地址 |
| --- | --- |
| 海康 | `rtsp://用户:密码@IP:554/Streaming/Channels/101`（`101` 主码流、`102` 子码流） |
| 大华 | `rtsp://用户:密码@IP:554/cam/realmonitor?channel=1&subtype=0` |
| 宇视 | `rtsp://用户:密码@IP:554/media/video1` |

密码里带 `@` `:` `/` 时必须百分号编码，否则 URL 会被拆错——表现出来像「用户名密码错误」，实际是地址被解析成了另一个主机。程序输出的地址已经编码好了。

## 下一步

地址在 VLC 里验证通过之后，取流那一层建议走 **FFmpeg backend**，并强制 TCP：

```bash
set OPENCV_FFMPEG_CAPTURE_OPTIONS=rtsp_transport;tcp
```

默认走 UDP，园区网络一抖就花屏。

> **不要走 GStreamer**：官方预编译的 OpenCV Windows 包不带 GStreamer 支持，`cv::CAP_GSTREAMER` 会直接打不开。要用就得自己从源码编译 OpenCV 并链接 GStreamer，在 Windows 上是好几天的活，而且每次升级都要重来。FFmpeg backend 在官方包里是带的。

整体设计见 [`docs/边缘计算设备与摄像头接入.md`](../../docs/边缘计算设备与摄像头接入.md)。
