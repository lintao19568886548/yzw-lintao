//! 园区档案页共用的展示规则。

use crate::spacetime_bindings::park_type::Park;

#[pure_function::pure]
pub(super) fn status_label(park: &Park) -> String {
    park.status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("正常")
        .to_string()
}

#[pure_function::pure]
pub(super) fn status_is_enabled(park: &Park) -> bool {
    !matches!(
        park.status.as_deref().map(str::trim),
        Some("0" | "disabled" | "停用")
    )
}

#[pure_function::pure]
pub(super) fn format_area(value: i64) -> String {
    let value = value.max(0);
    if value % 100 == 0 {
        (value / 100).to_string()
    } else {
        format!("{}.{:02}", value / 100, value % 100)
            .trim_end_matches('0')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 园区面积最多保留两位小数() {
        assert_eq!(format_area(12_300), "123");
        assert_eq!(format_area(12_345), "123.45");
    }
}
