//! 门禁业务共用校验。

/// 校验数据库中以 `0` 和 `1` 表示的二值状态。
///
/// 校验成功时原样返回状态值；失败时返回调用方提供的业务错误信息，
/// 便于不同门禁 Reducer 复用同一规则并保留各自的提示文案。
pub(super) fn validate_status(status: i8, message: &'static str) -> Result<i8, String> {
    matches!(status, 0 | 1)
        .then_some(status)
        .ok_or(message.into())
}

/// 清理并校验中国大陆手机号码。
///
/// 接受首位为 `1`、第二位为 `3` 至 `9` 且总长度为 11 的纯数字号码。
/// 返回值会移除用户输入两端的空白字符。
pub(super) fn validate_phone_number(phone_number: String) -> Result<String, String> {
    let phone_number = phone_number.trim().to_string();
    // 按字节检查是安全的：后续规则要求所有字符均为 ASCII 数字。
    let valid = phone_number.len() == 11
        && phone_number.starts_with('1')
        && phone_number
            .as_bytes()
            .get(1)
            .is_some_and(|value| matches!(value, b'3'..=b'9'))
        && phone_number.bytes().all(|value| value.is_ascii_digit());
    valid
        .then_some(phone_number)
        .ok_or("请输入正确的手机号码".into())
}
