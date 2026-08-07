//! 数据地图的归属统计薄包装。
//!
//! 真正的归属判定复用各模块已测试的纯函数（`hr::linked_id_counts` 等），
//! 这里只负责把不同表的外键归一化成同一种输入。

use std::collections::BTreeSet;

use crate::pages::hr::linked_id_counts;

/// 0哨兵外键的归属统计：`id == 0` 表示"未分配"，和指向已失效目标一样
/// 都算指不上——这两种情况对读图的人是同一件事：这行数据不落在任何
/// 一个有效父级之下。
#[pure_function::pure]
pub(super) fn sentinel_link_counts(
    ids: impl Iterator<Item = u64>,
    active_ids: &BTreeSet<u64>,
) -> (usize, usize) {
    linked_id_counts(ids.map(|id| (id != 0).then_some(id)), active_ids)
}

/// 可选外键的三桶归属统计：已挂靠 / 未分配（None，业务上合法）/
/// 悬空（Some 但目标不存在，真孤儿）。
///
/// 门禁车辆、访客经 `services::park_ref` 转成 `Option` 之后，None 是
/// "没登记园区"的正常状态，不能跟"指向已删园区"混为一谈——前者标注，
/// 后者报警。
#[pure_function::pure]
pub(super) fn optional_link_counts(
    ids: impl Iterator<Item = Option<u64>>,
    active_ids: &BTreeSet<u64>,
) -> (usize, usize, usize) {
    let mut linked = 0;
    let mut unassigned = 0;
    let mut dangling = 0;
    for id in ids {
        match id {
            Some(id) if active_ids.contains(&id) => linked += 1,
            Some(_) => dangling += 1,
            None => unassigned += 1,
        }
    }
    (linked, unassigned, dangling)
}

/// 水电表按安装位置分桶：挂厂房层 / 挂宿舍层 / 园区公共区域。
///
/// 三个桶都是合法状态，没有一个是孤儿——「公共用电」那类表本来就不属于
/// 任何一层。分开数是为了让读图的人知道台账补录到了哪一步，而不是把
/// 「还没挂到层上」和「数据坏了」混成一个数字。
#[pure_function::pure]
pub(super) fn meter_location_counts(
    locations: impl Iterator<Item = (u64, u64)>,
) -> (usize, usize, usize) {
    let mut on_factory = 0;
    let mut on_dormitory = 0;
    let mut public = 0;
    for (factory_floor_id, dormitory_floor_id) in locations {
        if factory_floor_id != 0 {
            on_factory += 1;
        } else if dormitory_floor_id != 0 {
            on_dormitory += 1;
        } else {
            public += 1;
        }
    }
    (on_factory, on_dormitory, public)
}

/// 把「百分之一平方米」的内部单位格式化成带千位分隔的整数平方米。
///
/// 地图上这些面积是几十万级别的对照数字，小数位没有意义，反而挤占版面。
#[pure_function::pure]
pub(super) fn format_area_square_metres(centi: i64) -> String {
    let whole = centi / 100;
    let digits = whole.abs().to_string();
    let grouped = digits
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(",");
    if whole < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 面积格式化按千位分隔且舍去小数() {
        assert_eq!(format_area_square_metres(51_910_500), "519,105");
        assert_eq!(format_area_square_metres(9_999), "99");
        assert_eq!(format_area_square_metres(0), "0");
    }

    #[test]
    fn 零哨兵与失效目标都算指不上() {
        let active = BTreeSet::from([7_u64]);
        // 7 挂靠成功；0 是哨兵；999 指向不存在的父级
        assert_eq!(sentinel_link_counts([7, 0, 999].into_iter(), &active), (1, 2));
    }

    #[test]
    fn 水电表按安装位置分三桶() {
        // 两个位置都为 0 是公共区域的表，不是漏填。
        assert_eq!(
            meter_location_counts([(3, 0), (0, 5), (0, 0), (0, 0)].into_iter()),
            (1, 1, 2)
        );
    }

    #[test]
    fn 可选外键区分未分配和悬空() {
        let active = BTreeSet::from([7_u64]);
        assert_eq!(
            optional_link_counts([Some(7), None, Some(999)].into_iter(), &active),
            (1, 1, 1)
        );
    }
}
