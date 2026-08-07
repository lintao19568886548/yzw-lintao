//! 设备台账表定义。
//!
//! 一行是一台联网设备（摄像头、门禁一体机、道闸、人行闸机）。水电表**不在这张
//! 表里**——它们留在 `utility_meter`，因为那张表还挂着倍率、分时标志和账单引用，
//! 搬过来等于动账。设备管理页在读的时候把两张表合成一份清单。
//! 完整设计见 `docs/设备管理.md` 第四章。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = device_asset,
    index(accessor = device_asset_by_customer, btree(columns = [customer_id])),
    index(accessor = device_asset_by_park, btree(columns = [park_id])),
    index(accessor = device_asset_by_factory, btree(columns = [factory_id]))
)]
pub struct DeviceAsset {
    #[primary_key]
    #[auto_inc]
    pub asset_id: u64,
    pub customer_id: String,
    /// 所属园区，必填非 0——设备必须属于某个园区。
    pub park_id: u64,
    /// 所在厂房；`0` 表示园区公共区域（大门、道路、围墙），沿用变压器的 0 哨兵约定。
    ///
    /// 门禁与摄像头**大量装在公共区域**，这里为 0 是常态而非例外。
    pub factory_id: u64,
    /// 传感器（`sensor`）还是执行终端（`actuator`）。
    ///
    /// **这一列永远由服务端从 `device_type` 推导，输入结构里没有对应字段。**
    /// 存下来是为了客户端不必再抄一份分类表；不接受客户端传入是为了让「摄像头被
    /// 标成执行终端」这种自相矛盾的行在结构上不可能出现。
    pub device_class: String,
    /// 设备类型：`camera` / `access_controller` / `barrier` / `turnstile`。
    pub device_type: String,
    /// 设备名称，必填（如「北大门 1 号枪机」）。
    pub device_name: String,
    /// 设备编号，园区内唯一——现场贴的标签认的就是它。
    pub device_code: String,
    /// 位置描述，必填（如「北大门东侧立杆」）。
    pub location: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    /// 厂商平台上的设备号，可空。语义与水电表的同名字段一致：绑上了才谈得上
    /// 在线状态与实时数据，没绑只是一条静态台账。
    pub external_device_id: Option<String>,
    /// 启用日期 `YYYY-MM-DD`，可选。
    pub commissioned_on: Option<String>,
    pub remark: Option<String>,
    pub is_deleted: bool,
    pub created_at: Timestamp,
    pub updated_at: Option<Timestamp>,
    /// 挂在哪台边缘计算设备上；`0` 表示还没接入。
    ///
    /// 表尾追加列，按 §2.6.1 带 `#[default(...)]` 属于安全迁移。摄像头改为
    /// 边缘接入之后，`external_device_id` 那个「厂商平台设备号」表达不了
    /// 「边缘设备 + 通道」这个二元组；该字段保留原义不动，水电表那条链路还在用。
    #[default(0u64)]
    pub gateway_id: u64,
    /// 在边缘设备上的第几路；`0` 表示未指定。
    #[default(0u32)]
    pub channel_no: u32,
}
