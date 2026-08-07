//! 园区注销：资产随园区一起注销，带账的记录挡住删除。
//!
//! 原来 `delete_park` 只把园区那一行标记成已删，园区下面的厂房、楼层、宿舍、
//! 水电表全都还活着——生产库里就留下过一栋 `park_id` 指向已删园区的「A型厂房」
//! 和它的楼层。这些行在各自的列表页照旧出现，却点不进园区，面积也照旧被算进
//! 统计口径。
//!
//! # 为什么是一张清单
//!
//! 第一版是在 `delete_park` 里逐张表手写循环。问题不在写得对不对，而在**新加一张
//! 挂园区的表时没有任何东西提醒你回来补一行**——漏了就是下一个孤儿，而且要等到
//! 生产库里出现脏数据才会被发现。
//!
//! 现在每张带 `park_id` 的表都是 [`ParkChild`] 的一个变体，两道关卡接力把关：
//!
//! 1. **编译器**：新增变体后 [`ParkChild::spec`]、[`ParkChild::blocked_rows`] 和
//!    [`ParkChild::cascade`] 三个 `match` 同时报「未覆盖」，处置方式想漏也漏不掉。
//!    用枚举而不是表名字符串正是为了这个——字符串配 `match` 需要兜底分支，漏写
//!    只会在运行时才发现。
//! 2. **`所有挂园区的表都已登记` 测试**：扫 `src/tables` 把每张带 `park_id` 列的
//!    表和 [`PARK_CHILDREN`] 对账，接住「表建好了但根本没加变体」这一步。
//!
//! # 怎么选处置方式
//!
//! 依据是**这张表有没有 `is_deleted` 列**，外加一条覆盖规则：
//!
//! - **纯资产台账和授权关系**（厂房、宿舍、水电表、图片关联、园区授权）：有软删
//!   标记，随园区一起注销。它们没有脱离园区独立存在的意义。
//!   设备台账（摄像头、门禁）同属这一类：它没有巡检历史之类的子记录，也不带账，
//!   为了删园区先逐台删摄像头只是白费功夫。
//! - **带账的记录**（合同、账单、流水、投资、报销）：即使有软删标记也挡住删除。
//!   这些是钱和义务，不该被一次点击藏起来。
//! - **没有软删标记的记录**（消防、变压器、电梯、维修工单、卫生检查、厂房维护、
//!   门禁、请假）：只能挡住。「级联」对它们只能是硬删，那是丢数据。
//!
//! 园区本来就有「停用」状态，那才是"不再经营但保留历史"的表达。删除留给建错
//! 档的情况，所以拦下来的提示会把用户引到停用。

use spacetimedb::ReducerContext;

use crate::{
    reducers::{
        access::{require_park, require_park_access, require_rental_manager},
        rental::assets::{soft_delete_dormitory_tree, soft_delete_factory_tree},
        platform::media::image_reducer::delete_image_if_unreferenced,
    },
    tables::*,
};

/// 园区注销时一张子表的处置方式。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Disposition {
    /// 随园区一起注销。
    Cascade,
    /// 挡住删除。
    ///
    /// 字符串是给用户看的量词，**共用同一个量词的表会合并计数**——六张运维表
    /// 分开报数只会让提示变成一串噪音，用户要的是"还有 3 条运维记录"。
    Block(&'static str),
}

/// 每一张带 `park_id` 列的表。
///
/// 新增挂园区的表 = 在这里加一个变体，然后跟着编译错误把三个 `match` 补齐。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ParkChild {
    RentalTenant,
    AmountBill,
    Finance,
    Investment,
    Reimbursement,
    CarryoverBatch,
    Firefighting,
    FirefightingAsset,
    Transformer,
    TransformerAsset,
    Elevator,
    ElevatorAsset,
    RepairOrder,
    HygieneCheck,
    FactoryMaintenance,
    AccessCar,
    AccessVisitor,
    AccessDoor,
    LeaveApplication,
    Factory,
    DormitoryBuilding,
    UtilityMeter,
    DeviceAsset,
    EdgeGateway,
    ParkImage,
    UserPark,
    RolePark,
}

/// 园区注销时按这个顺序处理子表。
///
/// 顺序即拦截提示里各项的先后：先列钱和义务，再列运维、门禁和人事——用户最需要
/// 先处理的排在最前面。
pub(crate) const PARK_CHILDREN: &[ParkChild] = &[
    // 钱和义务
    ParkChild::RentalTenant,
    ParkChild::AmountBill,
    ParkChild::Finance,
    ParkChild::Investment,
    ParkChild::Reimbursement,
    ParkChild::CarryoverBatch,
    // 运维台账，合并成一项（transformer 是弃置的旧表，生产为空，保留登记）
    ParkChild::Firefighting,
    ParkChild::FirefightingAsset,
    ParkChild::Transformer,
    ParkChild::TransformerAsset,
    ParkChild::Elevator,
    ParkChild::ElevatorAsset,
    ParkChild::RepairOrder,
    ParkChild::HygieneCheck,
    ParkChild::FactoryMaintenance,
    // 门禁与人事
    ParkChild::AccessCar,
    ParkChild::AccessVisitor,
    ParkChild::AccessDoor,
    ParkChild::LeaveApplication,
    // 随园区一起注销
    ParkChild::Factory,
    ParkChild::DormitoryBuilding,
    ParkChild::UtilityMeter,
    ParkChild::DeviceAsset,
    ParkChild::EdgeGateway,
    ParkChild::ParkImage,
    ParkChild::UserPark,
    ParkChild::RolePark,
];

impl ParkChild {
    /// 表访问器名（与 `#[spacetimedb::table(accessor = ...)]` 同名）和处置方式。
    ///
    /// 两者合成一个 `match` 是有意的：分成两个函数写，改了处置方式忘了改表名不会
    /// 报错，而表名正是完备性测试对账用的键。
    const fn spec(self) -> (&'static str, Disposition) {
        match self {
            Self::RentalTenant => ("rental_tenant", Disposition::Block("份合同")),
            Self::AmountBill => ("amount_bill", Disposition::Block("张租金账单")),
            Self::Finance => ("finance", Disposition::Block("笔财务流水")),
            Self::Investment => ("investment", Disposition::Block("条投资记录")),
            Self::Reimbursement => ("reimbursement", Disposition::Block("笔报销")),
            Self::CarryoverBatch => ("carryover_batch", Disposition::Block("笔结转记录")),
            Self::Firefighting => ("firefighting", Disposition::Block("条运维记录")),
            Self::FirefightingAsset => ("firefighting_asset", Disposition::Block("条运维记录")),
            Self::Transformer => ("transformer", Disposition::Block("条运维记录")),
            Self::TransformerAsset => ("transformer_asset", Disposition::Block("条运维记录")),
            Self::Elevator => ("elevator", Disposition::Block("条运维记录")),
            Self::ElevatorAsset => ("elevator_asset", Disposition::Block("条运维记录")),
            Self::RepairOrder => ("repair_order", Disposition::Block("条运维记录")),
            Self::HygieneCheck => ("hygiene_check", Disposition::Block("条运维记录")),
            Self::FactoryMaintenance => ("factory_maintenance", Disposition::Block("条运维记录")),
            Self::AccessCar => ("access_car", Disposition::Block("条门禁记录")),
            Self::AccessVisitor => ("access_visitor", Disposition::Block("条门禁记录")),
            Self::AccessDoor => ("access_door", Disposition::Block("条门禁记录")),
            Self::LeaveApplication => ("leave_application", Disposition::Block("条请假记录")),
            Self::Factory => ("factory", Disposition::Cascade),
            Self::DormitoryBuilding => ("dormitory_building", Disposition::Cascade),
            Self::UtilityMeter => ("utility_meter", Disposition::Cascade),
            Self::DeviceAsset => ("device_asset", Disposition::Cascade),
            Self::EdgeGateway => ("edge_gateway", Disposition::Cascade),
            Self::ParkImage => ("park_image", Disposition::Cascade),
            Self::UserPark => ("user_park", Disposition::Cascade),
            Self::RolePark => ("role_park", Disposition::Cascade),
        }
    }

    const fn disposition(self) -> Disposition {
        self.spec().1
    }

    /// 园区下还剩多少行挡着删除。
    fn blocked_rows(self, ctx: &ReducerContext, park_id: u64) -> usize {
        match self {
            Self::RentalTenant => ctx
                .db
                .rental_tenant()
                .rental_tenant_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted)
                .count(),
            Self::AmountBill => ctx
                .db
                .amount_bill()
                .amount_bill_by_park()
                .filter(park_id)
                .count(),
            Self::Finance => ctx
                .db
                .finance()
                .finance_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted)
                .count(),
            Self::Investment => ctx
                .db
                .investment()
                .investment_by_park()
                .filter(park_id)
                .count(),
            Self::Reimbursement => ctx
                .db
                .reimbursement()
                .reimbursement_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted)
                .count(),
            Self::CarryoverBatch => ctx
                .db
                .carryover_batch()
                .carryover_batch_by_park()
                .filter(park_id)
                .count(),
            Self::Firefighting => ctx
                .db
                .firefighting()
                .firefighting_by_park()
                .filter(park_id)
                .count(),
            Self::FirefightingAsset => ctx
                .db
                .firefighting_asset()
                .firefighting_asset_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted)
                .count(),
            Self::Transformer => ctx
                .db
                .transformer()
                .transformer_by_park()
                .filter(park_id)
                .count(),
            Self::TransformerAsset => ctx
                .db
                .transformer_asset()
                .transformer_asset_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted)
                .count(),
            Self::Elevator => ctx.db.elevator().elevator_by_park().filter(park_id).count(),
            Self::ElevatorAsset => ctx
                .db
                .elevator_asset()
                .elevator_asset_by_park()
                .filter(park_id)
                .filter(|row| !row.is_deleted)
                .count(),
            Self::RepairOrder => ctx
                .db
                .repair_order()
                .repair_order_by_park()
                .filter(park_id)
                .count(),
            Self::HygieneCheck => ctx
                .db
                .hygiene_check()
                .hygiene_check_by_park()
                .filter(park_id)
                .count(),
            Self::FactoryMaintenance => ctx
                .db
                .factory_maintenance()
                .factory_maintenance_by_park()
                .filter(park_id)
                .count(),
            Self::AccessCar => ctx
                .db
                .access_car()
                .access_car_by_park()
                .filter(park_id)
                .count(),
            Self::AccessVisitor => ctx
                .db
                .access_visitor()
                .access_visitor_by_park()
                .filter(park_id)
                .count(),
            Self::AccessDoor => ctx
                .db
                .access_door()
                .access_door_by_park()
                .filter(park_id)
                .count(),
            Self::LeaveApplication => ctx
                .db
                .leave_application()
                .leave_application_by_park()
                .filter(park_id)
                .count(),
            // 随园区注销的表不参与拦截。这里逐个列出而不是写 `_`：写了兜底分支，
            // 以后新增一张该拦的表却忘了写计数，就会静默返回 0 放行。
            Self::Factory
            | Self::DormitoryBuilding
            | Self::UtilityMeter
            | Self::DeviceAsset
            | Self::EdgeGateway
            | Self::ParkImage
            | Self::UserPark
            | Self::RolePark => 0,
        }
    }

    /// 随园区一起注销。
    fn cascade(self, ctx: &ReducerContext, park_id: u64) {
        match self {
            Self::Factory => {
                // 厂房要带走自己的楼层和图片关联，复用它自己的注销函数而不是在
                // 这里再抄一遍。
                let rows = ctx
                    .db
                    .factory()
                    .factory_by_park()
                    .filter(park_id)
                    .filter(|row| !row.is_deleted)
                    .collect::<Vec<_>>();
                for row in rows {
                    soft_delete_factory_tree(ctx, row);
                }
            }
            Self::DormitoryBuilding => {
                let rows = ctx
                    .db
                    .dormitory_building()
                    .dormitory_building_by_park()
                    .filter(park_id)
                    .filter(|row| !row.is_deleted)
                    .collect::<Vec<_>>();
                for row in rows {
                    soft_delete_dormitory_tree(ctx, row);
                }
            }
            Self::UtilityMeter => {
                // 不必检查「是否被合同引用」：拦截阶段已确认园区下没有未注销合同。
                let rows = ctx
                    .db
                    .utility_meter()
                    .utility_meter_by_park()
                    .filter(park_id)
                    .filter(|row| !row.is_deleted)
                    .collect::<Vec<_>>();
                for mut row in rows {
                    row.is_deleted = true;
                    row.updated_at = Some(ctx.timestamp);
                    ctx.db.utility_meter().meter_id().update(row);
                }
            }
            Self::DeviceAsset => {
                // 只标软删，不动 `device_asset_image`：软删的语义是数据保留、只从
                // 台账里隐藏，把图片关联一起解掉就不是隐藏而是丢失了。口径与单台
                // 设备注销（`delete_device_asset`）一致。
                let rows = ctx
                    .db
                    .device_asset()
                    .device_asset_by_park()
                    .filter(park_id)
                    .filter(|row| !row.is_deleted)
                    .collect::<Vec<_>>();
                for mut row in rows {
                    row.is_deleted = true;
                    row.updated_at = Some(ctx.timestamp);
                    ctx.db.device_asset().asset_id().update(row);
                }
            }
            Self::EdgeGateway => {
                // 边缘设备随园区注销，并且**必须解绑身份**：园区都没了，那台机器还
                // 拿着有效身份继续往库里写，是「电脑是客户的」这个前提下最该堵
                // 住的口子。口径与 `delete_edge_gateway` 一致。
                let rows = ctx
                    .db
                    .edge_gateway()
                    .edge_gateway_by_park()
                    .filter(park_id)
                    .filter(|row| !row.is_deleted)
                    .collect::<Vec<_>>();
                for mut row in rows {
                    if let Some(binding) =
                        ctx.db.edge_gateway_identity().gateway_id().find(row.gateway_id)
                    {
                        ctx.db
                            .edge_gateway_identity()
                            .identity()
                            .delete(binding.identity);
                    }
                    row.is_deleted = true;
                    row.is_online = false;
                    row.status_changed_at = ctx.timestamp;
                    row.registration_code = None;
                    row.registration_expires_at = None;
                    row.updated_at = Some(ctx.timestamp);
                    ctx.db.edge_gateway().gateway_id().update(row);
                }
            }
            Self::ParkImage => {
                // 图片关联是纯连接行，没有软删标记，直接解绑；没有别处引用的图片
                // 元数据一并清掉，避免 R2 记录堆积。口径与 `update_park_with_images`
                // 一致。
                let links = ctx
                    .db
                    .park_image()
                    .park_image_by_park()
                    .filter(park_id)
                    .collect::<Vec<_>>();
                for link in &links {
                    ctx.db.park_image().id().delete(link.id);
                }
                for img_id in links.into_iter().map(|link| link.img_id) {
                    delete_image_if_unreferenced(ctx, img_id);
                }
            }
            Self::UserPark => {
                // 授权行不是业务数据：园区没了，指向它的授权自然作废。留着的话
                // `visible_parks` 每次都要多查一行，授权列表里还会显示一个点不开
                // 的园区。
                let rows = ctx
                    .db
                    .user_park()
                    .user_park_by_park()
                    .filter(park_id)
                    .filter(|row| !row.is_deleted)
                    .collect::<Vec<_>>();
                for mut row in rows {
                    row.is_deleted = true;
                    row.updated_at = Some(ctx.timestamp);
                    ctx.db.user_park().id().update(row);
                }
            }
            Self::RolePark => {
                let rows = ctx
                    .db
                    .role_park()
                    .role_park_by_park()
                    .filter(park_id)
                    .filter(|row| !row.is_deleted)
                    .collect::<Vec<_>>();
                for mut row in rows {
                    row.is_deleted = true;
                    row.updated_at = Some(ctx.timestamp);
                    ctx.db.role_park().id().update(row);
                }
            }
            // 挡住删除的表不级联——能走到这一步就说明它们已经是空的。
            Self::RentalTenant
            | Self::AmountBill
            | Self::Finance
            | Self::Investment
            | Self::Reimbursement
            | Self::CarryoverBatch
            | Self::Firefighting
            | Self::FirefightingAsset
            | Self::Transformer
            | Self::TransformerAsset
            | Self::Elevator
            | Self::ElevatorAsset
            | Self::RepairOrder
            | Self::HygieneCheck
            | Self::FactoryMaintenance
            | Self::AccessCar
            | Self::AccessVisitor
            | Self::AccessDoor
            | Self::LeaveApplication => {}
        }
    }
}

/// 注销园区。资产台账随园区一起注销，带账的记录会挡住删除。
///
/// 延续原系统 `isDeleted=true` 的语义：数据不删除，只从台账里隐藏。
#[spacetimedb::reducer]
pub fn delete_park(ctx: &ReducerContext, park_id: u64) -> Result<(), String> {
    require_rental_manager(ctx)?;
    require_park_access(ctx, park_id)?;
    let mut park = require_park(ctx, park_id)?;

    if let Some(message) = blocker_message(&collect_blockers(ctx, park_id)) {
        return Err(message);
    }

    for child in PARK_CHILDREN {
        if child.disposition() == Disposition::Cascade {
            child.cascade(ctx, park_id);
        }
    }

    park.is_deleted = true;
    park.updated_at = Some(ctx.timestamp);
    ctx.db.park().park_id().update(park);
    Ok(())
}

/// 园区下挡住删除的记录条数，按量词合并、保持 [`PARK_CHILDREN`] 的顺序。
fn collect_blockers(ctx: &ReducerContext, park_id: u64) -> Vec<(usize, &'static str)> {
    let mut items: Vec<(usize, &'static str)> = Vec::new();
    for child in PARK_CHILDREN {
        let Disposition::Block(label) = child.disposition() else {
            continue;
        };
        let count = child.blocked_rows(ctx, park_id);
        if count == 0 {
            continue;
        }
        match items.iter_mut().find(|(_, existing)| *existing == label) {
            Some(entry) => entry.0 += count,
            None => items.push((count, label)),
        }
    }
    items
}

/// 拦下删除时给操作者看的说明；没有任何阻挡时返回 `None`。
///
/// 只列出真正非零的项，并且把「停用」这个正确出口说清楚——否则用户只会反复点
/// 删除然后来问为什么删不掉。
fn blocker_message(items: &[(usize, &'static str)]) -> Option<String> {
    if items.is_empty() {
        return None;
    }
    let detail = items
        .iter()
        .map(|(count, label)| format!("{count} {label}"))
        .collect::<Vec<_>>();
    Some(format!(
        "园区下还有 {}，删除会让它们变成找不到园区的孤立数据。如果只是不再经营，请把园区状态改为「停用」；确实要删，请先处理这些记录。",
        detail.join("、")
    ))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::Path};

    use super::*;

    #[test]
    fn 没有任何记录时不阻挡删除() {
        assert_eq!(blocker_message(&[]), None);
    }

    #[test]
    fn 只列出真正存在的记录并指向停用() {
        let message = blocker_message(&[(2, "份合同"), (3, "条运维记录")])
            .expect("有合同和运维记录时必须拦下");
        assert!(message.contains("2 份合同"), "{message}");
        assert!(message.contains("3 条运维记录"), "{message}");
        // 没有的项不能出现，否则用户会去找一张不存在的账单。
        assert!(!message.contains("账单"), "{message}");
        assert!(!message.contains("流水"), "{message}");
        // 必须告诉用户正确的出口是停用，而不是只说一句删不掉。
        assert!(message.contains("停用"), "{message}");
    }

    #[test]
    fn 任意一类记录都足以拦下删除() {
        let mut labels = Vec::new();
        for child in PARK_CHILDREN {
            if let Disposition::Block(label) = child.disposition()
                && !labels.contains(&label)
            {
                labels.push(label);
            }
        }
        assert!(!labels.is_empty(), "清单里一条拦截规则都没有");
        for label in labels {
            let message = blocker_message(&[(1, label)]).expect("单独一类记录也必须拦下");
            assert!(message.contains(&format!("1 {label}")), "{message}");
        }
    }

    /// 园区自己的主键列也叫 `park_id`，它不是子表。
    const PARK_TABLE: &str = "park";

    /// 接住编译器接不住的那一步：表建好了，但根本没在 [`ParkChild`] 里加变体。
    ///
    /// 变体加了以后忘了写处置方式，三个 `match` 会当场报错；这条测试补的是更早
    /// 的一环——建了表就走人，园区注销时那张表的行会静默变成孤儿。
    #[test]
    fn 所有挂园区的表都已登记() {
        let declared = PARK_CHILDREN
            .iter()
            .map(|child| child.spec().0.to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            declared.len(),
            PARK_CHILDREN.len(),
            "PARK_CHILDREN 里有重复的表：{declared:?}"
        );

        let actual = tables_with_park_column();
        let missing = actual.difference(&declared).collect::<Vec<_>>();
        assert!(
            missing.is_empty(),
            "这些表带 park_id 但没在 ParkChild 里登记，园区注销时会留下孤儿数据：{missing:?}"
        );
        let stale = declared.difference(&actual).collect::<Vec<_>>();
        assert!(
            stale.is_empty(),
            "这些表已经不带 park_id 了，请从 ParkChild 移除：{stale:?}"
        );
    }

    /// 扫 `src/tables`，取出所有带 `park_id` 列的表访问器名。
    ///
    /// 直接读源码而不是靠反射：`spacetimedb` 的表元数据只在模块运行时可得，单元
    /// 测试里拿不到。表定义写法是固定的——表访问器 `accessor = x` 单独成行，索引
    /// 访问器写在 `index(accessor = x, ...)` 里，前缀不同，够分辨。
    fn tables_with_park_column() -> BTreeSet<String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/tables");
        let mut found = BTreeSet::new();
        for path in rust_files(&root) {
            let source = fs::read_to_string(&path).expect("表定义文件应当可读");
            let mut accessor: Option<String> = None;
            for line in source.lines() {
                let line = line.trim();
                if let Some(name) = line.strip_prefix("accessor = ") {
                    accessor = Some(name.trim_end_matches(',').to_string());
                } else if line.starts_with("pub park_id:") {
                    let name = accessor
                        .clone()
                        .unwrap_or_else(|| panic!("{path:?} 里的 park_id 找不到所属表"));
                    if name != PARK_TABLE {
                        found.insert(name);
                    }
                }
            }
        }
        assert!(!found.is_empty(), "没扫到任何表，检查 {root:?} 是否存在");
        found
    }

    fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir).expect("表定义目录应当可读") {
            let path = entry.expect("目录项应当可读").path();
            if path.is_dir() {
                files.extend(rust_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
        files
    }
}
