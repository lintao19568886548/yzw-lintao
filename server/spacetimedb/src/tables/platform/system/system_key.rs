//! 租户系统配置键值表。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `key` 表。
///
/// MySQL JSON 使用规范化 JSON 字符串保存，客户端生成代码无需依赖动态 JSON 类型。
#[spacetimedb::table(
    accessor = system_key,
    index(accessor = system_key_by_customer, btree(columns = [customer_id])),
    index(accessor = system_key_by_customer_name, btree(columns = [customer_id, key_name]))
)]
pub struct SystemKey {
    #[primary_key]
    #[auto_inc]
    pub key_id: u64,
    pub customer_id: String,
    pub key_name: String,
    pub value_json: String,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
