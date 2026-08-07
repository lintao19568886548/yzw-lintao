//! 资产表单共用的定点数解析。
//!
//! 面积、层高、承重和租金在服务端一律以「乘一百的整数」存储，界面按原单位
//! 输入输出。等 `/rental/manage` 的旧弹窗表单退役后，那边的同名实现即可删除。

/// 把乘一百的整数还原为界面输入值，整数部分为零的小数保留两位。
pub(super) fn decimal_input(value: Option<i64>) -> String {
    value
        .map(|v| {
            if v % 100 == 0 {
                (v / 100).to_string()
            } else {
                format!("{}.{:02}", v / 100, v % 100)
                    .trim_end_matches('0')
                    .to_string()
            }
        })
        .unwrap_or_default()
}

/// 解析界面输入为乘一百的整数。
///
/// `optional` 为真时允许留空，返回 `None`；否则空值报错。
pub(super) fn parse_decimal(
    value: &str,
    label: &str,
    optional: bool,
) -> Result<Option<i64>, String> {
    let value = value.trim();
    if value.is_empty() {
        return if optional {
            Ok(None)
        } else {
            Err(format!("请输入{label}"))
        };
    }
    if value.starts_with('-') {
        return Err(format!("{label}不能为负数"));
    }
    let mut parts = value.split('.');
    let whole = parts
        .next()
        .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|v| v.parse::<i64>().ok())
        .ok_or_else(|| format!("{label}格式不正确"))?;
    let fraction = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return Err(format!("{label}格式不正确"));
    }
    if fraction.len() > 2 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{label}最多保留两位小数"));
    }
    let scaled = format!("{fraction:0<2}")
        .parse::<i64>()
        .map_err(|_| format!("{label}格式不正确"))?;
    whole
        .checked_mul(100)
        .and_then(|v| v.checked_add(scaled))
        .map(Some)
        .ok_or_else(|| format!("{label}超出可表示范围"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 定点数输入与还原可往返() {
        assert_eq!(decimal_input(Some(1250)), "12.5");
        assert_eq!(decimal_input(Some(1200)), "12");
        assert_eq!(decimal_input(None), "");
        assert_eq!(parse_decimal("12.5", "层高", false), Ok(Some(1250)));
        assert_eq!(parse_decimal("12", "层高", false), Ok(Some(1200)));
    }

    #[test]
    fn 非法定点数输入会被拒绝() {
        assert!(parse_decimal("-1", "层高", false).is_err());
        assert!(parse_decimal("1.234", "层高", false).is_err());
        assert!(parse_decimal("abc", "层高", false).is_err());
        assert!(parse_decimal("", "层高", false).is_err());
        assert_eq!(parse_decimal("", "层高", true), Ok(None));
    }
}
