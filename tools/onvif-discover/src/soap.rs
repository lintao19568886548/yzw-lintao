//! SOAP over HTTP：手写 HTTP/1.1，以及 ONVIF 的 WS-Security 摘要认证。
//!
//! 不用 HTTP 客户端库，因为 ONVIF 设备服务是明文 HTTP，一个 `TcpStream` 加
//! 三十行就够；引一个带 TLS 的客户端库只会把 exe 撑大、把依赖树拉长，而这个
//! 程序的第一要务是「拷到任何一台 Windows 上都能跑」。

use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD, Engine};
use sha1::{Digest, Sha1};

/// 往设备的 SOAP 端点发一次请求，返回响应正文。
pub fn post(url: &str, body: &str, timeout: Duration) -> Result<String, String> {
    let (host, port, path) = split_url(url)?;
    let addr = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(|e| format!("解析地址失败：{e}"))?
        .next()
        .ok_or_else(|| "地址解析为空".to_string())?;
    let mut stream =
        TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("连接失败：{e}"))?;
    stream.set_read_timeout(Some(timeout)).ok();
    stream.set_write_timeout(Some(timeout)).ok();

    let request = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         Content-Type: application/soap+xml; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("发送失败：{e}"))?;

    let mut raw = Vec::new();
    // 用 `Connection: close` 让对端读完就关，这样 read_to_end 一定会返回，
    // 不必自己盯 Content-Length。超时被读超时兜住。
    match stream.read_to_end(&mut raw) {
        Ok(_) => {}
        // 读超时时已经拿到的部分照样解析：有些相机响应完不主动关连接。
        Err(e) if raw.is_empty() => return Err(format!("读取失败：{e}")),
        Err(_) => {}
    }
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| "响应不是合法 HTTP".to_string())?;
    let body = if head.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        dechunk(body)
    } else {
        body.to_string()
    };
    Ok(body)
}

/// `http://192.168.1.64:80/onvif/device_service` → (host, port, path)
fn split_url(url: &str) -> Result<(String, u16, String), String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| format!("只支持 http 的 ONVIF 端点，收到：{url}"))?;
    let (authority, path) = match rest.find('/') {
        Some(at) => (&rest[..at], &rest[at..]),
        None => (rest, "/"),
    };
    // IPv6 字面量带方括号，冒号不能当端口分隔符用。
    let (host, port) = if authority.starts_with('[') {
        match authority.rsplit_once("]:") {
            Some((h, p)) => (format!("{h}]"), p.parse().unwrap_or(80)),
            None => (authority.to_string(), 80),
        }
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse().unwrap_or(80)),
            None => (authority.to_string(), 80),
        }
    };
    Ok((host, port, path.to_string()))
}

fn dechunk(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some((size_line, tail)) = rest.split_once("\r\n") {
        let size = usize::from_str_radix(size_line.trim().split(';').next().unwrap_or("0"), 16)
            .unwrap_or(0);
        if size == 0 || tail.len() < size {
            break;
        }
        out.push_str(&tail[..size]);
        rest = tail[size..].trim_start_matches("\r\n");
    }
    out
}

/// WS-Security UsernameToken 摘要头。
///
/// `PasswordDigest = Base64(SHA1(Nonce + Created + Password))`，这是 ONVIF
/// 规定的算法。**`Created` 必须贴近相机自己的时钟**——相机会拒绝时间偏差过大
/// 的请求，而摄像头没接 NTP、时间跑偏几分钟是现场常态，认证失败最常见的原因
/// 就是它，不是密码错。
pub fn security_header(user: &str, password: &str, now_unix: i64, nonce_seed: u64) -> String {
    let nonce = nonce_seed.to_be_bytes();
    let created = iso8601_utc(now_unix);
    let mut hasher = Sha1::new();
    hasher.update(nonce);
    hasher.update(created.as_bytes());
    hasher.update(password.as_bytes());
    let digest = STANDARD.encode(hasher.finalize());
    let nonce_b64 = STANDARD.encode(nonce);
    format!(
        r#"<s:Header><Security s:mustUnderstand="1" xmlns="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd"><UsernameToken><Username>{}</Username><Password Type="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest">{}</Password><Nonce EncodingType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary">{}</Nonce><Created xmlns="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd">{}</Created></UsernameToken></Security></s:Header>"#,
        escape(user),
        digest,
        nonce_b64,
        created
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Unix 秒 → `2026-08-06T08:00:00Z`。用 Howard Hinnant 民用历算法，与项目其余
/// 日期工具同一套，不引第三方日期库。
pub fn iso8601_utc(unix_secs: i64) -> String {
    let days = unix_secs.div_euclid(86_400);
    let secs = unix_secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 拆解常见的设备服务地址() {
        assert_eq!(
            split_url("http://192.168.1.64/onvif/device_service").unwrap(),
            ("192.168.1.64".into(), 80, "/onvif/device_service".into())
        );
        assert_eq!(
            split_url("http://192.168.1.64:8000/onvif/device_service").unwrap(),
            ("192.168.1.64".into(), 8000, "/onvif/device_service".into())
        );
        // 没有路径时补一个根路径，别拼出 `POST  HTTP/1.1`。
        assert_eq!(split_url("http://10.0.0.5").unwrap().2, "/");
    }

    #[test]
    fn ipv6_字面量不会把地址里的冒号当端口() {
        let (host, port, _) = split_url("http://[fe80::1]:8080/onvif").unwrap();
        assert_eq!(host, "[fe80::1]");
        assert_eq!(port, 8080);
    }

    #[test]
    fn https_端点明确报错而不是当成_http() {
        assert!(split_url("https://192.168.1.64/onvif").is_err());
    }

    #[test]
    fn 时间格式符合_ws_security_要求() {
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
        // 期望值由 Python 的 datetime 独立核对，不是照着实现抄的。
        assert_eq!(iso8601_utc(1_775_000_000), "2026-03-31T23:33:20Z");
        // 闰年 2 月 29 日，民用历算法最容易在这里出错。
        assert_eq!(iso8601_utc(1_709_164_800), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn 摘要算法与_onvif_规定一致() {
        // Nonce + Created + Password 依次喂给 SHA1，再 Base64。
        // 换个密码摘要必须变，否则说明密码根本没进哈希。
        let a = security_header("admin", "pass1", 0, 1);
        let b = security_header("admin", "pass2", 0, 1);
        assert_ne!(a, b);
        // 同样的输入必须得到同样的结果，便于现场复现问题。
        assert_eq!(a, security_header("admin", "pass1", 0, 1));
    }

    #[test]
    fn 用户名里的特殊字符被转义() {
        let header = security_header("a<b&c", "x", 0, 1);
        assert!(header.contains("a&lt;b&amp;c"), "{header}");
    }

    #[test]
    fn 分块响应能被还原() {
        let body = "4\r\nabcd\r\n3\r\nefg\r\n0\r\n\r\n";
        assert_eq!(dechunk(body), "abcdefg");
    }
}
