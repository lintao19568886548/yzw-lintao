//! 当前用户有权订阅的图片与资产图片关系。

use std::collections::BTreeSet;

use spacetimedb::{SpacetimeType, ViewContext};

use super::support::my_feedbacks;
use crate::views::{
    finance_views::approvals::reimbursement::my_reimbursement_images,
    hr::records::my_salaries,
    rental::{
        assets::{my_dormitories, my_factory_floors},
        contract::my_rental_tenants,
    },
    shared::identity::current_read_scope,
};
use crate::tables::*;

#[spacetimedb::view(accessor = my_park_images, public)]
pub fn my_park_images(ctx: &ViewContext) -> Vec<ParkImage> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut links = Vec::new();
    for park_id in scope.parks() {
        links.extend(ctx.db.park_image().park_image_by_park().filter(park_id));
    }
    links.sort_by_key(|link| link.id);
    links
}

/// 园区列表直接使用的轻量图片信息，避免客户端订阅租户下全部业务图片。
#[derive(SpacetimeType)]
pub struct ParkImagePreview {
    pub park_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_park_image_previews, public)]
pub fn my_park_image_previews(ctx: &ViewContext) -> Vec<ParkImagePreview> {
    let mut previews = my_park_images(ctx)
        .into_iter()
        .filter_map(|link| {
            let image = ctx.db.image().img_id().find(link.img_id)?;
            Some(ParkImagePreview {
                park_id: link.park_id,
                img_id: link.img_id,
                img_url: image.img_url,
            })
        })
        .collect::<Vec<_>>();
    previews.sort_by_key(|row| (row.park_id, row.img_id));
    previews
}

#[spacetimedb::view(accessor = my_factory_floor_images, public)]
pub fn my_factory_floor_images(ctx: &ViewContext) -> Vec<FactoryFloorImage> {
    let mut links = Vec::new();
    for floor in my_factory_floors(ctx) {
        links.extend(
            ctx.db
                .factory_floor_image()
                .floor_image_by_floor()
                .filter(floor.floor_id),
        );
    }
    links.sort_by_key(|link| link.id);
    links
}

/// 园区详情直接使用的楼层图片信息，避免 Super 账号订阅租户下全部图片主表。
#[derive(SpacetimeType)]
pub struct FactoryFloorImagePreview {
    pub floor_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_factory_floor_image_previews, public)]
pub fn my_factory_floor_image_previews(ctx: &ViewContext) -> Vec<FactoryFloorImagePreview> {
    let mut previews = my_factory_floor_images(ctx)
        .into_iter()
        .filter_map(|link| {
            let image = ctx.db.image().img_id().find(link.img_id)?;
            Some(FactoryFloorImagePreview {
                floor_id: link.floor_id,
                img_id: link.img_id,
                img_url: image.img_url,
            })
        })
        .collect::<Vec<_>>();
    previews.sort_by_key(|row| (row.floor_id, row.img_id));
    previews
}

#[spacetimedb::view(accessor = my_dormitory_images, public)]
pub fn my_dormitory_images(ctx: &ViewContext) -> Vec<DormitoryImage> {
    let mut links = Vec::new();
    for dormitory in my_dormitories(ctx) {
        links.extend(
            ctx.db
                .dormitory_image()
                .dormitory_image_by_dormitory()
                .filter(dormitory.dormitory_id),
        );
    }
    links.sort_by_key(|link| link.id);
    links
}

/// 园区详情直接使用的宿舍图片信息。
#[derive(SpacetimeType)]
pub struct DormitoryImagePreview {
    pub dormitory_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_dormitory_image_previews, public)]
pub fn my_dormitory_image_previews(ctx: &ViewContext) -> Vec<DormitoryImagePreview> {
    let mut previews = my_dormitory_images(ctx)
        .into_iter()
        .filter_map(|link| {
            let image = ctx.db.image().img_id().find(link.img_id)?;
            Some(DormitoryImagePreview {
                dormitory_id: link.dormitory_id,
                img_id: link.img_id,
                img_url: image.img_url,
            })
        })
        .collect::<Vec<_>>();
    previews.sort_by_key(|row| (row.dormitory_id, row.img_id));
    previews
}

#[spacetimedb::view(accessor = my_tenant_images, public)]
pub fn my_tenant_images(ctx: &ViewContext) -> Vec<TenantImage> {
    let mut links = Vec::new();
    for tenant in my_rental_tenants(ctx) {
        links.extend(
            ctx.db
                .tenant_image()
                .tenant_image_by_tenant()
                .filter(tenant.rental_tenant_id),
        );
    }
    links.sort_by_key(|link| link.id);
    links
}

/// 合同表单直接使用的最小图片信息。
#[derive(SpacetimeType)]
pub struct TenantImagePreview {
    pub rental_tenant_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_tenant_image_previews, public)]
pub fn my_tenant_image_previews(ctx: &ViewContext) -> Vec<TenantImagePreview> {
    let mut previews = my_tenant_images(ctx)
        .into_iter()
        .filter_map(|link| {
            let image = ctx.db.image().img_id().find(link.img_id)?;
            Some(TenantImagePreview {
                rental_tenant_id: link.rental_tenant_id,
                img_id: link.img_id,
                img_url: image.img_url,
            })
        })
        .collect::<Vec<_>>();
    previews.sort_by_key(|row| (row.rental_tenant_id, row.img_id));
    previews
}

#[spacetimedb::view(accessor = my_salary_images, public)]
pub fn my_salary_images(ctx: &ViewContext) -> Vec<SalaryImage> {
    let mut links = Vec::new();
    for salary in my_salaries(ctx) {
        links.extend(
            ctx.db
                .salary_image()
                .salary_image_by_salary()
                .filter(salary.salary_id),
        );
    }
    links.sort_by_key(|link| link.id);
    links
}

/// 工资页面直接使用的最小图片信息，避免 Super 订阅租户下全部业务图片。
#[derive(SpacetimeType)]
pub struct SalaryImagePreview {
    pub salary_id: u64,
    pub img_id: u64,
    pub img_url: String,
}

#[spacetimedb::view(accessor = my_salary_image_previews, public)]
pub fn my_salary_image_previews(ctx: &ViewContext) -> Vec<SalaryImagePreview> {
    let mut previews = my_salary_images(ctx)
        .into_iter()
        .filter_map(|link| {
            let image = ctx.db.image().img_id().find(link.img_id)?;
            Some(SalaryImagePreview {
                salary_id: link.salary_id,
                img_id: link.img_id,
                img_url: image.img_url,
            })
        })
        .collect::<Vec<_>>();
    previews.sort_by_key(|row| (row.salary_id, row.img_id));
    previews
}

#[spacetimedb::view(accessor = my_feedback_images, public)]
pub fn my_feedback_images(ctx: &ViewContext) -> Vec<ImageBinding> {
    let mut links = Vec::new();
    for feedback in my_feedbacks(ctx) {
        links.extend(
            ctx.db
                .image_binding()
                .image_binding_by_business()
                .filter(("feedback", feedback.id)),
        );
    }
    links.sort_by_key(|link| (link.biz_id, link.sort, link.id));
    links
}

#[spacetimedb::view(accessor = my_images, public)]
pub fn my_images(ctx: &ViewContext) -> Vec<Image> {
    let Some(scope) = current_read_scope(ctx) else {
        return vec![];
    };
    let mut image_ids = BTreeSet::new();
    image_ids.extend(my_park_images(ctx).into_iter().map(|link| link.img_id));
    image_ids.extend(
        my_factory_floor_images(ctx)
            .into_iter()
            .map(|link| link.img_id),
    );
    image_ids.extend(my_dormitory_images(ctx).into_iter().map(|link| link.img_id));
    image_ids.extend(my_tenant_images(ctx).into_iter().map(|link| link.img_id));
    image_ids.extend(my_salary_images(ctx).into_iter().map(|link| link.img_id));
    image_ids.extend(
        my_reimbursement_images(ctx)
            .into_iter()
            .map(|link| link.img_id),
    );
    image_ids.extend(my_feedback_images(ctx).into_iter().map(|link| link.img_id));
    let mut images = ctx
        .db
        .image()
        .image_by_customer()
        .filter(scope.customer_id.as_str())
        // 管理员可见租户下全部图片；其他人只能看到有权访问的业务记录引用到的图片。
        .filter(|image| scope.is_unrestricted() || image_ids.contains(&image.img_id))
        .collect::<Vec<_>>();
    images.sort_by_key(|image| image.img_id);
    images
}
