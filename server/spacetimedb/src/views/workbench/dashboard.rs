//! 当前登录用户可见范围内的总览聚合数据。

use std::collections::BTreeMap;

use spacetimedb::{SpacetimeType, ViewContext};

use crate::views::{
    finance_views::approvals::reimbursement::my_reimbursements, hr::records::my_employees,
    investment::records::my_investments, shared::identity::current_read_scope,
};
use crate::tables::*;

#[derive(SpacetimeType)]
pub struct DashboardRevenuePulse {
    pub period: String,
    pub receivable_cents: i64,
    pub received_cents: i64,
}

#[derive(SpacetimeType)]
pub struct DashboardOverview {
    pub customer_id: String,
    pub factory_count: u64,
    pub tenant_count: u64,
    pub occupied_area_centi_square_metres: i64,
    pub receivable_cents: i64,
    pub received_cents: i64,
    pub outstanding_cents: i64,
    pub outstanding_bill_count: u64,
    pub investment_count: u64,
    pub pending_reimbursement_count: u64,
    pub employee_count: u64,
    pub revenue_pulse: Vec<DashboardRevenuePulse>,
}

#[spacetimedb::view(accessor = dashboard_overview, public)]
pub fn dashboard_overview(ctx: &ViewContext) -> Option<DashboardOverview> {
    let scope = current_read_scope(ctx)?;

    // 总览直接依赖业务原表。客户端只订阅这一行聚合结果，账单、租户、厂房
    // 发生变化时由 SpacetimeDB 在服务端重新计算并推送，无需订阅历史明细表。
    let factories = ctx
        .db
        .factory()
        .factory_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted && scope.allows_park(row.park_id))
        .collect::<Vec<_>>();
    let tenants = ctx
        .db
        .rental_tenant()
        .rental_tenant_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| !row.is_deleted && scope.allows_park(row.park_id))
        .collect::<Vec<_>>();
    let bills = ctx
        .db
        .amount_bill()
        .amount_bill_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| scope.allows_park(row.park_id))
        .collect::<Vec<_>>();
    let investments = my_investments(ctx);
    let reimbursements = my_reimbursements(ctx);
    let employees = my_employees(ctx);

    let receivable_cents = bills.iter().map(|bill| bill.total_fee_cents).sum();
    let received_cents = bills.iter().map(|bill| bill.receipt_amount_cents).sum();
    let outstanding_cents = bills
        .iter()
        .map(|bill| (bill.total_fee_cents - bill.receipt_amount_cents).max(0))
        .sum();
    let outstanding_bill_count = bills
        .iter()
        .filter(|bill| bill.receipt_amount_cents < bill.total_fee_cents)
        .count() as u64;
    let occupied_area_centi_square_metres = tenants
        .iter()
        .filter_map(|tenant| tenant.area_centi_square_metres)
        .sum();

    let mut pulse = BTreeMap::<String, (i64, i64)>::new();
    for bill in &bills {
        let timestamp = bill.created_at.to_string();
        let period = timestamp.get(..7).unwrap_or("未知月份").to_string();
        let totals = pulse.entry(period).or_default();
        totals.0 += bill.total_fee_cents;
        totals.1 += bill.receipt_amount_cents;
    }
    let revenue_pulse = pulse
        .into_iter()
        .rev()
        .take(7)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(
            |(period, (receivable_cents, received_cents))| DashboardRevenuePulse {
                period,
                receivable_cents,
                received_cents,
            },
        )
        .collect();

    Some(DashboardOverview {
        customer_id: scope.customer_id,
        factory_count: factories.len() as u64,
        tenant_count: tenants.len() as u64,
        occupied_area_centi_square_metres,
        receivable_cents,
        received_cents,
        outstanding_cents,
        outstanding_bill_count,
        investment_count: investments.len() as u64,
        pending_reimbursement_count: reimbursements.iter().filter(|row| row.status == 0).count()
            as u64,
        employee_count: employees.len() as u64,
        revenue_pulse,
    })
}
