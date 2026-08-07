//! 以 `YYYY-MM-DD` 字符串为接口的日期与时间字段。
//!
//! 业务侧到处用的都是 `YYYY-MM-DD` 字符串（服务端也按这个格式解析），而组件库的
//! `DatePicker` 收发 `time::Date`。这里做一层适配，页面不必各自处理转换。
//!
//! 时间字段没有对应的组件库原语，用两个 `Select` 拼出来——原生 `input[type=time]`
//! 会弹出操作系统自带的选择器，配色和语言都不受控，和其余控件对不上。

use dioxus::prelude::*;
use time::{Date, Month};

use crate::components::{
    date_picker::DatePicker,
    select::{Select, SelectOption},
};

/// 把 `YYYY-MM-DD` 解析成日期；格式不对时返回 `None`，由调用方当作未填写。
fn parse_ymd(value: &str) -> Option<Date> {
    let mut parts = value.trim().split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}

fn format_ymd(value: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        value.year(),
        value.month() as u8,
        value.day()
    )
}

/// 日期字段：值和回调都是 `YYYY-MM-DD` 字符串，清空时回调空串。
#[component]
pub fn DateField(
    value: String,
    #[props(default)] disabled: bool,
    /// 可选日期下界，同样是 `YYYY-MM-DD`。
    #[props(default)]
    min: Option<String>,
    /// 可选日期上界。
    #[props(default)]
    max: Option<String>,
    on_change: EventHandler<String>,
) -> Element {
    // 必须用 use_reactive 把 value 声明成依赖。`value` 是普通 String 而不是信号，
    // 直接 `use_memo(move || ...)` 只会在首次渲染求值一次并永远缓存——调用方后来
    // 改了日期（例如「同步今日」）组件不会有任何反应。
    let selected: ReadSignal<Option<Date>> =
        use_memo(use_reactive!(|(value,)| parse_ymd(&value))).into();
    // 组件库要求上下界是具体日期，缺省时给一个足够宽的区间。
    let min_date = min
        .as_deref()
        .and_then(parse_ymd)
        .unwrap_or_else(|| Date::from_calendar_date(1970, Month::January, 1).expect("valid"));
    let max_date = max
        .as_deref()
        .and_then(parse_ymd)
        .unwrap_or_else(|| Date::from_calendar_date(2060, Month::December, 31).expect("valid"));

    rsx! {
        DatePicker {
            selected_date: selected,
            disabled,
            min_date,
            max_date,
            on_value_change: move |date: Option<Date>| {
                on_change.call(date.map(format_ymd).unwrap_or_default());
            },
        }
    }
}

/// 分钟下拉的候选值：常用的 5 分钟档，外加当前已有的非整点分钟。
///
/// 只给 5 分钟档会让 `08:32` 这类既有数据在下拉里无处可选，保存时被悄悄改掉；
/// 给满 60 项又要在一个长列表里翻找。这里两者兼顾。
fn minute_options(current: &str) -> Vec<String> {
    let mut options = (0..12)
        .map(|step| format!("{:02}", step * 5))
        .collect::<Vec<_>>();
    if !current.is_empty() && !options.iter().any(|value| value == current) {
        options.push(current.to_string());
        options.sort();
    }
    options
}

/// 时间字段：值和回调都是 `HH:MM`，未填写时为空串。
#[component]
pub fn TimeField(
    value: String,
    #[props(default)] disabled: bool,
    on_change: EventHandler<String>,
) -> Element {
    let hour = value.split(':').next().unwrap_or_default().to_string();
    let minute = value.split(':').nth(1).unwrap_or_default().to_string();
    let hour_for_change = hour.clone();
    let minute_for_change = minute.clone();
    let minutes = minute_options(&minute);
    // 同 DateField：不声明依赖的话，外部改写 value 后两个下拉不会跟着变。
    let hour_value: ReadSignal<Option<String>> =
        use_memo(use_reactive!(|(hour,)| Some(hour))).into();
    let minute_value: ReadSignal<Option<String>> =
        use_memo(use_reactive!(|(minute,)| Some(minute))).into();

    rsx! {
        div { class: "time-field",
            Select {
                value: Some(hour_value),
                disabled,
                on_value_change: move |next: Option<String>| {
                    let next = next.unwrap_or_default();
                    if next.is_empty() {
                        on_change.call(String::new());
                        return;
                    }
                    // 只选了小时就把分钟补成 00，否则回调出去的是半个时间。
                    let minute = if minute_for_change.is_empty() { "00" } else { minute_for_change.as_str() };
                    on_change.call(format!("{next}:{minute}"));
                },
                SelectOption::<String> { value: String::new(), index: 0usize, text_value: "时".to_string(), "时" }
                for hour_index in 0..24usize {
                    SelectOption::<String> {
                        key: "hour-{hour_index}",
                        value: format!("{hour_index:02}"),
                        index: hour_index + 1,
                        text_value: format!("{:02}", hour_index), "{hour_index:02}"
                    }
                }
            }
            span { class: "time-field-separator", ":" }
            Select {
                value: Some(minute_value),
                disabled,
                on_value_change: move |next: Option<String>| {
                    let next = next.unwrap_or_default();
                    let hour = if hour_for_change.is_empty() { "00" } else { hour_for_change.as_str() };
                    if next.is_empty() {
                        on_change.call(format!("{hour}:00"));
                        return;
                    }
                    on_change.call(format!("{hour}:{next}"));
                },
                SelectOption::<String> { value: String::new(), index: 0usize, text_value: "分".to_string(), "分" }
                for (index , value) in minutes.into_iter().enumerate() {
                    SelectOption::<String> {
                        key: "minute-{value}",
                        value: value.clone(),
                        index: index + 1,
                        text_value: value.to_string(), "{value}"
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 日期字符串可以往返() {
        let date = parse_ymd("2026-07-09").expect("应当解析成功");
        assert_eq!(format_ymd(date), "2026-07-09");
    }

    #[test]
    fn 分钟下拉保留既有的非整点取值() {
        let common = minute_options("");
        assert_eq!(common.len(), 12);
        assert_eq!(common.first().map(String::as_str), Some("00"));
        assert_eq!(common.last().map(String::as_str), Some("55"));

        // 已有 08:32 这类数据时，32 必须出现在候选里，否则保存会被改成别的值。
        let with_existing = minute_options("32");
        assert_eq!(with_existing.len(), 13);
        assert!(with_existing.iter().any(|value| value == "32"));

        // 已经在 5 分钟档里的值不重复添加。
        assert_eq!(minute_options("30").len(), 12);
    }

    #[test]
    fn 非法日期按未填写处理() {
        assert!(parse_ymd("").is_none());
        assert!(parse_ymd("2026-13-01").is_none());
        assert!(parse_ymd("2026-02-30").is_none());
        assert!(parse_ymd("2026-07-09-01").is_none());
    }
}
