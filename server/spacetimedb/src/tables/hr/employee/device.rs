//! 考勤设备绑定与异常审计表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `attendance_device_binding` 表。
#[spacetimedb::table(
    accessor = attendance_device_binding,
    index(accessor = attendance_device_binding_by_customer, btree(columns = [customer_id])),
    index(accessor = attendance_device_binding_by_user, btree(columns = [user_id])),
    index(accessor = attendance_device_binding_by_device, btree(columns = [device_id]))
)]
pub struct AttendanceDeviceBinding {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub user_id: u64,
    pub device_id: String,
    pub device_model: Option<String>,
    pub device_system: Option<String>,
    pub first_bind_time: Timestamp,
}

/// 对应 MySQL 业务库中的 `attendance_device_abnormal_log` 表。
#[spacetimedb::table(
    accessor = attendance_device_abnormal_log,
    index(accessor = attendance_device_abnormal_by_customer, btree(columns = [customer_id])),
    index(accessor = attendance_device_abnormal_by_user, btree(columns = [user_id])),
    index(accessor = attendance_device_abnormal_by_device, btree(columns = [current_device_id])),
    index(accessor = attendance_device_abnormal_by_created, btree(columns = [created_at]))
)]
pub struct AttendanceDeviceAbnormalLog {
    #[primary_key]
    #[auto_inc]
    pub id: u64,
    pub customer_id: String,
    pub user_id: u64,
    pub attendance_id: Option<u64>,
    /// `punch_in` 表示上班，`punch_out` 表示下班。
    pub action: String,
    /// 多个异常类型使用英文逗号分隔。
    pub abnormal_type: String,
    pub bound_device_id: Option<String>,
    pub current_device_id: String,
    pub duplicate_user_ids: Option<String>,
    pub duplicate_user_names: Option<String>,
    pub punch_time: Option<Timestamp>,
    pub created_at: Timestamp,
}
