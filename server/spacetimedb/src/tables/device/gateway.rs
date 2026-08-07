//! 边缘计算设备台账与身份绑定。
//!
//! 边缘设备是装在园区现场那台 Windows 电脑上的自研常驻程序，它**直接作为
//! SpacetimeDB 客户端**接入，不经过任何自建网关。因此：
//!
//! - **在线状态就是连接生死**，由 `client_connected` / `client_disconnected`
//!   两个生命周期钩子翻转，不需要心跳表，也不会有「每 30 秒写一次库、界面
//!   跟着抖一次」的问题；
//! - **下行指令与规则下发靠订阅**，后台插一行边缘设备就看得到，不用另设计协议。
//!
//! 授权依据始终是 `ctx.sender()`（ARCHITECTURE §1.7）。注册码只是一次性的
//! 绑定凭据，和用户首次登录换取身份绑定是同一个道理，不作为持续授权依据。
//! 完整设计见 `docs/边缘计算设备与摄像头接入.md`。

use spacetimedb::{Identity, Timestamp};

/// 健康等级：一切正常。
pub const HEALTH_NORMAL: &str = "normal";
/// 健康等级：能继续跑，但需要有人看一眼（磁盘偏紧、负载偏高）。
pub const HEALTH_WARNING: &str = "warning";
/// 健康等级：已经影响工作（磁盘满、摄像头大面积掉线）。
pub const HEALTH_CRITICAL: &str = "critical";
/// 健康等级：还没上报过。
pub const HEALTH_UNKNOWN: &str = "unknown";

#[spacetimedb::table(
    accessor = edge_gateway,
    index(accessor = edge_gateway_by_customer, btree(columns = [customer_id])),
    index(accessor = edge_gateway_by_park, btree(columns = [park_id]))
)]
pub struct EdgeGateway {
    #[primary_key]
    #[auto_inc]
    pub gateway_id: u64,
    pub customer_id: String,
    /// 所属园区，必填非 0。一台边缘设备只服务一个园区。
    pub park_id: u64,
    /// 设备名称，园区内唯一（如「北区值班室电脑」）——现场排查时靠它认机器。
    pub gateway_name: String,
    /// 一次性注册码，激活后清空。
    ///
    /// **明文存放是有意的**：它短、一次性、有有效期，作用只是让装机人员现场
    /// 手输一次。真正的长期凭据是绑定的 Identity，那个由 SpacetimeDB 持有，
    /// 我们这边永远看不到，也就无从泄露。
    pub registration_code: Option<String>,
    pub registration_expires_at: Option<Timestamp>,
    /// 边缘设备此刻在不在线。由连接生命周期翻转，不由心跳维护。
    pub is_online: bool,
    /// 上一次在线状态发生变化的时刻。「在线多久了 / 离线多久了」由它现算。
    pub status_changed_at: Timestamp,
    /// 边缘设备上报的程序版本，用于确认自升级有没有铺到位。
    pub agent_version: Option<String>,
    /// 机器健康等级。电脑是客户的，出了问题要能自证清白。
    pub health_level: String,
    /// 健康详情（如「磁盘剩余 2.1 GB」）。
    pub health_detail: Option<String>,
    /// 健康等级或详情上一次变化的时刻。
    ///
    /// 与在线状态同一个原则：**只在变化时写库**。边缘设备本地持续采样，只有跨过
    /// 阈值才上报一次；正常运行时这一行几乎不动，界面也就不会无故重渲染。
    pub health_changed_at: Option<Timestamp>,
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
}

/// 边缘设备的 SpacetimeDB 身份绑定，形状与 `user_identity` 一致。
///
/// `gateway_id` 唯一：一台边缘设备同时只认一个身份。换机器要先吊销，否则旧机器
/// 还能继续上报——这正是「电脑是客户的」场景下必须堵住的口子。
#[spacetimedb::table(accessor = edge_gateway_identity)]
pub struct EdgeGatewayIdentity {
    #[primary_key]
    pub identity: Identity,
    #[unique]
    pub gateway_id: u64,
    pub bound_at: Timestamp,
}
