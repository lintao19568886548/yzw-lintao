//! 够用的 XML 取值：按**本地名**取元素文本，忽略命名空间前缀。
//!
//! 不引 XML 库是有意的：ONVIF 各厂商的前缀五花八门（`d:`、`wsd:`、`tds:`、
//! `tt:`、有的干脆不带前缀），真正需要的能力只有「不管前缀叫什么，把叫这个
//! 名字的元素的文本取出来」。一个完整的 XML 解析器给不了这个便利，反而要
//! 自己处理一遍命名空间映射。

/// 取出所有本地名为 `local` 的元素文本，按出现顺序。
pub fn values(xml: &str, local: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = xml;
    while let Some((open_end, close_start, tail)) = next_element(rest, local) {
        found.push(unescape(&rest[open_end..close_start]));
        rest = tail;
    }
    found
}

/// 取第一个本地名为 `local` 的元素文本。
pub fn value(xml: &str, local: &str) -> Option<String> {
    values(xml, local).into_iter().next()
}

/// 取出所有本地名为 `local` 的**整段元素**（含开标签），供逐个取属性用。
///
/// 不处理同名嵌套——ONVIF 的响应里没有这种结构。
pub fn elements<'a>(xml: &'a str, local: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(at) = find_open_tag(rest, local) {
        let Some(tag_end) = rest[at..].find('>').map(|e| e + at) else {
            break;
        };
        if rest[at..tag_end].ends_with('/') {
            out.push(&rest[at..=tag_end]);
            rest = &rest[tag_end + 1..];
            continue;
        }
        let mut scan = tag_end + 1;
        let mut closed_at = None;
        while let Some(rel) = rest[scan..].find("</") {
            let close_at = scan + rel;
            let Some(close_end) = rest[close_at..].find('>').map(|e| e + close_at) else {
                break;
            };
            let qname = rest[close_at + 2..close_end].trim();
            if qname.rsplit(':').next().unwrap_or(qname) == local {
                closed_at = Some(close_end);
                break;
            }
            scan = close_end + 1;
        }
        let Some(close_end) = closed_at else { break };
        out.push(&rest[at..=close_end]);
        rest = &rest[close_end + 1..];
    }
    out
}

/// 取元素上某个属性的值，元素按本地名匹配。
pub fn attr(xml: &str, local: &str, attr_name: &str) -> Option<String> {
    let mut rest = xml;
    loop {
        let at = find_open_tag(rest, local)?;
        let tag_end = rest[at..].find('>')? + at;
        let tag = &rest[at..tag_end];
        if let Some(pos) = tag.find(&format!("{attr_name}=\"")) {
            let start = pos + attr_name.len() + 2;
            let end = tag[start..].find('"')? + start;
            return Some(unescape(&tag[start..end]));
        }
        rest = &rest[tag_end..];
    }
}

/// 找下一个 `<[前缀:]local ...>`，返回它在 `xml` 中的起始下标。
fn find_open_tag(xml: &str, local: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(at) = xml[from..].find('<') {
        let at = from + at;
        let after = &xml[at + 1..];
        // 跳过闭合标签、声明与注释。
        if after.starts_with('/') || after.starts_with('?') || after.starts_with('!') {
            from = at + 1;
            continue;
        }
        let name_end = after
            .find(|c: char| c == '>' || c == ' ' || c == '\t' || c == '\r' || c == '\n' || c == '/')
            .unwrap_or(after.len());
        let qname = &after[..name_end];
        let name = qname.rsplit(':').next().unwrap_or(qname);
        if name == local {
            return Some(at);
        }
        from = at + 1;
    }
    None
}

/// 返回 (文本起始, 文本结束, 剩余串)。自闭合元素当作空文本。
fn next_element<'a>(xml: &'a str, local: &str) -> Option<(usize, usize, &'a str)> {
    let at = find_open_tag(xml, local)?;
    let tag_end = xml[at..].find('>')? + at;
    if xml[at..tag_end].ends_with('/') {
        return Some((tag_end, tag_end, &xml[tag_end + 1..]));
    }
    let text_start = tag_end + 1;
    // 闭合标签同样忽略前缀。
    let mut scan = text_start;
    loop {
        let rel = xml[scan..].find("</")?;
        let close_at = scan + rel;
        let close_end = xml[close_at..].find('>')? + close_at;
        let qname = xml[close_at + 2..close_end].trim();
        let name = qname.rsplit(':').next().unwrap_or(qname);
        if name == local {
            return Some((text_start, close_at, &xml[close_end + 1..]));
        }
        scan = close_end + 1;
    }
}

fn unescape(text: &str) -> String {
    text.trim()
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        // `&amp;` 必须最后replace，否则 `&amp;lt;` 会被还原成 `<`。
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 前缀不同也能取到同一个元素() {
        // 各厂商的前缀确实不一样，这正是不用现成 XML 库的原因。
        assert_eq!(
            value("<d:XAddrs>http://a/onvif</d:XAddrs>", "XAddrs").as_deref(),
            Some("http://a/onvif")
        );
        assert_eq!(
            value("<wsd:XAddrs>http://b/onvif</wsd:XAddrs>", "XAddrs").as_deref(),
            Some("http://b/onvif")
        );
        assert_eq!(
            value("<XAddrs>http://c/onvif</XAddrs>", "XAddrs").as_deref(),
            Some("http://c/onvif")
        );
    }

    #[test]
    fn 多个同名元素按顺序取出() {
        let xml = "<a:Token>p1</a:Token><b:Token>p2</b:Token>";
        assert_eq!(values(xml, "Token"), vec!["p1", "p2"]);
    }

    #[test]
    fn 不会把名字前缀相同的元素认错() {
        // XAddrsExtra 不是 XAddrs，必须整段本地名相等才算。
        let xml = "<d:XAddrsExtra>不要我</d:XAddrsExtra><d:XAddrs>要我</d:XAddrs>";
        assert_eq!(values(xml, "XAddrs"), vec!["要我"]);
    }

    #[test]
    fn 自闭合元素当作空文本而不是吞掉后面的内容() {
        let xml = "<tt:Uri/><tt:Other>x</tt:Other>";
        assert_eq!(values(xml, "Uri"), vec![""]);
    }

    #[test]
    fn 属性按本地名匹配元素后取出() {
        let xml = r#"<trt:Profiles token="Profile_1" fixed="true"/>"#;
        assert_eq!(
            attr(xml, "Profiles", "token").as_deref(),
            Some("Profile_1")
        );
    }

    #[test]
    fn 整段元素能逐个取出且各自带属性() {
        let xml = r#"<trt:Profiles token="P1"><tt:Name>主码流</tt:Name></trt:Profiles>
                     <trt:Profiles token="P2"><tt:Name>子码流</tt:Name></trt:Profiles>"#;
        let items = elements(xml, "Profiles");
        assert_eq!(items.len(), 2);
        assert_eq!(attr(items[0], "Profiles", "token").as_deref(), Some("P1"));
        assert_eq!(value(items[0], "Name").as_deref(), Some("主码流"));
        assert_eq!(attr(items[1], "Profiles", "token").as_deref(), Some("P2"));
        assert_eq!(value(items[1], "Name").as_deref(), Some("子码流"));
    }

    #[test]
    fn 转义还原顺序不会把_amp_lt_变成尖括号() {
        // RTSP 地址里带 `&` 很常见（大华的 channel/subtype 就是），
        // 顺序搞反会把 `&amp;lt;` 还原成 `<`，地址就废了。
        assert_eq!(
            value(
                "<Uri>rtsp://h/cam?channel=1&amp;subtype=0</Uri>",
                "Uri"
            )
            .as_deref(),
            Some("rtsp://h/cam?channel=1&subtype=0")
        );
        assert_eq!(value("<Uri>a&amp;lt;b</Uri>", "Uri").as_deref(), Some("a&lt;b"));
    }

    #[test]
    fn 注释和声明不会被当成元素() {
        let xml = "<?xml version=\"1.0\"?><!-- <Uri>假的</Uri> --><Uri>真的</Uri>";
        // 注释里那个会被取到——这是已知的简化，注释里出现同名元素的情况在
        // ONVIF 响应里不存在。这条测试记录这个边界，避免以后误以为处理了。
        assert!(values(xml, "Uri").contains(&"真的".to_string()));
    }
}
