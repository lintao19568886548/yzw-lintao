//! 消防设施巡检记录表定义。
//!
//! 与 `transformer_inspection` 完全同构：一行一次巡检事件，**只增不删**，
//! 巡检人取自登录会话快照。每个设施单独巡检——"这层楼三台灭火器巡了两台"
//! 从此有据可查。完整设计见 `docs/消防设施台账与扫码巡检.md`。
//!
//! 本表没有 `park_id` 列：园区归属经由资产表判定，删除园区时由在册资产阻挡。

use spacetimedb::Timestamp;

#[spacetimedb::table(
    accessor = firefighting_inspection,
    index(accessor = firefighting_inspection_by_customer, btree(columns = [customer_id])),
    index(accessor = firefighting_inspection_by_asset, btree(columns = [asset_id])),
    index(accessor = firefighting_inspection_by_check_time, btree(columns = [check_time]))
)]
pub struct FirefightingInspection {
    #[primary_key]
    #[auto_inc]
    pub inspection_id: u64,
    pub customer_id: String,
    /// 巡检对象（`firefighting_asset.asset_id`），必填。
    pub asset_id: u64,
    /// 运行状态：`正常` / `异常`。
    pub status: String,
    /// 异常描述；状态为异常时必填。
    pub abnormal_note: Option<String>,
    /// 巡检人（`SystemUser.id`），取自登录会话。
    pub inspector_user_id: u64,
    /// 巡检人姓名，写入时从账号快照。
    pub inspector_name: String,
    pub check_time: Timestamp,
    pub remark: Option<String>,
    pub created_at: Timestamp,
}
