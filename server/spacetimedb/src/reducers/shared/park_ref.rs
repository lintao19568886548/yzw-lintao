//! 园区外键的统一写法。
//!
//! # 为什么不是 `Option<u64>`
//!
//! SpacetimeDB 的索引过滤参数必须实现 `FilterableValue`，而它只对整数、`bool`、
//! 字符串、`Identity`、`Timestamp` 和无载荷枚举实现——**`Option<u64>` 不在其中**
//! （2.6.1 实测，编译期直接报 `cannot appear as an argument to an index filtering
//! operation`）。带 `Option<u64>` 的列即使声明了 btree 索引也调不出 `.filter()`，
//! 只能 `.iter()` 全表扫。
//!
//! 所以所有园区外键统一是 `u64`，用 [`NO_PARK`]（0）表示「没有园区」。这是被
//! 数据库逼出来的选择，不是偏好。
//!
//! # 代价与对策
//!
//! 代价是类型上分不出「这张表的 0 合法吗」——`u64` 就是 `u64`。对策是**不要手写
//! `if park_id != 0`**，一律走这里的函数：函数名就是那张表的语义，读代码的人不必
//! 回去翻表定义，改语义时也 grep 得全。
//!
//! - 校验入参，必填 → [`required_park_ref`]
//! - 校验入参，可空 → [`optional_park_ref`]
//! - 判断已存的行有没有园区 → [`has_park`]

use spacetimedb::ReducerContext;

use crate::{reducers::shared::access::require_park, tables::Park};

/// 「没有园区」。
///
/// 选 0 而不是 `u64::MAX` 是因为 `#[auto_inc]` 从 1 开始发号，0 永远不会是
/// 真实园区编号。
pub(crate) const NO_PARK: u64 = 0;

/// 已存的行有没有归属园区。
///
/// 读表时用它代替 `park_id != 0`——散在各处的裸比较改起来找不全，而这里改一次
/// 全都跟着变。
pub(crate) fn has_park(park_id: u64) -> bool {
    park_id != NO_PARK
}

/// 必填园区外键：[`NO_PARK`] 和不存在的园区都拒绝。
///
/// 返回园区记录，调用方通常还要用它的名字或地址。
pub(crate) fn required_park_ref(ctx: &ReducerContext, park_id: u64) -> Result<Park, String> {
    if park_id == NO_PARK {
        return Err("请选择所属园区".into());
    }
    require_park(ctx, park_id)
}

/// 可空园区外键：[`NO_PARK`] 原样放行，非 0 才校验园区存在。
///
/// 返回归一化后的园区编号——调用方直接把它写进行里，不要再用原始入参，否则
/// `Some(0)` 这类脏输入会绕过校验存进库。
pub(crate) fn optional_park_ref(ctx: &ReducerContext, park_id: u64) -> Result<u64, String> {
    if park_id != NO_PARK {
        require_park(ctx, park_id)?;
    }
    Ok(park_id)
}
