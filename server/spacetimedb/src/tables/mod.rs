//! 服务端数据表定义。
//!
//! # 分组
//!
//! 顶层是**业务分组**，与侧边栏的菜单分组一一对应，而不是二十多个域平铺在一层：
//!
//! | 目录 | 侧边栏 |
//! |---|---|
//! | `rental` | 租赁：园区管理、待租厂房、租户管理、合同管理 |
//! | `hr` | 人事：人事管理、工资管理、角色管理 |
//! | `finance` | 财务：财务管理、账单管理、报销管理与审核 |
//! | `access_control` | 门禁管理 |
//! | `device` | 设备管理 |
//! | `maintenance` | 维护管理 |
//! | `investment` | 招商（服务端已完成，前端尚无页面） |
//! | `platform` | 无对应菜单：多租户中心、菜单权限、图片、通知、系统配置 |
//!
//! 分组只是目录，不是模块边界——所有表仍然统一从 `crate::tables::*` 导出，
//! 调用方不需要知道某张表归在哪一组。挪动分组不会波及任何 `use`。

mod access_control;
mod device;
#[path = "finance/mod.rs"]
mod finance_tables;
mod hr;
#[path = "investment/mod.rs"]
mod investment_tables;
mod maintenance;
mod platform;
mod rental;

pub use access_control::*;
pub use device::*;
pub use finance_tables::*;
pub use hr::*;
pub use investment_tables::*;
pub use maintenance::*;
pub use platform::*;
pub use rental::*;

#[cfg(test)]
mod 追加列必须在末尾 {
    use std::{collections::BTreeMap, fs, path::Path};

    /// 带 `#[default(...)]` 的列一律是后来追加的，它们后面不能再有普通列。
    ///
    /// SpacetimeDB 2.6.1 按**列序**比对 schema：把新列插在结构体中间，发布时报
    /// 「Reordering table X requires a manual migration」直接中止。这个错误
    /// `cargo check` 看不见、`cargo test` 看不见、`spacetime build` 也看不见，
    /// 要到发布那一刻才暴露——而那时前端往往已经发上去了，线上会出现「前端绑定
    /// 有新列、模块没有」的错配。
    ///
    /// 判据之所以成立：新表一次定稿，不需要 `#[default]`；只有事后追加的列才写
    /// 它。因此「`#[default]` 之后又出现普通列」等价于「有人把追加列插在了中间」。
    #[test]
    fn 带默认值的列后面不能再有普通列() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tables");
        let mut offenders: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut scanned = 0usize;
        for path in rust_files(&root) {
            let source = fs::read_to_string(&path).expect("表定义文件应当可读");
            let mut in_struct = false;
            let mut seen_default = false;
            let mut pending_default = false;
            let mut struct_name = String::new();
            for line in source.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("pub struct ") {
                    in_struct = true;
                    seen_default = false;
                    pending_default = false;
                    struct_name = trimmed
                        .trim_start_matches("pub struct ")
                        .trim_end_matches(" {")
                        .to_string();
                    scanned += 1;
                    continue;
                }
                if !in_struct {
                    continue;
                }
                if trimmed == "}" {
                    in_struct = false;
                    continue;
                }
                if trimmed.starts_with("#[default(") {
                    pending_default = true;
                    continue;
                }
                // 其余属性（#[primary_key]、#[auto_inc]…）与注释不改变状态。
                if trimmed.starts_with('#') || trimmed.starts_with("//") || trimmed.is_empty() {
                    continue;
                }
                if !trimmed.starts_with("pub ") {
                    continue;
                }
                let field = trimmed
                    .trim_start_matches("pub ")
                    .split(':')
                    .next()
                    .unwrap_or(trimmed)
                    .to_string();
                if pending_default {
                    seen_default = true;
                } else if seen_default {
                    offenders
                        .entry(struct_name.clone())
                        .or_default()
                        .push(field);
                }
                pending_default = false;
            }
        }
        assert!(scanned > 0, "没扫到任何表结构，检查 {root:?} 是否存在");
        assert!(
            offenders.is_empty(),
            "这些列排在带 #[default] 的追加列之后，发布时会因列序变化被拒：\n{}\n\
             把追加列移到结构体末尾即可。",
            offenders
                .iter()
                .map(|(table, fields)| format!("  {table}: {}", fields.join("、")))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir).expect("表定义目录应当可读") {
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
