//! 应用页面。

mod access_control;
mod contract;
mod dashboard;
mod data_map;
mod device;
mod dynamic_workspace;
mod finance;
mod hr;
mod login;
mod maintenance;
mod permission_management;
mod profile;
mod reimbursement;
mod rental_list;
mod rental_management;
mod salary;
mod smart_meter;
mod tenant_management;

pub use contract::*;
pub use dashboard::*;
pub use data_map::*;
pub use device::*;
pub use dynamic_workspace::*;
pub use finance::*;
pub use hr::*;
pub use login::*;
pub use maintenance::*;
pub use permission_management::*;
pub use profile::*;
pub use reimbursement::*;
pub use rental_list::*;
pub use rental_management::*;
pub use salary::*;
pub use smart_meter::*;
pub use tenant_management::*;
mod billing;
pub use access_control::*;
pub use billing::*;

#[cfg(test)]
mod pure_marking_tests {
    /// model.rs 是页面的函数式核心：公开函数要么是纯函数（必须标记
    /// `#[pure_function::pure]`），要么读时钟（必须以 `now_` / `today` 命名）。
    #[test]
    fn model_rs_的公开函数都已标记为纯函数或按时钟命名() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/pages");
        let mut missing = Vec::new();
        for path in model_files(&root) {
            let source = std::fs::read_to_string(&path).expect("源码文件应当可读");
            let lines: Vec<&str> = source.lines().collect();
            for (index, line) in lines.iter().enumerate() {
                let trimmed = line.trim_start();
                let Some(name) = public_fn_name(trimmed) else { continue };
                if name.starts_with("now_") || name.starts_with("today") {
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
            "model.rs 的公开函数应当标记 #[pure_function::pure]（读时钟的函数请以 now_/today 命名）：\n{}",
            missing.join("\n")
        );
    }

    /// 提取 `pub` / `pub(crate)` / `pub(super)` 函数名；非公开函数返回 None。
    fn public_fn_name(trimmed: &str) -> Option<&str> {
        let rest = trimmed.strip_prefix("pub")?;
        let rest = rest
            .strip_prefix("(crate)")
            .or_else(|| rest.strip_prefix("(super)"))
            .unwrap_or(rest);
        let rest = rest.trim_start().strip_prefix("fn ")?;
        rest.split(|c: char| !c.is_alphanumeric() && c != '_').next()
    }

    fn model_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else { return files };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(model_files(&path));
            } else if path.file_name().is_some_and(|name| name == "model.rs") {
                files.push(path);
            }
        }
        files.sort();
        files
    }
}
