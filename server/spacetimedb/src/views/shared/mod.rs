//! 视图层的公共依赖。
//!
//! `current_read_scope` 是每个 view 的第一行——它把「当前是谁、能看哪些园区、
//! 有哪些职能」解析成一个 `ReadScope`。放在业务分组之外，是因为它不属于任何一
//! 个域，而是所有域共用的入口。

pub(crate) mod identity;
