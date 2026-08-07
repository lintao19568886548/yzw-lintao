//! MySQL 兼容字段的输入规范化。

#[pure_function::pure]
pub(crate) fn required_text(value: String, message: &'static str) -> Result<String, String> {
    normalize_optional_text(Some(value)).ok_or(message.into())
}

#[pure_function::pure]
pub(crate) fn normalize_optional_text(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// MySQL 历史数据中除 `0` 外的状态统一视为启用。
#[pure_function::pure]
pub(crate) fn normalize_status(value: i8) -> i8 {
    i8::from(value != 0)
}

#[pure_function::pure]
pub(crate) fn validate_max_length(
    value: &str,
    max_chars: usize,
    message: &'static str,
) -> Result<(), String> {
    (value.chars().count() <= max_chars)
        .then_some(())
        .ok_or(message.into())
}

/// 规范化可选文本并在超长时报错，是多处「可空且限长」字段的通用校验。
#[pure_function::pure]
pub(crate) fn limited_optional_text(
    value: Option<String>,
    max_chars: usize,
    message: &'static str,
) -> Result<Option<String>, String> {
    let value = normalize_optional_text(value);
    if let Some(value) = value.as_deref() {
        validate_max_length(value, max_chars, message)?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 必填文本会去除首尾空格() {
        assert_eq!(
            required_text("  Super  ".into(), "缺失"),
            Ok("Super".into())
        );
        assert_eq!(required_text("   ".into(), "缺失"), Err("缺失".into()));
    }

    #[test]
    fn 状态值兼容_mysql_历史数据() {
        assert_eq!(normalize_status(0), 0);
        assert_eq!(normalize_status(1), 1);
        assert_eq!(normalize_status(-1), 1);
    }

    /// 接住纯函数命名约定的另一半：`check_*` / `normalize_*` 是纯函数命名，
    /// 必须带 `#[pure_function::pure]` 让宏在编译期检查它确实不碰 `ctx`。
    /// 需要 `ctx` 的校验函数请按惯例命名为 `require_*` 或 `validated_*`。
    #[test]
    fn 所有名为_check_或_normalize_的函数都已标记为纯函数() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        // 模式在运行期拼接，避免匹配到本测试自身的源码。
        let patterns = [format!("fn {}", "check_"), format!("fn {}", "normalize_")];
        let mut missing = Vec::new();
        for path in rust_files(&root) {
            let source = std::fs::read_to_string(&path).expect("源码文件应当可读");
            let lines: Vec<&str> = source.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//")
                    || !patterns.iter().any(|pattern| trimmed.contains(pattern.as_str()))
                {
                    continue;
                }
                let marked = lines[..index]
                    .iter()
                    .rev()
                    .map(|prev| prev.trim())
                    .take_while(|prev| {
                        prev.starts_with("#[") || prev.starts_with("///") || prev.starts_with("//")
                    })
                    .any(|prev| prev == "#[pure_function::pure]" || prev == "#[pure]");
                if !marked {
                    missing.push(format!("{}:{} {}", path.display(), index + 1, trimmed));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "这些函数按命名应为纯函数，但没有 #[pure_function::pure] 标记：\n{}",
            missing.join("\n")
        );
    }

    fn rust_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).expect("源码目录应当可读") {
            let path = entry.expect("目录项应当可读").path();
            if path.is_dir() {
                files.extend(rust_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
        files
    }
}
