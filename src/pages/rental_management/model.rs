//! 租赁经营指标的纯计算模型。

use std::collections::{BTreeMap, BTreeSet};

use crate::spacetime_bindings::{
    dormitory_floor_type::DormitoryFloor, dormitory_type::Dormitory,
    factory_floor_type::FactoryFloor, factory_type::Factory, park_type::Park,
    rental_tenant_floor_type::RentalTenantFloor, rental_tenant_type::RentalTenant,
    tenant_profile_type::TenantProfile,
};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ParkSnapshot {
    pub park: Park,
    pub active_contracts: Vec<RentalTenant>,
    /// 园区厂房总面积。以前借住在 `Park.area_centi_square_metres` 上，那个
    /// 字段已经删掉——它是人工填的，而这里从来都是按楼层台账重算后覆盖它。
    pub total_area: i64,
    pub rented_area: i64,
    pub vacant_area: i64,
    pub occupancy_basis_points: i64,
}

impl ParkSnapshot {
    #[pure_function::pure]
    pub(super) fn from_rows(park: Park, tenants: &[RentalTenant], total_area: i64) -> Self {
        let active_contracts = tenants
            .iter()
            .filter(|tenant| tenant.park_id == park.park_id && is_active_income_contract(tenant))
            .cloned()
            .collect::<Vec<_>>();
        let rented_area = active_contracts
            .iter()
            .filter_map(|tenant| tenant.area_centi_square_metres)
            .filter(|area| *area > 0)
            .sum::<i64>();
        let total_area = total_area.max(0);
        let vacant_area = total_area.saturating_sub(rented_area).max(0);
        let occupancy_basis_points = if total_area == 0 {
            0
        } else {
            rented_area
                .saturating_mul(10_000)
                .saturating_div(total_area)
                .clamp(0, 10_000)
        };
        Self {
            park,
            active_contracts,
            total_area,
            rented_area,
            vacant_area,
            occupancy_basis_points,
        }
    }

    #[pure_function::pure]
    pub(super) fn occupancy_percent(&self) -> f64 {
        self.occupancy_basis_points as f64 / 100.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RentalSummary {
    pub total_area: i64,
    pub rented_area: i64,
    pub vacant_area: i64,
    pub active_contracts: usize,
    pub occupancy_basis_points: i64,
}

impl RentalSummary {
    #[pure_function::pure]
    pub(super) fn from_snapshots(rows: &[ParkSnapshot]) -> Self {
        let total_area = rows.iter().map(|row| row.total_area.max(0)).sum::<i64>();
        let rented_area = rows.iter().map(|row| row.rented_area).sum::<i64>();
        let vacant_area = total_area.saturating_sub(rented_area).max(0);
        let active_contracts = rows.iter().map(|row| row.active_contracts.len()).sum();
        let occupancy_basis_points = if total_area == 0 {
            0
        } else {
            rented_area
                .saturating_mul(10_000)
                .saturating_div(total_area)
                .clamp(0, 10_000)
        };
        Self {
            total_area,
            rented_area,
            vacant_area,
            active_contracts,
            occupancy_basis_points,
        }
    }
}

impl ParkSnapshot {
    /// 按楼层台账和合同占用算出园区的经营快照。
    ///
    /// 总面积来自楼层逐层求和，已租面积来自合同↔楼层关联——两者都不再读
    /// 人工维护的字段。合同行只用来数「生效中合同数」。
    #[pure_function::pure]
    pub fn from_asset_rows(
        park: crate::spacetime_bindings::park_type::Park,
        tenants: &[crate::spacetime_bindings::rental_tenant_type::RentalTenant],
        factories: &[crate::spacetime_bindings::factory_type::Factory],
        floors: &[crate::spacetime_bindings::factory_floor_type::FactoryFloor],
        tenant_floors: &[RentalTenantFloor],
    ) -> Self {
        let factory_ids = factories
            .iter()
            .filter(|factory| factory.park_id == park.park_id && !factory.is_deleted)
            .map(|factory| factory.factory_id)
            .collect::<std::collections::HashSet<_>>();
        let used_by_floor = floor_used_areas(tenant_floors, tenants);
        let (total_area, rented_area) = floors
            .iter()
            .filter(|floor| !floor.is_deleted && factory_ids.contains(&floor.factory_id))
            .fold((0_i64, 0_i64), |(total, used), floor| {
                (
                    total.saturating_add(floor.total_area_centi_square_metres),
                    used.saturating_add(
                        used_by_floor.get(&floor.floor_id).copied().unwrap_or(0),
                    ),
                )
            });
        let vacant_area = total_area.saturating_sub(rented_area).max(0);
        let occupancy_basis_points = if total_area > 0 {
            ((i128::from(rented_area) * 10_000) / i128::from(total_area)).clamp(0, 10_000) as i64
        } else {
            0
        };

        let mut snapshot = Self::from_rows(park, tenants, total_area);
        snapshot.rented_area = rented_area;
        snapshot.vacant_area = vacant_area;
        snapshot.occupancy_basis_points = occupancy_basis_points;
        snapshot
    }
}

/// 园区面积的两个口径。厂房和宿舍分开计量，不合并成一个数字——两者的
/// 出租方式、计价单位和空置含义都不一样，加在一起没有业务意义。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParkAreas {
    /// 厂房各层面积之和（百分之一平方米）。
    pub factory: i64,
    /// 宿舍各层「房间数 × 单间面积」之和。
    pub dormitory: i64,
}

/// 按楼层台账算出园区面积，不再人工填写。
///
/// 厂房面积是**逐层面积求和**，而不是「第一层面积 × 层数」。生产数据里
/// 同一栋楼的楼层并不等大（见过 1 层 5600、2 层 5350、其余 5600 ㎡ 的），
/// 用首层乘层数会多算 250 ㎡；楼层等大时两种算法结果相同，不等大时只有
/// 求和是对的。
///
/// 宿舍面积按「房间数 × 单间面积」算，单间面积没维护的层记 0——宁可少算
/// 也不要凭层数猜出一个看起来精确的数字。
#[pure_function::pure]
pub(crate) fn park_areas(
    park_id: u64,
    factories: &[Factory],
    floors: &[FactoryFloor],
    dormitories: &[Dormitory],
    dormitory_floors: &[DormitoryFloor],
) -> ParkAreas {
    let factory_ids = factories
        .iter()
        .filter(|factory| factory.park_id == park_id && !factory.is_deleted)
        .map(|factory| factory.factory_id)
        .collect::<BTreeSet<_>>();
    let factory_area = floors
        .iter()
        .filter(|floor| !floor.is_deleted && factory_ids.contains(&floor.factory_id))
        .fold(0_i64, |sum, floor| {
            sum.saturating_add(floor.total_area_centi_square_metres.max(0))
        });

    let dormitory_ids = dormitories
        .iter()
        .filter(|dormitory| dormitory.park_id == park_id && !dormitory.is_deleted)
        .map(|dormitory| dormitory.dormitory_id)
        .collect::<BTreeSet<_>>();
    let dormitory_area = dormitory_floors
        .iter()
        .filter(|floor| !floor.is_deleted && dormitory_ids.contains(&floor.dormitory_id))
        .fold(0_i64, |sum, floor| {
            let rooms = i64::from(floor.room_count.max(0));
            let each = floor.room_area_centi_square_metres.unwrap_or(0).max(0);
            sum.saturating_add(rooms.saturating_mul(each))
        });

    ParkAreas {
        factory: factory_area,
        dormitory: dormitory_area,
    }
}

/// 各厂房楼层被有效合同占用的面积（百分之一平方米）。
///
/// 这是原来那个人工填写的 `used_area_centi_square_metres` 的替代品：楼层
/// 总面积固定在楼层上，占用面积由合同关联算出，两者相减即为可租。跟宿舍
/// 「已用房间数」是同一个模式。
///
/// 只排除已注销的合同，判定口径与服务端 `replace_rental_tenant_floors` 的
/// 超额校验保持一致——两边一旦不同，界面显示的可租面积会和服务端愿意放行
/// 的面积对不上。
#[pure_function::pure]
pub(crate) fn floor_used_areas(
    links: &[RentalTenantFloor],
    tenants: &[RentalTenant],
) -> BTreeMap<u64, i64> {
    let active = tenants
        .iter()
        .filter(|tenant| !tenant.is_deleted)
        .map(|tenant| tenant.rental_tenant_id)
        .collect::<BTreeSet<_>>();
    let mut used = BTreeMap::new();
    for link in links
        .iter()
        .filter(|link| active.contains(&link.rental_tenant_id))
    {
        let entry = used.entry(link.floor_id).or_insert(0_i64);
        *entry = entry.saturating_add(link.area_centi_square_metres.max(0));
    }
    used
}

/// 楼层的满租/待招商计数。
///
/// 满租：合同占用面积覆盖了全部楼层面积，且楼层面积已经维护（不是 0）。
/// 其余情况（有空置面积，或者面积压根没填）一律算待招商——跟“待租厂房”
/// 页面提示的口径一致：“总面积已经全部出租，或尚未维护楼层面积”。
#[pure_function::pure]
pub(crate) fn floor_occupancy_counts(
    floors: &[crate::spacetime_bindings::factory_floor_type::FactoryFloor],
    used_by_floor: &BTreeMap<u64, i64>,
) -> (usize, usize) {
    let mut full = 0;
    let mut vacant = 0;
    for floor in floors.iter().filter(|floor| !floor.is_deleted) {
        let used = used_by_floor.get(&floor.floor_id).copied().unwrap_or(0);
        let is_full = floor.total_area_centi_square_metres > 0
            && used >= floor.total_area_centi_square_metres;
        if is_full {
            full += 1;
        } else {
            vacant += 1;
        }
    }
    (full, vacant)
}

/// 未删除厂房里，`park_id` 真的指向一个未删除园区的数量，和指不上的数量。
///
/// `park_id = 0` 表示原系统里"尚未分配园区"，指向已删除园区的情况同样
/// 算指不上——两者在关系上是一回事：这栋厂房不落在任何一个当前在管的
/// 园区之下。
#[pure_function::pure]
pub(crate) fn factory_park_link_counts(
    factories: &[crate::spacetime_bindings::factory_type::Factory],
    parks: &[Park],
) -> (usize, usize) {
    let active_park_ids = parks
        .iter()
        .filter(|park| !park.is_deleted)
        .map(|park| park.park_id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut linked = 0;
    let mut orphaned = 0;
    for factory in factories.iter().filter(|factory| !factory.is_deleted) {
        if active_park_ids.contains(&factory.park_id) {
            linked += 1;
        } else {
            orphaned += 1;
        }
    }
    (linked, orphaned)
}

/// 未删除楼层里，`factory_id` 真的指向一个未删除厂房的数量，和指不上的数量。
///
/// 指不上通常意味着父厂房已经被删除——删除厂房目前不会级联删除它名下的
/// 楼层，这些楼层会变成"父级不存在"的残留数据。
#[pure_function::pure]
pub(crate) fn floor_factory_link_counts(
    floors: &[crate::spacetime_bindings::factory_floor_type::FactoryFloor],
    factories: &[crate::spacetime_bindings::factory_type::Factory],
) -> (usize, usize) {
    let active_factory_ids = factories
        .iter()
        .filter(|factory| !factory.is_deleted)
        .map(|factory| factory.factory_id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut linked = 0;
    let mut orphaned = 0;
    for floor in floors.iter().filter(|floor| !floor.is_deleted) {
        if active_factory_ids.contains(&floor.factory_id) {
            linked += 1;
        } else {
            orphaned += 1;
        }
    }
    (linked, orphaned)
}

#[pure_function::pure]
pub(crate) fn is_active_income_contract(row: &RentalTenant) -> bool {
    if row.is_deleted || !row.transaction_type {
        return false;
    }
    !matches!(
        row.status.as_deref().map(str::trim),
        Some("expired" | "已过期" | "过期" | "已退租" | "退租" | "0")
    )
}

/// 有效合同（`RentalTenant`）里，用姓名+电话匹配不到任何客户档案
/// （`TenantProfile`）的份数。
///
/// 这条关系不是外键——数据库里合同表压根没有指向客户档案表的字段，两张表
/// 能不能对上完全靠姓名+电话这个约定（跟 `tenant_management` 页面「未归档」
/// 用的是同一套判定，见 `crate::pages::tenant_management::party_key`）。
/// 这个数字越大，说明有越多合同在业务上找不到对应的客户主档。
#[pure_function::pure]
pub(crate) fn unmatched_contract_count(contracts: &[RentalTenant], profiles: &[TenantProfile]) -> usize {
    let profile_keys = profiles
        .iter()
        .filter(|profile| !profile.is_deleted)
        .map(|profile| crate::pages::tenant_management::party_key(&profile.tenant_name, &profile.phone_number))
        .collect::<BTreeSet<_>>();
    contracts
        .iter()
        .filter(|row| is_active_income_contract(row))
        .filter(|row| {
            !profile_keys.contains(&crate::pages::tenant_management::party_key(
                &row.tenant_name,
                &row.phone_number,
            ))
        })
        .count()
}

#[pure_function::pure]
pub(super) fn format_area(area: i64) -> String {
    let area = area.max(0);
    format!("{}.{:02}", area / 100, area % 100)
}

#[pure_function::pure]
pub(super) fn park_is_enabled(status: Option<&str>) -> bool {
    !matches!(status.map(str::trim), Some("0" | "disabled" | "停用"))
}

#[cfg(test)]
mod tests {
    use spacetimedb_sdk::Timestamp;

    use super::*;

    fn park(_area: i64) -> Park {
        Park {
            park_id: 7,
            customer_id: "public".into(),
            park_name: "测试园区".into(),
            address: "深圳".into(),
            description: None,
            status: Some("1".into()),
            contact: None,
            manager: None,
            is_deleted: false,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    fn tenant(area: i64, status: &str, transaction_type: bool) -> RentalTenant {
        RentalTenant {
            rental_tenant_id: 1,
            basic_ele_capacity_centi_kw: None,
            basic_ele_price_scaled: None,
            customer_id: "public".into(),
            tenant_name: "客户".into(),
            phone_number: "13800000000".into(),
            transaction_type,
            status: Some(status.into()),
            contract_start: None,
            contract_end: None,
            rental_amount_cents: None,
            increase_date: None,
            increase_rate_basis_points: None,
            increase_data: None,
            penalty_rate_basis_points: None,
            area_centi_square_metres: Some(area),
            remark: None,
            park_id: 7,
            send_message_at: None,
            is_deleted: false,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 出租率仅统计有效收入合同并限制在百分之百() {
        let rows = vec![
            tenant(8_000, "当期", true),
            tenant(5_000, "expired", true),
            tenant(2_000, "当期", false),
        ];
        let snapshot = ParkSnapshot::from_rows(park(0), &rows, 5_000);
        assert_eq!(snapshot.rented_area, 8_000);
        assert_eq!(snapshot.vacant_area, 0);
        assert_eq!(snapshot.occupancy_basis_points, 10_000);
    }

    fn floor(
        floor_id: u64,
        total: i64,
    ) -> crate::spacetime_bindings::factory_floor_type::FactoryFloor {
        crate::spacetime_bindings::factory_floor_type::FactoryFloor {
            floor_id,
            customer_id: "public".into(),
            factory_id: 1,
            floor_name: "1层".into(),
            floor_height_centi_metres: None,
            load_bearing_centi_units: None,
            rent_price_cents: 0,
            total_area_centi_square_metres: total,
            description: None,
            is_deleted: false,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    fn used(pairs: &[(u64, i64)]) -> BTreeMap<u64, i64> {
        pairs.iter().copied().collect()
    }

    fn dorm(dormitory_id: u64, park_id: u64) -> Dormitory {
        Dormitory {
            dormitory_id,
            customer_id: "public".into(),
            park_id,
            dormitory_name: "宿舍".into(),
            remark: None,
            is_deleted: false,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    fn dorm_floor(dormitory_id: u64, rooms: i32, each: Option<i64>) -> DormitoryFloor {
        DormitoryFloor {
            dormitory_floor_id: dormitory_id * 10,
            customer_id: "public".into(),
            dormitory_id,
            floor_no: 1,
            room_count: rooms,
            room_area_centi_square_metres: each,
            floor_height_centi_metres: None,
            rent_price_cents: None,
            remark: None,
            is_deleted: false,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 厂房面积逐层求和而不是首层乘层数() {
        // 生产数据里同一栋楼的楼层并不等大：1 层 5600、2 层 5350、3 层 5600。
        // 首层 × 3 会算出 16800，逐层求和才是 16550。
        let floors = vec![
            floor(1, 560_000),
            floor(2, 535_000),
            floor(3, 560_000),
        ];
        let areas = park_areas(7, &[factory(7)], &floors, &[], &[]);
        assert_eq!(areas.factory, 1_655_000);
        assert_ne!(areas.factory, 560_000 * 3);
    }

    #[test]
    fn 宿舍面积按房间数乘单间面积且与厂房分开() {
        let areas = park_areas(
            7,
            &[factory(7)],
            &[floor(1, 100_000)],
            &[dorm(1, 7)],
            &[dorm_floor(1, 20, Some(3_000))],
        );
        assert_eq!(areas.factory, 100_000);
        assert_eq!(areas.dormitory, 60_000);
    }

    #[test]
    fn 单间面积没维护的宿舍层记零而不是猜() {
        let areas = park_areas(7, &[], &[], &[dorm(1, 7)], &[dorm_floor(1, 20, None)]);
        assert_eq!(areas.dormitory, 0);
    }

    #[test]
    fn 别的园区的楼层不计入本园区面积() {
        // factory(9) 属于园区 9，查园区 7 时它名下的楼层必须被排除。
        let areas = park_areas(7, &[factory(9)], &[floor(1, 100_000)], &[], &[]);
        assert_eq!(areas.factory, 0);
    }

    fn link(rental_tenant_id: u64, floor_id: u64, area: i64) -> RentalTenantFloor {
        RentalTenantFloor {
            id: floor_id * 100 + rental_tenant_id,
            customer_id: "public".into(),
            rental_tenant_id,
            floor_id,
            area_centi_square_metres: area,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 同一层的多份合同占用累加() {
        // 一层可以分租给多个租户，已用面积是各份合同占用之和。
        let tenants = vec![tenant(0, "当期", true), tenant(0, "当期", true)];
        let mut a = tenants[0].clone();
        a.rental_tenant_id = 1;
        let mut b = tenants[1].clone();
        b.rental_tenant_id = 2;
        let used = floor_used_areas(&[link(1, 5, 30_000), link(2, 5, 20_000)], &[a, b]);
        assert_eq!(used.get(&5), Some(&50_000));
    }

    #[test]
    fn 已注销合同不再占用楼层() {
        // 判定口径必须和服务端一致，否则界面显示的可租面积会和服务端
        // 愿意放行的面积对不上。
        let mut gone = tenant(0, "当期", true);
        gone.rental_tenant_id = 1;
        gone.is_deleted = true;
        let used = floor_used_areas(&[link(1, 5, 30_000)], &[gone]);
        assert_eq!(used.get(&5), None);
    }

    #[test]
    fn 满租楼层要求合同占用覆盖全部维护面积() {
        assert_eq!(
            floor_occupancy_counts(&[floor(1, 1_000)], &used(&[(1, 1_000)])),
            (1, 0)
        );
        // 有空置面积
        assert_eq!(
            floor_occupancy_counts(&[floor(1, 1_000)], &used(&[(1, 400)])),
            (0, 1)
        );
        // 没有任何合同关联，整层空置
        assert_eq!(floor_occupancy_counts(&[floor(1, 1_000)], &used(&[])), (0, 1));
        // 面积压根没维护，跟“待租厂房”页的口径一致，算待招商
        assert_eq!(floor_occupancy_counts(&[floor(1, 0)], &used(&[])), (0, 1));
    }

    #[test]
    fn 已删除的楼层不参与统计() {
        let mut deleted = floor(1, 1_000);
        deleted.is_deleted = true;
        assert_eq!(floor_occupancy_counts(&[deleted], &used(&[(1, 1_000)])), (0, 0));
    }

    fn factory(park_id: u64) -> crate::spacetime_bindings::factory_type::Factory {
        crate::spacetime_bindings::factory_type::Factory {
            factory_id: 1,
            customer_id: "public".into(),
            factory_name: "厂房".into(),
            park_id,
            build_date: None,
            description: None,
            is_own: true,
            is_deleted: false,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 厂房归属统计区分已挂靠园区和未挂靠() {
        let parks = [park(1_000)]; // park_id = 7
        let factories = [
            factory(7),   // 挂靠有效园区
            factory(0),   // 未分配园区
            factory(999), // 指向不存在的园区
        ];
        assert_eq!(factory_park_link_counts(&factories, &parks), (1, 2));
    }

    #[test]
    fn 厂房归属已删除的园区也算未挂靠() {
        let mut deleted_park = park(1_000);
        deleted_park.is_deleted = true;
        assert_eq!(factory_park_link_counts(&[factory(7)], &[deleted_park]), (0, 1));
    }

    #[test]
    fn 楼层归属统计区分父厂房是否还存在() {
        let factories = [factory(7)]; // factory_id = 1
        let mut orphaned = floor(1_000, 1_000);
        orphaned.factory_id = 999;
        assert_eq!(
            floor_factory_link_counts(&[floor(1_000, 1_000), orphaned], &factories),
            (1, 1)
        );
    }

    #[test]
    fn 楼层归属已删除的厂房算指不上() {
        let mut deleted_factory = factory(7);
        deleted_factory.is_deleted = true;
        assert_eq!(
            floor_factory_link_counts(&[floor(1_000, 1_000)], &[deleted_factory]),
            (0, 1)
        );
    }

    fn profile(name: &str, phone: &str) -> TenantProfile {
        TenantProfile {
            tenant_profile_id: 1,
            customer_id: "public".into(),
            tenant_name: name.into(),
            tenant_type: "enterprise".into(),
            unified_social_credit_code: None,
            legal_representative: None,
            contact_name: name.into(),
            phone_number: phone.into(),
            email: None,
            address: None,
            source: None,
            status: 1,
            risk_level: "normal".into(),
            remark: None,
            is_deleted: false,
            created_at: Timestamp::UNIX_EPOCH,
            updated_at: None,
        }
    }

    #[test]
    fn 未归档合同用姓名和电话匹配不是外键() {
        let mut matched = tenant(8_000, "当期", true);
        matched.tenant_name = "张三".into();
        matched.phone_number = "13900001111".into();
        let mut unmatched = tenant(5_000, "当期", true);
        unmatched.tenant_name = "李四".into();
        unmatched.phone_number = "13900002222".into();
        let profiles = [profile("张三", "13900001111")];
        assert_eq!(
            unmatched_contract_count(&[matched, unmatched], &profiles),
            1
        );
    }

    #[test]
    fn 未归档统计只看有效收入合同() {
        let mut expired = tenant(1_000, "expired", true);
        expired.tenant_name = "王五".into();
        assert_eq!(unmatched_contract_count(&[expired], &[]), 0);
    }
}
