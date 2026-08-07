//! 当前用户有权访问的厂房与楼层。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::views::shared::identity::current_read_scope;
use crate::tables::*;

#[spacetimedb::view(accessor = my_factories, public)]
pub fn my_factories(ctx: &ViewContext) -> Vec<Factory> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut factories = ctx
        .db
        .factory()
        .factory_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|factory| !factory.is_deleted && scope.allows_park(factory.park_id))
        .collect::<Vec<_>>();
    factories.sort_by_key(|factory| factory.factory_id);
    factories
}

#[spacetimedb::view(accessor = my_factory_floors, public)]
pub fn my_factory_floors(ctx: &ViewContext) -> Vec<FactoryFloor> {
    let factory_ids = my_factories(ctx)
        .into_iter()
        .map(|factory| factory.factory_id)
        .collect::<BTreeSet<_>>();
    let mut floors = Vec::new();
    for factory_id in factory_ids {
        floors.extend(
            ctx.db
                .factory_floor()
                .factory_floor_by_factory()
                .filter(factory_id)
                .filter(|floor| !floor.is_deleted),
        );
    }
    floors.sort_by_key(|floor| floor.floor_id);
    floors
}

/// 当前账号可见园区里的水电表台账。
///
/// 按园区过滤而不是按楼层：园区公共区域的表不挂在任何一层，按楼层过滤
/// 会把「公共用电」那一类表整批漏掉。
#[spacetimedb::view(accessor = my_utility_meters, public)]
pub fn my_utility_meters(ctx: &ViewContext) -> Vec<UtilityMeter> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut meters = Vec::new();
    for park_id in scope.parks() {
        meters.extend(
            ctx.db
                .utility_meter()
                .utility_meter_by_park()
                .filter(park_id)
                .filter(|meter| !meter.is_deleted),
        );
    }
    meters.sort_by_key(|meter| meter.meter_id);
    meters
}

#[spacetimedb::view(accessor = my_dormitories, public)]
pub fn my_dormitories(ctx: &ViewContext) -> Vec<Dormitory> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut dormitories = Vec::new();
    for park_id in scope.parks() {
        dormitories.extend(
            ctx.db
                .dormitory_building()
                .dormitory_building_by_park()
                .filter(park_id)
                .filter(|dormitory| !dormitory.is_deleted),
        );
    }
    dormitories.sort_by_key(|dormitory| dormitory.dormitory_id);
    dormitories
}

/// 当前账号可见宿舍的楼层。
///
/// 数据范围跟着宿舍走，宿舍本身已按园区过滤过。
#[spacetimedb::view(accessor = my_dormitory_floors, public)]
pub fn my_dormitory_floors(ctx: &ViewContext) -> Vec<DormitoryFloor> {
    let dormitory_ids = my_dormitories(ctx)
        .into_iter()
        .map(|dormitory| dormitory.dormitory_id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut floors = Vec::new();
    for dormitory_id in dormitory_ids {
        floors.extend(
            ctx.db
                .dormitory_floor()
                .dormitory_floor_by_dormitory()
                .filter(dormitory_id)
                .filter(|floor| !floor.is_deleted),
        );
    }
    floors.sort_by_key(|floor| (floor.dormitory_id, floor.floor_no));
    floors
}
