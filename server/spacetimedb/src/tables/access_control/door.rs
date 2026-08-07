//! 门禁设备表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `access_door` 表。
#[spacetimedb::table(
    accessor = access_door,
    index(accessor = access_door_by_customer, btree(columns = [customer_id])),
    index(accessor = access_door_by_park, btree(columns = [park_id])),
    index(accessor = access_door_by_customer_code, btree(columns = [customer_id, device_code]))
)]
pub struct AccessDoor {
    #[primary_key]
    #[auto_inc]
    pub device_id: u64,
    pub customer_id: String,
    pub device_code: String,
    pub device_name: String,
    pub location: String,
    /// `1` 表示开启，`0` 表示关闭。
    pub status: i8,
    /// 园区外键。`0` 表示不绑定园区，见 `reducers::shared::park_ref`。
    pub park_id: u64,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
