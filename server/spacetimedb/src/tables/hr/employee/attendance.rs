//! 考勤打卡记录表定义。

use spacetimedb::Timestamp;

/// 对应 MySQL 业务库中的 `attendances` 表。
#[spacetimedb::table(
    accessor = attendance,
    index(accessor = attendance_by_customer, btree(columns = [customer_id])),
    index(accessor = attendance_by_status_time, btree(columns = [status, punch_in]))
)]
pub struct Attendance {
    #[primary_key]
    #[auto_inc]
    pub attendance_id: u64,
    pub customer_id: String,
    pub punch_in: Timestamp,
    pub punch_out: Option<Timestamp>,
    /// `0` 正常、`1` 迟到、`2` 早退、`3` 迟到且早退、`4` 缺勤、`5` 请假。
    pub status: Option<i8>,
    /// MySQL `decimal(18,15)` 乘以 `10^15` 后保存。
    pub longitude_e15: i64,
    /// MySQL `decimal(18,15)` 乘以 `10^15` 后保存。
    pub latitude_e15: i64,
    pub user_id: Option<u64>,
    pub username: String,
    /// 上班卡是怎么打的：`gps` 手机定位打卡、`face` 边缘设备人脸识别。
    ///
    /// 表尾追加列，按 §2.6.1 带 `#[default(...)]` 属于安全迁移。`#[default]` 要求
    /// 常量表达式，`String` 构造不出来，所以用 `Option<String>`。
    ///
    /// **`None` 表示这一行写在加这列之前**，而那时这张表只有手机打卡一个入口，
    /// 展示时按手机定位算——这是事实，不是猜测。新写入的行一律带明确取值。
    #[default(None::<String>)]
    pub punch_in_source: Option<String>,
    /// 下班卡是怎么打的；没打下班卡时为 `None`。
    ///
    /// **上下班分两列而不是一列**：一行记录里这是两个事件，来源可以不同——早上
    /// 从摄像头前走过打了上班卡，晚上忘了走那条路、用手机打的下班卡。只有一列
    /// 时这行的来源就说不清，而这恰恰是核工资、处理申诉时要查的东西。
    #[default(None::<String>)]
    pub punch_out_source: Option<String>,
}

/// 打卡来源：手机定位打卡。
pub const PUNCH_SOURCE_GPS: &str = "gps";
/// 打卡来源：边缘设备人脸识别。
pub const PUNCH_SOURCE_FACE: &str = "face";
