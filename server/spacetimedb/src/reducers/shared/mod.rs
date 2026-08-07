//! 跨域公用的 reducer 基础设施。
//!
//! 这四个模块不属于任何业务域，而是所有域都要用的东西：`access` 是身份与权限
//! 判定（被一百多处引用），`validation` 是字段校验，`park_ref` 是园区外键的统
//! 一写法，`bootstrap` 是初始化。放在业务分组之外，改动它们波及全局这件事才
//! 一眼看得出来。

pub(crate) mod access;
pub(crate) mod bootstrap;
pub(crate) mod park_ref;
pub(crate) mod validation;
