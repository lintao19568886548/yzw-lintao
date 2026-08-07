//! 过期登录会话的周期性清扫任务。
//!
//! View 只能拿到 `sender`，没有时间源，无法像 Reducer 那样过滤 `expires_at`。
//! 因此「`user_session` 行存在」必须等价于「会话仍然有效」。一次性到期任务
//! 负担了主要工作，这个周期任务只是兜底：任何一次性任务丢失或积压时，
//! 过期会话最多多存活一个清扫周期。

use spacetimedb::ScheduleAt;

#[spacetimedb::table(
    accessor = user_session_sweep,
    scheduled(crate::reducers::platform::center::auth::password::sweep_expired_sessions)
)]
pub struct UserSessionSweep {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
}
