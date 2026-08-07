//! 允许打卡的地点及半径。
//!
//! 在这张表出现之前，打卡**完全不限地点**：`validate_coordinate` 只检查经纬度
//! 是不是合法数字（防定位失败），员工在家、在外地照样打卡成功，系统只是把坐标
//! 记进 `Attendance`。老板没有任何地方能设置"只准在公司打卡"。
//!
//! # 为什么不挂在园区上
//!
//! 园区表只有文字地址，没有坐标；而且打卡点和园区不是一回事——办公室可能不在
//! 任何园区里，一个园区也可能有好几个门。单独一张表还能给每个点配自己的半径。
//!
//! # 一个点都没设时不限制
//!
//! 判定规则是「有启用的打卡点就必须落在其中之一的范围内，一个都没有就不限」。
//! 这样不设置就是原来的行为，设了才开始管——否则这张表一上线，所有人立刻打不
//! 了卡。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = attendance_location,
    index(accessor = attendance_location_by_customer, btree(columns = [customer_id]))
)]
pub struct AttendanceLocation {
    #[primary_key]
    #[auto_inc]
    pub location_id: u64,
    pub customer_id: String,
    /// 给人看的名字，例如「总部办公室」「周茂森园区门卫」。
    pub location_name: String,
    /// 参考地址，只用于展示和核对，判定不看它。
    pub address: Option<String>,
    /// 经度，`decimal(18,15)` 乘以 `10^15`，与 `Attendance` 同精度。
    pub longitude_e15: i64,
    /// 纬度，同上。
    pub latitude_e15: i64,
    /// 允许打卡的半径，米。
    ///
    /// 城区 GPS 误差常有 20～100 米，半径给太小会把正常上班的人挡在门外，
    /// 所以下限定在 50 米。
    pub radius_metres: i32,
    /// 停用后不参与判定。
    ///
    /// 与软删分开：临时封一个门用停用，历史考勤记录仍然对得上这个点的名字。
    pub is_enabled: bool,
    pub is_deleted: bool,
    pub remark: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}
