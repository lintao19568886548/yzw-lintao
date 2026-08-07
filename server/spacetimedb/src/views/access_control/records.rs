//! 当前用户有权访问的访客、车辆和门禁设备。

use spacetimedb::ViewContext;

use crate::{tables::*, views::shared::identity::current_read_scope};

#[spacetimedb::view(accessor = my_access_visitors, public)]
pub fn my_access_visitors(ctx: &ViewContext) -> Vec<AccessVisitor> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .access_visitor()
        .access_visitor_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| scope.allows_optional_park(row.park_id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.visitor_id);
    rows
}

#[spacetimedb::view(accessor = my_access_cars, public)]
pub fn my_access_cars(ctx: &ViewContext) -> Vec<AccessCar> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .access_car()
        .access_car_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| scope.allows_optional_park(row.park_id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.car_id);
    rows
}

#[spacetimedb::view(accessor = my_access_doors, public)]
pub fn my_access_doors(ctx: &ViewContext) -> Vec<AccessDoor> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut rows = ctx
        .db
        .access_door()
        .access_door_by_customer()
        .filter(scope.customer_id.as_str())
        .filter(|row| scope.allows_optional_park(row.park_id))
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.device_id);
    rows
}
