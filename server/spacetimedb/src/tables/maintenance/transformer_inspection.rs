//! 变压器巡检记录表定义。
//!
//! 一行是一次巡检事件，挂在 `transformer_asset` 上，**只增不删**：服务端不为
//! 它实现更新或删除 Reducer，填错了补一条更正记录。巡检人取自登录会话而不是
//! 手填文本，不可代填。完整设计见 `docs/变压器台账与扫码巡检.md`。
//!
//! 本表没有 `park_id` 列：园区归属经由资产表判定，删除园区时由在册资产阻挡。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = transformer_inspection,
    index(accessor = transformer_inspection_by_customer, btree(columns = [customer_id])),
    index(accessor = transformer_inspection_by_asset, btree(columns = [asset_id])),
    index(accessor = transformer_inspection_by_check_time, btree(columns = [check_time]))
)]
pub struct TransformerInspection {
    #[primary_key]
    #[auto_inc]
    pub inspection_id: u64,
    pub customer_id: String,
    /// 巡检对象（`transformer_asset.asset_id`），必填——巡检必须落在具体设备上。
    pub asset_id: u64,
    /// 运行状态：`正常` / `异常`。
    pub status: String,
    /// 异常描述；状态为异常时必填。
    pub abnormal_note: Option<String>,
    /// 巡检人（`SystemUser.id`），取自登录会话。
    pub inspector_user_id: u64,
    /// 巡检人姓名，写入时从账号快照——历史记录应当冻结当时的名字，
    /// 且维护域订阅里没有用户表可供前端反查。
    pub inspector_name: String,
    pub check_time: Timestamp,
    pub remark: Option<String>,
    pub created_at: Timestamp,
}
