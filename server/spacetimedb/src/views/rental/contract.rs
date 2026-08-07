//! 当前用户有权访问的租赁客户。

use std::collections::BTreeSet;

use spacetimedb::ViewContext;

use crate::views::shared::identity::current_read_scope;
use crate::tables::*;

#[spacetimedb::view(accessor = my_tenant_profiles, public)]
pub fn my_tenant_profiles(ctx: &ViewContext) -> Vec<TenantProfile> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let visible_parties = my_rental_tenants(ctx)
        .into_iter()
        .map(|row| {
            format!(
                "{}|{}",
                row.tenant_name.trim().to_lowercase(),
                row.phone_number.trim()
            )
        })
        .collect::<BTreeSet<_>>();
    let mut rows = ctx
        .db
        .tenant_profile()
        .tenant_profile_by_customer()
        .filter(scope.customer_id.as_str())
        // 管理员可见租户全部档案；其他人只能看到与自己可见租客同名同号的档案。
        .filter(|row| {
            !row.is_deleted
                && (scope.is_unrestricted()
                    || visible_parties.contains(&format!(
                        "{}|{}",
                        row.tenant_name.trim().to_lowercase(),
                        row.phone_number.trim()
                    )))
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.tenant_profile_id);
    rows
}

#[spacetimedb::view(accessor = my_rental_tenants, public)]
pub fn my_rental_tenants(ctx: &ViewContext) -> Vec<RentalTenant> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut tenants = ctx
        .db
        .rental_tenant()
        .rental_tenant_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|tenant| !tenant.is_deleted && scope.allows_park(tenant.park_id))
        .collect::<Vec<_>>();
    tenants.sort_by_key(|tenant| tenant.rental_tenant_id);
    tenants
}


/// 当前账号可见合同的楼层归属关系。
///
/// 数据范围跟着合同走（`my_rental_tenants` 已经按租户和园区过滤过），
/// 这样园区受限的账号不会通过关联表看到别的园区的楼层占用情况。
#[spacetimedb::view(accessor = my_rental_tenant_floors, public)]
pub fn my_rental_tenant_floors(ctx: &ViewContext) -> Vec<RentalTenantFloor> {
    let tenant_ids = my_rental_tenants(ctx)
        .into_iter()
        .map(|tenant| tenant.rental_tenant_id)
        .collect::<BTreeSet<_>>();
    let mut links = Vec::new();
    for rental_tenant_id in tenant_ids {
        links.extend(
            ctx.db
                .rental_tenant_floor()
                .rental_tenant_floor_by_tenant()
                .filter(rental_tenant_id),
        );
    }
    links.sort_by_key(|link| link.id);
    links
}

/// 当前账号可见合同的用表关系与约定水电单价。
///
/// 数据范围跟着合同走：园区受限的账号不会通过关联表看到别的园区的水电报价。
#[spacetimedb::view(accessor = my_rental_tenant_meters, public)]
pub fn my_rental_tenant_meters(ctx: &ViewContext) -> Vec<RentalTenantMeter> {
    let tenant_ids = my_rental_tenants(ctx)
        .into_iter()
        .map(|tenant| tenant.rental_tenant_id)
        .collect::<BTreeSet<_>>();
    let mut links = Vec::new();
    for rental_tenant_id in tenant_ids {
        links.extend(
            ctx.db
                .rental_tenant_meter()
                .rental_tenant_meter_by_tenant()
                .filter(rental_tenant_id),
        );
    }
    links.sort_by_key(|link| link.id);
    links
}

/// 当前账号可见合同约定的周期性费用（电损／服务／垃圾）。
///
/// 数据范围跟着合同走，口径与 [`my_rental_tenant_meters`] 一致。
#[spacetimedb::view(accessor = my_rental_tenant_fees, public)]
pub fn my_rental_tenant_fees(ctx: &ViewContext) -> Vec<RentalTenantFee> {
    let tenant_ids = my_rental_tenants(ctx)
        .into_iter()
        .map(|tenant| tenant.rental_tenant_id)
        .collect::<BTreeSet<_>>();
    let mut rows = Vec::new();
    for rental_tenant_id in tenant_ids {
        rows.extend(
            ctx.db
                .rental_tenant_fee()
                .rental_tenant_fee_by_tenant()
                .filter(rental_tenant_id),
        );
    }
    rows.sort_by_key(|row| row.id);
    rows
}

/// 当前账号可见合同的宿舍楼层归属。
///
/// 「某层已用几间」就是从这里算出来的：房间总数固定，减去这里的占用即为可租。
#[spacetimedb::view(accessor = my_rental_tenant_dormitory_floors, public)]
pub fn my_rental_tenant_dormitory_floors(ctx: &ViewContext) -> Vec<RentalTenantDormitoryFloor> {
    let tenant_ids = my_rental_tenants(ctx)
        .into_iter()
        .map(|tenant| tenant.rental_tenant_id)
        .collect::<BTreeSet<_>>();
    let mut links = Vec::new();
    for rental_tenant_id in tenant_ids {
        links.extend(
            ctx.db
                .rental_tenant_dormitory_floor()
                .rental_tenant_dormitory_floor_by_tenant()
                .filter(rental_tenant_id),
        );
    }
    links.sort_by_key(|link| link.id);
    links
}
