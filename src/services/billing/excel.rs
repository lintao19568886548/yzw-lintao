//! 服务端生成账单 Excel，客户端只负责触发下载。

use dioxus::prelude::*;

use super::types::{BillExcelRow, GeneratedBillingFile};

#[post("/api/billing/excel/export")]
pub async fn export_amount_bill_excel(
    token: String,
    rows: Vec<BillExcelRow>,
) -> Result<GeneratedBillingFile, ServerFnError> {
    #[cfg(feature = "server")]
    {
        super::super::storage::validate_admin_access(&token).await?;
        if rows.len() > 50_000 {
            return Err(ServerFnError::new("单次最多导出 50000 条账单"));
        }

        Ok(GeneratedBillingFile {
            file_name: "云园慧控-总账单.xlsx".into(),
            content_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                .into(),
            bytes: build_excel_file(&rows)?,
        })
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (token, rows);
        Err(ServerFnError::new("Excel 导出接口只能在服务端执行"))
    }
}

#[cfg(feature = "server")]
fn build_excel_file(rows: &[BillExcelRow]) -> Result<Vec<u8>, ServerFnError> {
    use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook};

    let mut workbook = Workbook::new();
    let worksheet = workbook.add_worksheet();
    worksheet
        .set_name("总账单")
        .map_err(|error| ServerFnError::new(format!("创建工作表失败：{error}")))?;
    worksheet
        .set_freeze_panes(1, 0)
        .map_err(|error| ServerFnError::new(format!("设置 Excel 冻结行失败：{error}")))?;

    let header_format = Format::new()
        .set_bold()
        .set_font_color(Color::White)
        .set_background_color(Color::RGB(0x174D3C))
        .set_align(FormatAlign::Center)
        .set_border(FormatBorder::Thin);
    let money_format = Format::new()
        .set_num_format("¥#,##0.00;[Red]-¥#,##0.00")
        .set_border(FormatBorder::Thin);
    let text_format = Format::new().set_border(FormatBorder::Thin);
    let headers = [
        "园区",
        "项目名称",
        "租户名称",
        "电费",
        "水费",
        "厂房租金",
        "管理费",
        "服务费",
        "垃圾费",
        "开票税金",
        "滞纳金",
        "其他应收",
        "应收合计",
        "实收合计",
        "未收金额",
        "收款状态",
        "收款日期",
        "备注",
    ];
    for (column, title) in headers.iter().enumerate() {
        worksheet
            .write_string_with_format(0, column as u16, *title, &header_format)
            .map_err(|error| ServerFnError::new(format!("写入表头失败：{error}")))?;
    }

    let cents = |value: i64| value as f64 / 100.0;
    for (index, row) in rows.iter().enumerate() {
        let excel_row = (index + 1) as u32;
        for (column, value) in [
            row.park_name.as_str(),
            row.project_name.as_str(),
            row.tenant_name.as_str(),
        ]
        .iter()
        .enumerate()
        {
            worksheet
                .write_string_with_format(excel_row, column as u16, *value, &text_format)
                .map_err(|error| ServerFnError::new(format!("写入账单文本失败：{error}")))?;
        }
        for (offset, value) in [
            row.ele_fee_cents,
            row.water_fee_cents,
            row.factory_rent_cents,
            row.management_fee_cents,
            row.service_fee_cents,
            row.garbage_fee_cents,
            row.invoice_tax_cents,
            row.penalty_fee_cents,
            row.receive_fee_cents,
            row.total_fee_cents,
            row.receipt_amount_cents,
            row.remaining_amount_cents,
        ]
        .iter()
        .enumerate()
        {
            worksheet
                .write_number_with_format(
                    excel_row,
                    (offset + 3) as u16,
                    cents(*value),
                    &money_format,
                )
                .map_err(|error| ServerFnError::new(format!("写入账单金额失败：{error}")))?;
        }
        for (column, value) in [
            (15u16, row.collection_status.as_str()),
            (16u16, row.receipt_date.as_str()),
            (17u16, row.remark.as_str()),
        ] {
            worksheet
                .write_string_with_format(excel_row, column, value, &text_format)
                .map_err(|error| ServerFnError::new(format!("写入账单状态失败：{error}")))?;
        }
    }

    for (column, width) in [
        (0, 18.0),
        (1, 28.0),
        (2, 20.0),
        (15, 14.0),
        (16, 14.0),
        (17, 36.0),
    ] {
        worksheet
            .set_column_width(column, width)
            .map_err(|error| ServerFnError::new(format!("设置列宽失败：{error}")))?;
    }
    for column in 3..15 {
        worksheet
            .set_column_width(column, 14.0)
            .map_err(|error| ServerFnError::new(format!("设置金额列宽失败：{error}")))?;
    }

    workbook
        .save_to_buffer()
        .map_err(|error| ServerFnError::new(format!("生成 Excel 失败：{error}")))
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use std::io::Cursor;

    use calamine::{Reader, Xlsx};

    use super::*;

    #[test]
    fn 导出的工作簿可重新打开且金额单位正确() {
        let rows = vec![BillExcelRow {
            park_name: "十一高步园区".into(),
            project_name: "8303".into(),
            tenant_name: "测试租户".into(),
            ele_fee_cents: 12_345,
            total_fee_cents: 12_345,
            remaining_amount_cents: 10_000,
            collection_status: "部分收款".into(),
            ..BillExcelRow::default()
        }];
        let bytes = build_excel_file(&rows).expect("应当生成 Excel");
        assert!(bytes.starts_with(b"PK\x03\x04"));

        let mut workbook: Xlsx<_> = calamine::open_workbook_from_rs(Cursor::new(bytes))
            .expect("生成结果应能作为 xlsx 打开");
        let sheet = workbook
            .worksheet_range("总账单")
            .expect("总账单工作表存在");
        assert_eq!(sheet.get_value((1, 0)).unwrap().to_string(), "十一高步园区");
        assert_eq!(sheet.get_value((1, 3)).unwrap().to_string(), "123.45");
        assert_eq!(sheet.get_value((1, 14)).unwrap().to_string(), "100");
    }
}
