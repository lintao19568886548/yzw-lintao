//! 中心系统配置键值表。

use spacetimedb::Timestamp;

/// 对应中心库 `key` 表，JSON 值使用规范化字符串保存。
#[spacetimedb::table(
    accessor = center_system_key,
    index(accessor = center_system_key_by_scope, btree(columns = [center_scope])),
    index(accessor = center_system_key_by_name, btree(columns = [key_name]))
)]
pub struct CenterSystemKey {
    #[primary_key]
    #[auto_inc]
    pub key_id: u64,
    /// 中心库全局分区标识，固定为 `0`。
    pub center_scope: u8,
    pub key_name: String,
    pub value_json: String,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
