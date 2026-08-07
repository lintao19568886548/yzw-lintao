//! 租金收缴的对账确认：状态清单 + 解释器。
//!
//! 实现 `docs/租金收缴与对账确认流程.md` 的前两步——确认动作与状态判定。
//! 账期截止的自动顺延是第三步，按文档 §5.3 必须等确认动作跑过一个完整账期
//! 再做，这里刻意不含。
//!
//! # 清单加解释器
//!
//! 状态机不写成散落在各处的 `if`，而是两张纯数据清单加两个极小的解释器：
//!
//! - [`STATE_RULES`]：状态判定清单，自上而下第一条命中即当前状态，**顺序即
//!   优先级**——收齐确认压过一切（闭环是终态），实收达标压过差额确认（催缴
//!   期间补齐了就该走确认收齐，而不是继续停在催缴）；
//! - [`ACTION_RULES`]：动作清单，哪个动作在哪些状态下允许。
//!
//! 加一种状态或动作 = 在清单里加一行，解释器不动。拒绝理由集中在
//! [`reject_reason`] 一个穷举 `match` 里，漏写新状态的文案会被编译器拦下。
//!
//! 状态本身**不落库**：它完全由「账单金额 + 确认记录」推导（[`collection_state`]
//! 是纯函数），存下来反而要在每次改实收时同步维护一份会漂移的副本。
//!
//! 前端 `src/pages/billing/model.rs` 有一份同构的镜像用于展示与按钮判定，
//! 两边的清单必须保持一致；服务端是权威，前端骗得过按钮骗不过 Reducer。

use spacetimedb::{ReducerContext, Table};

use crate::{
    reducers::{
        access::{require_amount_bill, require_park_access, require_rental_manager},
        validation::limited_optional_text,
    },
    tables::*,
};

/// 一张账单在收缴对账里的当前状态。
///
/// 与文档第四章的状态表一一对应；「待催交/已催交」合并为 [`AwaitingReceipt`]，
/// 因为催交留痕还没实现（文档 §5.2），两者暂无法区分。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CollectionState {
    /// 待收款：还没有任何收款动作。
    AwaitingReceipt,
    /// 待确认收齐：实收已达应收，只差经理点一下。**这是「钱到了却没确认」
    /// 的提醒态**（文档 §4.1），停留在这里的账单要在界面上单独标出。
    AwaitingCollectedConfirm,
    /// 待确认差额：实收不足，经理还没有知悉。
    AwaitingShortfallConfirm,
    /// 差额催交中：经理已确认差额，二次催交进行中。
    ShortfallChasing,
    /// 已闭环：经理已确认收齐，终态。
    Closed,
}

/// 状态判定所需的全部事实，从账单行与确认记录提取。
#[derive(Clone, Copy, Debug)]
pub(crate) struct BillFacts {
    pub receivable_cents: i64,
    pub received_cents: i64,
    /// 财务是否录入过收款时间。实收为 0 但录了时间也算有过收款动作。
    pub has_receipt_time: bool,
    pub has_collected_confirm: bool,
    pub has_shortfall_confirm: bool,
}

impl BillFacts {
    /// 有没有发生过任何收款动作。
    ///
    /// 不能只看 `received > 0`：应收为 0 的历史账单一建出来就满足
    /// 「实收 ≥ 应收」，会立刻变成待确认而制造噪音。没有收款动作的账单
    /// 一律停在待收款。
    fn has_movement(&self) -> bool {
        self.received_cents > 0 || self.has_receipt_time
    }
}

/// 状态判定清单：自上而下，第一条命中即当前状态。
///
/// 末条谓词恒真，解释器因此是全函数；有测试守着这个不变量。
const STATE_RULES: &[(CollectionState, fn(&BillFacts) -> bool)] = &[
    // 收齐确认是终态，压过一切——闭环之后金额怎么变都不再改变状态，
    // 这正是确认要留快照的原因。
    (CollectionState::Closed, |f| f.has_collected_confirm),
    // 实收达标压过差额确认：催缴期间补齐了，该走确认收齐。
    (CollectionState::AwaitingCollectedConfirm, |f| {
        f.has_movement() && f.received_cents >= f.receivable_cents
    }),
    (CollectionState::ShortfallChasing, |f| f.has_shortfall_confirm),
    (CollectionState::AwaitingShortfallConfirm, |f| f.has_movement()),
    (CollectionState::AwaitingReceipt, |_| true),
];

/// 经理的确认动作。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ConfirmAction {
    /// 确认已收齐——唯一能闭环的动作。
    Collected,
    /// 确认差额——只表示知悉，不闭环。
    Shortfall,
}

/// 动作清单：哪个动作在哪些状态下允许。
const ACTION_RULES: &[(ConfirmAction, &[CollectionState])] = &[
    (
        ConfirmAction::Collected,
        &[CollectionState::AwaitingCollectedConfirm],
    ),
    (
        ConfirmAction::Shortfall,
        &[
            CollectionState::AwaitingShortfallConfirm,
            // 催缴中允许再次确认：租户补了一部分但仍不足时，新的差额值得
            // 再留一次快照。金额没变化的重复确认由 check_shortfall_repeat 拦。
            CollectionState::ShortfallChasing,
        ],
    ),
];

/// 解释器一：读状态清单，返回当前状态。
#[pure_function::pure]
pub(crate) fn collection_state(facts: &BillFacts) -> CollectionState {
    STATE_RULES
        .iter()
        .find(|(_, applies)| applies(facts))
        .map(|(state, _)| *state)
        // 末条恒真，到不了这里；不写 unwrap 是为了不给未来改清单的人埋雷。
        .unwrap_or(CollectionState::AwaitingReceipt)
}

/// 解释器二：读动作清单，判定动作是否被当前状态允许。
#[pure_function::pure]
pub(crate) fn check_action(action: ConfirmAction, facts: &BillFacts) -> Result<(), String> {
    let state = collection_state(facts);
    let allowed = ACTION_RULES
        .iter()
        .find(|(candidate, _)| *candidate == action)
        .is_some_and(|(_, states)| states.contains(&state));
    if allowed {
        Ok(())
    } else {
        Err(reject_reason(action, state, facts))
    }
}

/// 拒绝理由。穷举 `match`：新增状态或动作时漏写文案，编译器当场报错。
fn reject_reason(action: ConfirmAction, state: CollectionState, facts: &BillFacts) -> String {
    let shortfall = facts.receivable_cents.saturating_sub(facts.received_cents);
    match (action, state) {
        (_, CollectionState::Closed) => "这张账单已经确认收齐，流程已闭环".into(),
        (ConfirmAction::Collected, CollectionState::AwaitingReceipt)
        | (ConfirmAction::Shortfall, CollectionState::AwaitingReceipt) => {
            "还没有收款记录，请先由财务录入实收".into()
        }
        (
            ConfirmAction::Collected,
            CollectionState::AwaitingShortfallConfirm | CollectionState::ShortfallChasing,
        ) => format!("实收还差 {}，不能确认收齐", format_cents(shortfall)),
        (ConfirmAction::Shortfall, CollectionState::AwaitingCollectedConfirm) => {
            "实收已达应收，应当确认收齐而不是确认差额".into()
        }
        // 动作在自己的允许状态里不会走到这里；穷举是给编译器看的。
        (ConfirmAction::Collected, CollectionState::AwaitingCollectedConfirm)
        | (
            ConfirmAction::Shortfall,
            CollectionState::AwaitingShortfallConfirm | CollectionState::ShortfallChasing,
        ) => "动作被允许".into(),
    }
}

/// 金额没有变化的重复差额确认没有信息量，拦下——否则经理连点两下就是两条
/// 一模一样的记录，翻确认历史的人要逐条对金额才能发现什么都没发生。
#[pure_function::pure]
pub(crate) fn check_shortfall_repeat(
    latest_snapshot: Option<(i64, i64)>,
    current: (i64, i64),
) -> Result<(), String> {
    if latest_snapshot == Some(current) {
        return Err("这笔差额已经确认过，金额没有变化".into());
    }
    Ok(())
}

/// 分 → 「1,234.56 元」。给拒绝理由用，口径与前端 format_money 一致（无货币符号）。
fn format_cents(cents: i64) -> String {
    let absolute = cents.unsigned_abs();
    format!("{}.{:02} 元", absolute / 100, absolute % 100)
}

#[spacetimedb::reducer]
pub fn confirm_bill_collected(
    ctx: &ReducerContext,
    bill_id: u64,
    remark: Option<String>,
) -> Result<(), String> {
    insert_confirmation(ctx, bill_id, ConfirmAction::Collected, remark)
}

#[spacetimedb::reducer]
pub fn confirm_bill_shortfall(
    ctx: &ReducerContext,
    bill_id: u64,
    remark: Option<String>,
) -> Result<(), String> {
    insert_confirmation(ctx, bill_id, ConfirmAction::Shortfall, remark)
}

/// 两个确认动作的共同事务体。
///
/// 权限是**园区管理**而不是账单管理：确认是园区经理的动作（文档第一章），
/// 财务录实收、经理认结果，两个岗位各有各的边界。
fn insert_confirmation(
    ctx: &ReducerContext,
    bill_id: u64,
    action: ConfirmAction,
    remark: Option<String>,
) -> Result<(), String> {
    let manager_user_id = require_rental_manager(ctx)?;
    let bill = require_amount_bill(ctx, bill_id)?;
    require_park_access(ctx, bill.park_id)?;

    let confirmations = ctx
        .db
        .bill_collection_confirmation()
        .confirmation_by_bill()
        .filter(bill_id)
        .collect::<Vec<_>>();
    let facts = BillFacts {
        receivable_cents: bill.total_fee_cents,
        received_cents: bill.receipt_amount_cents,
        has_receipt_time: bill.receipt_time.is_some(),
        has_collected_confirm: confirmations
            .iter()
            .any(|row| row.confirm_type == CONFIRM_TYPE_COLLECTED),
        has_shortfall_confirm: confirmations
            .iter()
            .any(|row| row.confirm_type == CONFIRM_TYPE_SHORTFALL),
    };
    check_action(action, &facts)?;
    if action == ConfirmAction::Shortfall {
        let latest = confirmations
            .iter()
            .filter(|row| row.confirm_type == CONFIRM_TYPE_SHORTFALL)
            .max_by_key(|row| row.confirmation_id)
            .map(|row| (row.receivable_cents, row.received_cents));
        check_shortfall_repeat(latest, (bill.total_fee_cents, bill.receipt_amount_cents))?;
    }

    let remark = limited_optional_text(remark, 200, "备注不能超过200个字符")?;
    // 姓名快照：优先真实姓名，缺失回退登录名——人事变动后记录仍然可读。
    let confirmed_by_name = ctx
        .db
        .system_user()
        .id()
        .find(manager_user_id)
        .map(|user| {
            if user.real_name.trim().is_empty() {
                user.username
            } else {
                user.real_name
            }
        })
        .unwrap_or_else(|| format!("用户 #{manager_user_id}"));

    ctx.db
        .bill_collection_confirmation()
        .insert(BillCollectionConfirmation {
            confirmation_id: 0,
            customer_id: bill.customer_id,
            bill_id,
            confirm_type: match action {
                ConfirmAction::Collected => CONFIRM_TYPE_COLLECTED.into(),
                ConfirmAction::Shortfall => CONFIRM_TYPE_SHORTFALL.into(),
            },
            receivable_cents: bill.total_fee_cents,
            received_cents: bill.receipt_amount_cents,
            confirmed_by: manager_user_id,
            confirmed_by_name,
            remark,
            confirmed_at: ctx.timestamp,
        });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(
        receivable: i64,
        received: i64,
        has_receipt_time: bool,
        collected: bool,
        shortfall: bool,
    ) -> BillFacts {
        BillFacts {
            receivable_cents: receivable,
            received_cents: received,
            has_receipt_time,
            has_collected_confirm: collected,
            has_shortfall_confirm: shortfall,
        }
    }

    #[test]
    fn 状态清单末条恒真解释器是全函数() {
        let (state, applies) = STATE_RULES.last().expect("清单不能为空");
        assert_eq!(*state, CollectionState::AwaitingReceipt);
        // 恒真谓词对任何事实都成立——随便挑几组极端值验证。
        for f in [
            facts(0, 0, false, false, false),
            facts(i64::MAX, i64::MIN, true, true, true),
        ] {
            assert!(applies(&f), "末条谓词必须恒真，否则解释器有判不出状态的输入");
        }
    }

    #[test]
    fn 没有收款动作时停在待收款() {
        // 应收为 0 的账单一建出来就满足「实收 ≥ 应收」，但没有任何收款动作，
        // 不能立刻变成待确认——那会给历史零额账单批量刷上提醒。
        assert_eq!(
            collection_state(&facts(0, 0, false, false, false)),
            CollectionState::AwaitingReceipt
        );
        assert_eq!(
            collection_state(&facts(100_00, 0, false, false, false)),
            CollectionState::AwaitingReceipt
        );
    }

    #[test]
    fn 实收达标即待确认收齐() {
        assert_eq!(
            collection_state(&facts(100_00, 100_00, false, false, false)),
            CollectionState::AwaitingCollectedConfirm
        );
        // 多缴同样按待确认收齐处理（文档 §2.4：多缴归入平账）。
        assert_eq!(
            collection_state(&facts(100_00, 120_00, false, false, false)),
            CollectionState::AwaitingCollectedConfirm
        );
        // 实收为 0 但财务录了收款时间，也算发生过收款动作——零额账单靠它闭环。
        assert_eq!(
            collection_state(&facts(0, 0, true, false, false)),
            CollectionState::AwaitingCollectedConfirm
        );
    }

    #[test]
    fn 部分收款先待确认差额后催交中() {
        assert_eq!(
            collection_state(&facts(100_00, 40_00, false, false, false)),
            CollectionState::AwaitingShortfallConfirm
        );
        assert_eq!(
            collection_state(&facts(100_00, 40_00, false, false, true)),
            CollectionState::ShortfallChasing
        );
    }

    #[test]
    fn 催缴期间补齐了优先走确认收齐() {
        // 有差额确认在册，但实收已经追平——状态清单的顺序保证「实收达标」
        // 压过「催缴中」，经理该做的是确认收齐而不是继续催。
        assert_eq!(
            collection_state(&facts(100_00, 100_00, false, false, true)),
            CollectionState::AwaitingCollectedConfirm
        );
    }

    #[test]
    fn 收齐确认是终态压过一切() {
        // 闭环之后即使实收被改小（比如财务更正），状态也不回退——确认留了
        // 快照，争议按快照说话。
        assert_eq!(
            collection_state(&facts(100_00, 10_00, true, true, true)),
            CollectionState::Closed
        );
    }

    #[test]
    fn 确认收齐只在待确认收齐状态允许() {
        assert!(check_action(ConfirmAction::Collected, &facts(100_00, 100_00, false, false, false)).is_ok());

        let err = check_action(ConfirmAction::Collected, &facts(100_00, 40_00, false, false, false))
            .expect_err("差额状态不能确认收齐");
        assert!(err.contains("60.00 元"), "拒绝理由要报出差多少钱：{err}");

        let err = check_action(ConfirmAction::Collected, &facts(100_00, 0, false, false, false))
            .expect_err("没有收款动作不能确认收齐");
        assert!(err.contains("财务"), "{err}");

        let err = check_action(ConfirmAction::Collected, &facts(100_00, 100_00, false, true, false))
            .expect_err("已闭环不能再确认");
        assert!(err.contains("闭环"), "{err}");
    }

    #[test]
    fn 确认差额在差额与催缴两个状态允许() {
        assert!(check_action(ConfirmAction::Shortfall, &facts(100_00, 40_00, false, false, false)).is_ok());
        assert!(check_action(ConfirmAction::Shortfall, &facts(100_00, 40_00, false, false, true)).is_ok());

        let err = check_action(ConfirmAction::Shortfall, &facts(100_00, 100_00, false, false, false))
            .expect_err("实收达标不能确认差额");
        assert!(err.contains("确认收齐"), "{err}");
    }

    #[test]
    fn 金额没变化的重复差额确认被拦() {
        assert!(check_shortfall_repeat(None, (100_00, 40_00)).is_ok());
        assert!(check_shortfall_repeat(Some((100_00, 30_00)), (100_00, 40_00)).is_ok());
        assert!(check_shortfall_repeat(Some((100_00, 40_00)), (100_00, 40_00)).is_err());
    }

    #[test]
    fn 每个动作都登记在动作清单里() {
        for action in [ConfirmAction::Collected, ConfirmAction::Shortfall] {
            let allowed = ACTION_RULES
                .iter()
                .find(|(candidate, _)| *candidate == action)
                .map(|(_, states)| *states)
                .unwrap_or_else(|| panic!("{action:?} 不在动作清单里"));
            assert!(!allowed.is_empty(), "{action:?} 没有任何允许状态");
            // 终态不该允许任何动作——闭环就是闭环。
            assert!(
                !allowed.contains(&CollectionState::Closed),
                "{action:?} 不该在已闭环状态下允许"
            );
        }
    }
}
