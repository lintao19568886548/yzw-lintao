//! 总览页面使用的显示格式转换。

pub fn format_money(cents: i64) -> String {
    let yuan = cents as f64 / 100.0;
    let absolute = yuan.abs();
    if absolute >= 100_000_000.0 {
        format!("{:.2}亿", yuan / 100_000_000.0)
    } else if absolute >= 10_000.0 {
        format!("{:.1}万", yuan / 10_000.0)
    } else {
        format!("{yuan:.2}")
    }
}

pub fn format_area(centi_square_metres: i64) -> String {
    let square_metres = centi_square_metres as f64 / 100.0;
    if square_metres >= 10_000.0 {
        format!("{:.1}万㎡", square_metres / 10_000.0)
    } else {
        format!("{square_metres:.0}㎡")
    }
}

pub fn collection_rate(receivable_cents: i64, received_cents: i64) -> u32 {
    if receivable_cents <= 0 {
        return 0;
    }
    ((received_cents.max(0) as f64 / receivable_cents as f64) * 100.0)
        .round()
        .clamp(0.0, 100.0) as u32
}
