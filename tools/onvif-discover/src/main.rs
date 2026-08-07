//! 局域网 ONVIF 摄像头发现与 RTSP 地址探测。
//!
//! 装机现场跑一次，回答两个问题：这个网段上有哪些摄像头、它们的 RTSP 地址
//! 是什么。拿到地址先用 VLC 验证能不能打开，通了再往下写取流代码——顺序反了
//! 会在「代码有问题还是地址有问题」之间空耗很久。
//!
//! 用法：
//! ```text
//! onvif-discover                          # 只发现，不需要密码
//! onvif-discover -u admin -p 密码          # 发现并取出 RTSP 地址
//! onvif-discover --host 192.168.1.64 -u admin -p 密码   # 跳过发现，直查一台
//! onvif-discover --timeout 8              # 网络慢就加长等待
//! ```

mod soap;
mod xml;

use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// WS-Discovery 的固定组播地址与端口，由规范写死。
const DISCOVERY_ADDR: (Ipv4Addr, u16) = (Ipv4Addr::new(239, 255, 255, 250), 3702);

fn main() {
    let args = Args::parse(std::env::args().skip(1));
    if args.help {
        print_help();
        return;
    }

    println!("ONVIF 摄像头发现工具\n");

    let devices = if let Some(host) = &args.host {
        println!("跳过发现，直接查询 {host}\n");
        vec![Device {
            xaddr: format!("http://{host}/onvif/device_service"),
            name: None,
            hardware: None,
            source: host.clone(),
        }]
    } else {
        discover(&args)
    };

    if devices.is_empty() {
        println!("没有发现任何 ONVIF 设备。");
        print_no_result_hints();
        return;
    }

    println!("发现 {} 台设备：\n", devices.len());
    for (index, device) in devices.iter().enumerate() {
        println!("[{}] {}", index + 1, device.title());
        println!("    设备服务：{}", device.xaddr);
        match (&args.user, &args.password) {
            (Some(user), Some(password)) => match fetch_stream_uris(device, user, password, &args) {
                Ok(uris) if uris.is_empty() => {
                    println!("    RTSP：设备没有返回任何视频档案");
                }
                Ok(uris) => {
                    for (profile, uri) in uris {
                        println!("    RTSP（{profile}）：{}", with_credentials(&uri, user, password));
                    }
                }
                Err(error) => {
                    println!("    RTSP：取不到——{error}");
                    print_auth_hints(&error);
                    print_guessed_paths(&device.host());
                }
            },
            _ => {
                println!("    RTSP：未提供账号密码，跳过。加 -u 与 -p 可取出真实地址");
                print_guessed_paths(&device.host());
            }
        }
        println!();
    }

    println!("下一步：把上面的 RTSP 地址粘进 VLC（媒体 → 打开网络串流）验证。");
    println!("能在 VLC 里放出画面，才说明地址、账号、网络这三样都通了。");
}

/// 一台被发现的设备。
struct Device {
    xaddr: String,
    name: Option<String>,
    hardware: Option<String>,
    /// 响应来自哪个 IP，用来兜底显示。
    source: String,
}

impl Device {
    fn title(&self) -> String {
        match (&self.name, &self.hardware) {
            (Some(name), Some(hw)) => format!("{name}（{hw}）"),
            (Some(name), None) => name.clone(),
            (None, Some(hw)) => hw.clone(),
            (None, None) => self.source.clone(),
        }
    }

    /// 从设备服务地址里取出 host[:port]。
    fn host(&self) -> String {
        self.xaddr
            .strip_prefix("http://")
            .and_then(|rest| rest.split('/').next())
            .unwrap_or(&self.source)
            .to_string()
    }
}

/// 往每一张网卡各发一次探测。
///
/// **必须逐网卡发**，不能只绑 `0.0.0.0`：园区那台电脑常常有两张网卡——一张连
/// 办公网、一张单独连摄像头的网段。只绑通配地址时组播往往只从默认路由那张网卡
/// 出去，摄像头那张网卡上的设备一台也发现不了，看起来就像「这网段没有摄像头」。
fn discover(args: &Args) -> Vec<Device> {
    let mut found: BTreeMap<String, Device> = BTreeMap::new();
    let interfaces = local_ipv4s();
    if interfaces.is_empty() {
        println!("没有找到可用的 IPv4 网卡。");
        return Vec::new();
    }
    println!(
        "在 {} 张网卡上探测，等待 {} 秒……",
        interfaces.len(),
        args.timeout.as_secs()
    );
    for ip in &interfaces {
        println!("  · {ip}");
    }
    println!();

    for ip in interfaces {
        let Ok(socket) = UdpSocket::bind(SocketAddr::new(IpAddr::V4(ip), 0)) else {
            continue;
        };
        socket.set_read_timeout(Some(Duration::from_millis(400))).ok();
        socket.set_multicast_ttl_v4(2).ok();
        let probe = probe_message(nonce());
        let target = SocketAddr::new(IpAddr::V4(DISCOVERY_ADDR.0), DISCOVERY_ADDR.1);
        // 发三遍：UDP 组播丢包很常见，摄像头也可能正忙着别的事。
        for _ in 0..3 {
            let _ = socket.send_to(probe.as_bytes(), target);
        }

        let deadline = Instant::now() + args.timeout;
        let mut buf = vec![0u8; 65_535];
        while Instant::now() < deadline {
            let Ok((len, from)) = socket.recv_from(&mut buf) else {
                continue;
            };
            let body = String::from_utf8_lossy(&buf[..len]);
            let Some(xaddr) = xml::value(&body, "XAddrs") else {
                continue;
            };
            // 一台设备可能给出多个地址（多网卡），取第一个 http 的。
            let Some(xaddr) = xaddr
                .split_whitespace()
                .find(|candidate| candidate.starts_with("http://"))
            else {
                continue;
            };
            let scopes = xml::value(&body, "Scopes").unwrap_or_default();
            found.entry(xaddr.to_string()).or_insert(Device {
                xaddr: xaddr.to_string(),
                name: scope_value(&scopes, "name"),
                hardware: scope_value(&scopes, "hardware"),
                source: from.ip().to_string(),
            });
        }
    }
    found.into_values().collect()
}

/// ONVIF 把设备名和型号塞在 Scopes 里，形如
/// `onvif://www.onvif.org/name/HIKVISION%20DS-2CD3T47`。
fn scope_value(scopes: &str, key: &str) -> Option<String> {
    let needle = format!("/{key}/");
    scopes.split_whitespace().find_map(|scope| {
        scope
            .rfind(&needle)
            .map(|at| percent_decode(&scope[at + needle.len()..]))
            .filter(|value| !value.is_empty())
    })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).trim().to_string()
}

/// 取媒体服务地址 → 取档案 → 逐个取 RTSP 地址。
fn fetch_stream_uris(
    device: &Device,
    user: &str,
    password: &str,
    args: &Args,
) -> Result<Vec<(String, String)>, String> {
    let media = media_service(device, user, password, args).unwrap_or_else(|| device.xaddr.clone());

    let profiles_body = envelope(
        &soap::security_header(user, password, now_unix(), nonce()),
        r#"<GetProfiles xmlns="http://www.onvif.org/ver10/media/wsdl"/>"#,
    );
    let response = soap::post(&media, &profiles_body, args.timeout)?;
    if let Some(reason) = fault_reason(&response) {
        return Err(reason);
    }

    let mut uris = Vec::new();
    // 档案 token 在属性上，名字在子元素里。
    for (token, name) in profile_tokens(&response) {
        let body = envelope(
            &soap::security_header(user, password, now_unix(), nonce()),
            &format!(
                r#"<GetStreamUri xmlns="http://www.onvif.org/ver10/media/wsdl"><StreamSetup xmlns="http://www.onvif.org/ver10/media/wsdl"><Stream xmlns="http://www.onvif.org/ver10/schema">RTP-Unicast</Stream><Transport xmlns="http://www.onvif.org/ver10/schema"><Protocol>RTSP</Protocol></Transport></StreamSetup><ProfileToken>{token}</ProfileToken></GetStreamUri>"#
            ),
        );
        let Ok(reply) = soap::post(&media, &body, args.timeout) else {
            continue;
        };
        if let Some(uri) = xml::value(&reply, "Uri") {
            uris.push((name.unwrap_or(token), uri));
        }
    }
    Ok(uris)
}

/// 从 GetCapabilities 里取媒体服务地址；拿不到就退回设备服务地址。
///
/// 多数相机在设备服务地址上也接受媒体调用，所以失败不致命。
fn media_service(device: &Device, user: &str, password: &str, args: &Args) -> Option<String> {
    let body = envelope(
        &soap::security_header(user, password, now_unix(), nonce()),
        r#"<GetCapabilities xmlns="http://www.onvif.org/ver10/device/wsdl"><Category>Media</Category></GetCapabilities>"#,
    );
    let response = soap::post(&device.xaddr, &body, args.timeout).ok()?;
    xml::value(&response, "XAddr")
}

/// `<trt:Profiles token="Profile_1"><tt:Name>主码流</tt:Name>…`
///
/// token 在属性上、名字在子元素里，所以要按整段元素取，不能按文本切。
fn profile_tokens(xml_body: &str) -> Vec<(String, Option<String>)> {
    let mut out: Vec<(String, Option<String>)> = Vec::new();
    for one in xml::elements(xml_body, "Profiles") {
        let Some(token) = xml::attr(one, "Profiles", "token") else {
            continue;
        };
        if out.iter().any(|(existing, _)| existing == &token) {
            continue;
        }
        out.push((token, xml::value(one, "Name")));
    }
    out
}

/// 把账号密码塞进 RTSP 地址，方便直接粘进 VLC。
fn with_credentials(uri: &str, user: &str, password: &str) -> String {
    match uri.strip_prefix("rtsp://") {
        // 设备已经带了凭据就不重复塞。
        Some(rest) if rest.contains('@') => uri.to_string(),
        Some(rest) => format!("rtsp://{}:{}@{rest}", escape_userinfo(user), escape_userinfo(password)),
        None => uri.to_string(),
    }
}

/// 密码里出现 `@` `:` `/` 会把 URL 拆错，必须百分号编码。
fn escape_userinfo(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '.' | '_' | '~' => c.to_string(),
            other => {
                let mut buf = [0u8; 4];
                other
                    .encode_utf8(&mut buf)
                    .bytes()
                    .map(|b| format!("%{b:02X}"))
                    .collect()
            }
        })
        .collect()
}

fn fault_reason(response: &str) -> Option<String> {
    if !response.contains("Fault") {
        return None;
    }
    xml::value(response, "Text")
        .or_else(|| xml::value(response, "Reason"))
        .or_else(|| Some("设备返回 SOAP Fault".to_string()))
}

fn envelope(header: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope">{header}<s:Body>{body}</s:Body></s:Envelope>"#
    )
}

fn probe_message(nonce: u64) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><e:Envelope xmlns:e="http://www.w3.org/2003/05/soap-envelope" xmlns:w="http://schemas.xmlsoap.org/ws/2004/08/addressing" xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery" xmlns:dn="http://www.onvif.org/ver10/network/wsdl"><e:Header><w:MessageID>urn:uuid:{nonce:016x}-0000-4000-8000-000000000000</w:MessageID><w:To e:mustUnderstand="true">urn:schemas-xmlsoap-org:ws:2005:04:discovery</w:To><w:Action e:mustUnderstand="true">http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</w:Action></e:Header><e:Body><d:Probe><d:Types>dn:NetworkVideoTransmitter</d:Types></d:Probe></e:Body></e:Envelope>"#
    )
}

fn local_ipv4s() -> Vec<Ipv4Addr> {
    let mut out = Vec::new();
    let Ok(addrs) = if_addrs::get_if_addrs() else {
        return out;
    };
    for iface in addrs {
        if iface.is_loopback() {
            continue;
        }
        if let IpAddr::V4(v4) = iface.ip() {
            out.push(v4);
        }
    }
    out.sort();
    out.dedup();
    out
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

fn nonce() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}

fn print_guessed_paths(host: &str) {
    let bare = host.split(':').next().unwrap_or(host);
    println!("    常见厂商的固定路径（拿不到 ONVIF 地址时可以逐个试）：");
    println!("      海康：rtsp://用户:密码@{bare}:554/Streaming/Channels/101   （101 主码流 / 102 子码流）");
    println!("      大华：rtsp://用户:密码@{bare}:554/cam/realmonitor?channel=1&subtype=0");
    println!("      宇视：rtsp://用户:密码@{bare}:554/media/video1");
}

fn print_auth_hints(error: &str) {
    let lowered = error.to_lowercase();
    if lowered.contains("auth") || error.contains("认证") || lowered.contains("401") {
        println!("    提示：认证失败最常见的原因不是密码错，而是**摄像头时间跑偏**——");
        println!("          ONVIF 摘要认证把时间戳算进哈希，相机没接 NTP 时容易差几分钟。");
        println!("          先到相机管理页把时间校准，再试一次。");
        println!("    其次：不少相机要在管理页里单独「启用 ONVIF」并为它设独立的账号。");
    }
}

fn print_no_result_hints() {
    println!();
    println!("按下面的顺序排查：");
    println!("  1. 这台电脑和摄像头在同一个网段吗？摄像头常单独挂一张网卡或一个 VLAN；");
    println!("  2. Windows 防火墙是否拦了本程序的 UDP 入站？发现响应是设备主动发回来的，");
    println!("     被拦掉就什么都收不到——临时关掉防火墙试一次即可确认是不是它；");
    println!("  3. 摄像头是否在管理页里启用了 ONVIF？不少型号出厂默认关闭；");
    println!("  4. 交换机是否禁用了组播？企业交换机常关掉 IGMP 转发。");
    println!();
    println!("以上都不通时，直接用 --host 指定摄像头 IP 查询，或者用上面列出的");
    println!("厂商固定路径在 VLC 里逐个试——发现不了不代表 RTSP 不通。");
}

fn print_help() {
    println!("局域网 ONVIF 摄像头发现与 RTSP 地址探测\n");
    println!("用法：");
    println!("  onvif-discover                                   只发现设备，不需要密码");
    println!("  onvif-discover -u admin -p 密码                   发现并取出 RTSP 地址");
    println!("  onvif-discover --host 192.168.1.64 -u admin -p 密码   跳过发现，直查一台");
    println!();
    println!("选项：");
    println!("  -u, --user <账号>       ONVIF 账号");
    println!("  -p, --pass <密码>       ONVIF 密码");
    println!("      --host <地址>       跳过发现，直接查询这个 IP");
    println!("      --timeout <秒>      发现等待时长，默认 5");
    println!("  -h, --help              显示本帮助");
}

struct Args {
    user: Option<String>,
    password: Option<String>,
    host: Option<String>,
    timeout: Duration,
    help: bool,
}

impl Args {
    fn parse(args: impl Iterator<Item = String>) -> Self {
        let mut parsed = Args {
            user: None,
            password: None,
            host: None,
            timeout: Duration::from_secs(5),
            help: false,
        };
        let mut iter = args.peekable();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-u" | "--user" => parsed.user = iter.next(),
                "-p" | "--pass" | "--password" => parsed.password = iter.next(),
                "--host" => parsed.host = iter.next(),
                "--timeout" => {
                    if let Some(secs) = iter.next().and_then(|v| v.parse().ok()) {
                        parsed.timeout = Duration::from_secs(secs);
                    }
                }
                "-h" | "--help" => parsed.help = true,
                _ => {}
            }
        }
        parsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 从_scopes_里取出设备名与型号() {
        let scopes = "onvif://www.onvif.org/type/video_encoder \
                      onvif://www.onvif.org/name/HIKVISION%20DS-2CD3T47 \
                      onvif://www.onvif.org/hardware/DS-2CD3T47WD";
        assert_eq!(scope_value(scopes, "name").as_deref(), Some("HIKVISION DS-2CD3T47"));
        assert_eq!(scope_value(scopes, "hardware").as_deref(), Some("DS-2CD3T47WD"));
        assert_eq!(scope_value(scopes, "location"), None);
    }

    #[test]
    fn 密码里的特殊字符会被编码进_rtsp_地址() {
        // `@` 不编码的话 VLC 会把地址拆错，表现成「用户名密码错误」，
        // 而实际上是地址被解析成了另一个主机。
        let uri = with_credentials("rtsp://192.168.1.64:554/Streaming", "admin", "a@b:c");
        assert_eq!(uri, "rtsp://admin:a%40b%3Ac@192.168.1.64:554/Streaming");
    }

    #[test]
    fn 设备自带凭据时不重复拼() {
        let uri = with_credentials("rtsp://u:p@10.0.0.9/live", "admin", "x");
        assert_eq!(uri, "rtsp://u:p@10.0.0.9/live");
    }

    #[test]
    fn 从设备服务地址取出主机() {
        let device = Device {
            xaddr: "http://192.168.1.64:8000/onvif/device_service".into(),
            name: None,
            hardware: None,
            source: "192.168.1.64".into(),
        };
        assert_eq!(device.host(), "192.168.1.64:8000");
    }

    #[test]
    fn 解析出多个档案的_token_与名称() {
        let body = r#"<trt:Profiles token="Profile_1" fixed="true"><tt:Name>mainStream</tt:Name></trt:Profiles>
                      <trt:Profiles token="Profile_2" fixed="true"><tt:Name>subStream</tt:Name></trt:Profiles>"#;
        let tokens = profile_tokens(body);
        assert_eq!(tokens.len(), 2, "{tokens:?}");
        assert_eq!(tokens[0].0, "Profile_1");
        assert_eq!(tokens[0].1.as_deref(), Some("mainStream"));
        assert_eq!(tokens[1].0, "Profile_2");
    }

    #[test]
    fn 参数解析支持长短两种写法() {
        let args = Args::parse(
            ["-u", "admin", "--pass", "secret", "--timeout", "9"]
                .into_iter()
                .map(String::from),
        );
        assert_eq!(args.user.as_deref(), Some("admin"));
        assert_eq!(args.password.as_deref(), Some("secret"));
        assert_eq!(args.timeout, Duration::from_secs(9));
    }

    #[test]
    fn soap_fault_被识别为错误而不是空结果() {
        let body = "<s:Fault><s:Reason><s:Text>Sender not authorized</s:Text></s:Reason></s:Fault>";
        assert_eq!(
            fault_reason(body).as_deref(),
            Some("Sender not authorized")
        );
        assert_eq!(fault_reason("<GetProfilesResponse/>"), None);
    }
}
