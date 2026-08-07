//! 园区外键的统一写法（与服务端 `reducers::park_ref` 对应）。
//!
//! 所有园区外键都是 `u64`，用 [`NO_PARK`]（0）表示「没有园区」——不是偏好，
//! 是 SpacetimeDB 逼出来的：索引过滤参数必须实现 `FilterableValue`，而
//! `Option<u64>` 没有实现，可空列即使声明了索引也只能全表扫。
//!
//! 客户端只是消费者，但同样不要手写 `park_id != 0`——判断散在各页面里，
//! 改语义时找不全。

/// 「没有园区」。
pub const NO_PARK: u64 = 0;

/// 园区外键是否指向了某个园区。
pub fn has_park(park_id: u64) -> bool {
    park_id != NO_PARK
}

/// 园区外键转成 `Option`，供 `find`／归属统计一类只认 `Option` 的调用方使用。
pub fn park_ref(park_id: u64) -> Option<u64> {
    has_park(park_id).then_some(park_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 零表示没有园区() {
        assert!(!has_park(NO_PARK));
        assert_eq!(park_ref(NO_PARK), None);
        assert!(has_park(7));
        assert_eq!(park_ref(7), Some(7));
    }
}
