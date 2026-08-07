//! 读取 Excel 文本并调用阿里云百炼识别账单草稿。

use dioxus::html::FileData;
use dioxus::prelude::*;
#[cfg(feature = "server")]
use serde_json::Value;

use super::types::AiAmountBillDraft;

const MAX_EXCEL_BYTES: u64 = 20 * 1024 * 1024;

pub fn validate_amount_bill_excel(file: &FileData) -> Result<(), String> {
    if file.size() == 0 || file.size() > MAX_EXCEL_BYTES {
        return Err("Excel 文件大小必须在 1 字节到 20MB 之间".into());
    }
    if !file.name().to_ascii_lowercase().ends_with(".xlsx") {
        return Err("AI Excel 导入仅支持 .xlsx 文件".into());
    }
    Ok(())
}

pub async fn analyze_amount_bill_excel(file: FileData) -> Result<Vec<AiAmountBillDraft>, String> {
    validate_amount_bill_excel(&file)?;
    let bytes = file
        .read_bytes()
        .await
        .map_err(|error| format!("读取 Excel 文件失败：{error}"))?;
    let token =
        crate::services::credentials::load_saved_token().ok_or("登录凭证不存在，请重新登录")?;
    analyze_amount_bill_excel_server(token, file.name(), bytes.to_vec())
        .await
        .map_err(|error| format!("AI Excel 导入失败：{error}"))
}

#[post("/api/billing/excel/analyze")]
async fn analyze_amount_bill_excel_server(
    token: String,
    file_name: String,
    bytes: Vec<u8>,
) -> Result<Vec<AiAmountBillDraft>, ServerFnError> {
    #[cfg(feature = "server")]
    {
        use std::io::Cursor;

        use calamine::{Reader, Xlsx};

        super::super::storage::validate_admin_access(&token).await?;
        if bytes.is_empty() || bytes.len() as u64 > MAX_EXCEL_BYTES {
            return Err(ServerFnError::new(
                "Excel 文件大小必须在 1 字节到 20MB 之间",
            ));
        }
        if !file_name.to_ascii_lowercase().ends_with(".xlsx") {
            return Err(ServerFnError::new("AI Excel 导入仅支持 .xlsx 文件"));
        }
        let mut workbook: Xlsx<_> = calamine::open_workbook_from_rs(Cursor::new(bytes))
            .map_err(|error| ServerFnError::new(format!("解析 Excel 失败：{error}")))?;
        let mut lines = vec![format!("Workbook: {file_name}")];
        let mut characters = lines[0].chars().count();
        for sheet_name in workbook.sheet_names().to_vec() {
            if characters >= 80_000 {
                break;
            }
            lines.push(format!("# Sheet: {sheet_name}"));
            match workbook.worksheet_range(&sheet_name) {
                Ok(range) => {
                    for (row_index, row) in range.rows().take(800).enumerate() {
                        let text = row
                            .iter()
                            .take(24)
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join("\t");
                        if text.trim().is_empty() {
                            continue;
                        }
                        let line = format!("R{}\t{text}", row_index + 1);
                        characters += line.chars().count();
                        if characters > 80_000 {
                            break;
                        }
                        lines.push(line);
                    }
                }
                Err(error) => {
                    lines.push(format!("读取工作表失败：{error}"));
                }
            }
        }
        let workbook_text = lines.join("\n");
        let key = std::env::var("ALIYUN_BAILIAN_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ServerFnError::new("服务端尚未配置 ALIYUN_BAILIAN_KEY"))?;
        let model =
            std::env::var("YIZU_AMOUNT_BILL_AI_MODEL").unwrap_or_else(|_| "qwen3.5-plus".into());
        let prompt = build_prompt(&workbook_text);
        let payload = serde_json::json!({
            "model": model,
            "temperature": 0,
            "response_format": { "type": "json_object" },
            "messages": [
                { "role": "system", "content": "你负责从 Excel 文本中提取园区租赁账单，只返回 JSON。" },
                { "role": "user", "content": prompt }
            ]
        });
        let response = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(280))
            .build()
            .map_err(|error| ServerFnError::new(format!("创建 AI 请求失败：{error}")))?
            .post("https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions")
            .bearer_auth(key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(payload.to_string())
            .send()
            .await
            .map_err(|error| ServerFnError::new(format!("连接阿里云百炼失败：{error}")))?;
        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|error| ServerFnError::new(format!("读取 AI 响应失败：{error}")))?;
        if !status.is_success() {
            let message = serde_json::from_str::<Value>(&response_text)
                .ok()
                .and_then(|value| {
                    value
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
            return Err(ServerFnError::new(format!("阿里云百炼拒绝请求：{message}")));
        }
        let response_json: Value = serde_json::from_str(&response_text)
            .map_err(|error| ServerFnError::new(format!("AI 响应格式无效：{error}")))?;
        let content = response_json
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| ServerFnError::new("AI 没有返回账单内容"))?;
        let parsed = parse_json_content(content)
            .ok_or_else(|| ServerFnError::new("AI 返回的账单 JSON 无法解析"))?;
        let bills = parsed
            .get("bills")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let drafts = bills
            .iter()
            .map(draft_from_json)
            .filter(|draft| {
                !draft.tenant_name.is_empty()
                    || !draft.project_name.is_empty()
                    || !draft.park_name.is_empty()
                    || draft.total_fee_cents > 0
            })
            .collect::<Vec<_>>();
        if drafts.is_empty() {
            return Err(ServerFnError::new("未从 Excel 识别到有效账单"));
        }
        Ok(drafts)
    }
    #[cfg(not(feature = "server"))]
    {
        let _ = (token, file_name, bytes);
        Err(ServerFnError::new("AI Excel 接口只能在服务端执行"))
    }
}

#[cfg(feature = "server")]
fn build_prompt(workbook_text: &str) -> String {
    format!(
        "请从以下 Excel 文本提取账单。不同园区、租户或账期必须拆成独立账单。未知字段用空字符串，金额保留数字。返回：\n\
         {{\"bills\":[{{\"parkName\":\"\",\"tenantName\":\"\",\"projectName\":\"\",\"publicBankAccount\":{{\"name\":\"\",\"number\":\"\",\"bank\":\"\"}},\"privateBankAccount\":{{\"name\":\"\",\"number\":\"\",\"bank\":\"\"}},\"eleFee\":\"\",\"waterFee\":\"\",\"factoryRent\":\"\",\"managementFee\":\"\",\"serviceFee\":\"\",\"garbageFee\":\"\",\"invoiceTax\":\"\",\"penaltyFee\":\"\",\"receiveFee\":\"\",\"totalFee\":\"\",\"receiptAmount\":\"\",\"receiptDate\":\"\",\"remark\":\"\"}}]}}\n\
         规则：实体名称必须简短，不要把“明细、通知单、账单”等整段标题当作名称；totalFee 缺失时按费用字段求和。\n\n{workbook_text}"
    )
}

#[cfg(feature = "server")]
fn parse_json_content(content: &str) -> Option<Value> {
    serde_json::from_str(content.trim()).ok().or_else(|| {
        let start = content.find('{')?;
        let end = content.rfind('}')?;
        serde_json::from_str(&content[start..=end]).ok()
    })
}

#[cfg(feature = "server")]
fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_f64().map(|number| number.to_string()))
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[cfg(feature = "server")]
fn cents(value: &Value, key: &str) -> i64 {
    let raw = text(value, key)
        .replace(',', "")
        .replace('￥', "")
        .replace('¥', "");
    raw.parse::<f64>()
        .ok()
        .filter(|number| number.is_finite() && *number >= 0.0)
        .map(|number| (number * 100.0).round() as i64)
        .unwrap_or_default()
}

#[cfg(feature = "server")]
fn account(value: &Value, key: &str) -> String {
    value
        .get(key)
        .filter(|account| account.is_object())
        .and_then(|account| serde_json::to_string(account).ok())
        .unwrap_or_default()
}

#[cfg(feature = "server")]
fn draft_from_json(value: &Value) -> AiAmountBillDraft {
    let mut draft = AiAmountBillDraft {
        park_name: text(value, "parkName"),
        tenant_name: text(value, "tenantName"),
        project_name: text(value, "projectName"),
        public_bank_account: account(value, "publicBankAccount"),
        private_bank_account: account(value, "privateBankAccount"),
        ele_fee_cents: cents(value, "eleFee"),
        water_fee_cents: cents(value, "waterFee"),
        factory_rent_cents: cents(value, "factoryRent"),
        management_fee_cents: cents(value, "managementFee"),
        service_fee_cents: cents(value, "serviceFee"),
        garbage_fee_cents: cents(value, "garbageFee"),
        invoice_tax_cents: cents(value, "invoiceTax"),
        penalty_fee_cents: cents(value, "penaltyFee"),
        receive_fee_cents: cents(value, "receiveFee"),
        total_fee_cents: cents(value, "totalFee"),
        receipt_amount_cents: cents(value, "receiptAmount"),
        receipt_date: text(value, "receiptDate"),
        remark: text(value, "remark"),
    };
    if draft.total_fee_cents == 0 {
        draft.total_fee_cents = [
            draft.ele_fee_cents,
            draft.water_fee_cents,
            draft.factory_rent_cents,
            draft.management_fee_cents,
            draft.service_fee_cents,
            draft.garbage_fee_cents,
            draft.invoice_tax_cents,
            draft.penalty_fee_cents,
            draft.receive_fee_cents,
        ]
        .into_iter()
        .fold(0i64, i64::saturating_add);
    }
    draft
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[test]
    fn 可以从代码围栏中提取_ai_json() {
        let parsed = parse_json_content("```json\n{\"bills\":[]}\n```").expect("应提取 JSON");
        assert!(parsed["bills"].as_array().is_some());
    }

    #[test]
    fn 金额转为分且缺少合计时自动求和() {
        let value = serde_json::json!({
            "parkName": "十一高步园区",
            "tenantName": "测试租户",
            "eleFee": "￥1,234.56",
            "waterFee": 20,
            "totalFee": ""
        });
        let draft = draft_from_json(&value);
        assert_eq!(draft.ele_fee_cents, 123_456);
        assert_eq!(draft.water_fee_cents, 2_000);
        assert_eq!(draft.total_fee_cents, 125_456);
    }
}
