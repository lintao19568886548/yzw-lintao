//! 登录会话到期调度任务。

use spacetimedb::{Identity, ScheduleAt, Timestamp};

/// 到期时删除匹配的会话；旧任务不会误删后来续期的会话。
#[spacetimedb::table(
    accessor = user_session_expiry,
    scheduled(crate::reducers::platform::center::auth::password::expire_user_session)
)]
pub struct UserSessionExpiry {
    #[primary_key]
    #[auto_inc]
    pub scheduled_id: u64,
    pub scheduled_at: ScheduleAt,
    pub identity: Identity,
    pub expected_expires_at: Timestamp,
}
