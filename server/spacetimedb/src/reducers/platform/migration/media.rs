//! 从原 MySQL 导入园区与厂房楼层图片关系。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::shared::access::{AdminContext, current_customer_id},
    tables::{FactoryFloorImage, Image, ParkImage, *},
};

use super::sequence::align_media_import_sequences;

const CONTRACT_MEDIA_MIGRATION_OWNER: &str =
    "c200465e70fb4bc2eb6dbc48e4f430ac76389ae3ae0936893243633e2a083356";

#[derive(SpacetimeType)]
pub struct MysqlMediaImageImport {
    pub img_id: u64,
    pub img_url: String,
    pub hash: String,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlParkImageImport {
    pub id: u64,
    pub park_id: u64,
    pub img_id: u64,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlFactoryFloorImageImport {
    pub id: u64,
    pub floor_id: u64,
    pub img_id: u64,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlMediaBatchImport {
    pub images: Vec<MysqlMediaImageImport>,
    pub park_images: Vec<MysqlParkImageImport>,
    pub factory_floor_images: Vec<MysqlFactoryFloorImageImport>,
}

#[derive(SpacetimeType)]
pub struct MysqlTenantImageImport {
    pub rental_tenant_id: u64,
    pub img_id: u64,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlContractMediaBatchImport {
    pub customer_id: String,
    pub images: Vec<MysqlMediaImageImport>,
    pub tenant_images: Vec<MysqlTenantImageImport>,
}

fn timestamp(value: Option<i64>, fallback: Timestamp) -> Timestamp {
    value
        .map(Timestamp::from_micros_since_unix_epoch)
        .unwrap_or(fallback)
}

fn optional_timestamp(value: Option<i64>) -> Option<Timestamp> {
    value.map(Timestamp::from_micros_since_unix_epoch)
}

/// 以 MySQL 主键幂等导入图片及其园区、楼层关系。
#[spacetimedb::reducer]
pub fn import_mysql_media_batch(
    ctx: &ReducerContext,
    batch: MysqlMediaBatchImport,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;

    for source in &batch.images {
        if source.img_url.trim().is_empty() || source.hash.trim().is_empty() {
            return Err(format!("图片 {} 的地址或哈希为空", source.img_id));
        }
        if let Some(existing) = ctx.db.image().img_id().find(source.img_id) {
            if existing.customer_id != customer_id {
                return Err(format!("图片 {} 已属于其他租户", source.img_id));
            }
        }
    }

    for source in &batch.park_images {
        let park = ctx
            .db
            .park()
            .park_id()
            .find(source.park_id)
            .ok_or_else(|| format!("园区图片关系 {} 的园区不存在", source.id))?;
        if park.customer_id != customer_id {
            return Err(format!("园区 {} 已属于其他租户", source.park_id));
        }
    }

    for source in &batch.factory_floor_images {
        let floor = ctx
            .db
            .factory_floor()
            .floor_id()
            .find(source.floor_id)
            .ok_or_else(|| format!("楼层图片关系 {} 的楼层不存在", source.id))?;
        if floor.customer_id != customer_id {
            return Err(format!("楼层 {} 已属于其他租户", source.floor_id));
        }
    }

    for source in batch.images {
        let row = Image {
            img_id: source.img_id,
            customer_id: customer_id.clone(),
            img_url: source.img_url,
            hash: source.hash,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        if ctx.db.image().img_id().find(row.img_id).is_some() {
            ctx.db.image().img_id().update(row);
        } else {
            ctx.db.image().insert(row);
        }
    }

    for source in batch.park_images {
        let image = ctx
            .db
            .image()
            .img_id()
            .find(source.img_id)
            .ok_or_else(|| format!("园区图片关系 {} 的图片不存在", source.id))?;
        if image.customer_id != customer_id {
            return Err(format!("图片 {} 已属于其他租户", source.img_id));
        }
        let row = ParkImage {
            id: source.id,
            customer_id: customer_id.clone(),
            park_id: source.park_id,
            img_id: source.img_id,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx.db.park_image().id().find(row.id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("园区图片关系 {} 已属于其他租户", row.id));
            }
            Some(_) => {
                ctx.db.park_image().id().update(row);
            }
            None => {
                if ctx
                    .db
                    .park_image()
                    .park_image_by_pair()
                    .filter((row.park_id, row.img_id))
                    .next()
                    .is_some()
                {
                    return Err(format!("园区 {} 已绑定图片 {}", row.park_id, row.img_id));
                }
                ctx.db.park_image().insert(row);
            }
        }
    }

    for source in batch.factory_floor_images {
        let image = ctx
            .db
            .image()
            .img_id()
            .find(source.img_id)
            .ok_or_else(|| format!("楼层图片关系 {} 的图片不存在", source.id))?;
        if image.customer_id != customer_id {
            return Err(format!("图片 {} 已属于其他租户", source.img_id));
        }
        let row = FactoryFloorImage {
            id: source.id,
            customer_id: customer_id.clone(),
            floor_id: source.floor_id,
            img_id: source.img_id,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx.db.factory_floor_image().id().find(row.id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("楼层图片关系 {} 已属于其他租户", row.id));
            }
            Some(_) => {
                ctx.db.factory_floor_image().id().update(row);
            }
            None => {
                if ctx
                    .db
                    .factory_floor_image()
                    .floor_image_by_pair()
                    .filter((row.floor_id, row.img_id))
                    .next()
                    .is_some()
                {
                    return Err(format!("楼层 {} 已绑定图片 {}", row.floor_id, row.img_id));
                }
                ctx.db.factory_floor_image().insert(row);
            }
        }
    }

    align_media_import_sequences(ctx);
    Ok(())
}

/// 从旧 MySQL 幂等迁移合同原图及合同关系。
///
/// 该入口不依赖业务登录会话，只允许生产数据库所有者身份调用，避免为了离线迁移
/// 临时开放匿名写入或重置管理员密码。图片保留旧主键，合同关系按业务唯一键去重。
#[spacetimedb::reducer]
pub fn import_mysql_contract_media_batch(
    ctx: &ReducerContext,
    batch: MysqlContractMediaBatchImport,
) -> Result<(), String> {
    if ctx.sender().to_string() != CONTRACT_MEDIA_MIGRATION_OWNER {
        return Err("仅数据库所有者可以执行合同图片迁移".into());
    }
    let customer_id = batch.customer_id.trim().to_string();
    if customer_id.is_empty() || ctx.db.customer().customer_id().find(&customer_id).is_none() {
        return Err("迁移目标租户不存在".into());
    }

    for source in &batch.images {
        if source.img_url.trim().is_empty() || source.hash.trim().is_empty() {
            return Err(format!("图片 {} 的地址或哈希为空", source.img_id));
        }
        if let Some(existing) = ctx.db.image().img_id().find(source.img_id) {
            if existing.customer_id != customer_id {
                return Err(format!("图片 {} 已属于其他租户", source.img_id));
            }
            if existing.hash != source.hash {
                return Err(format!("图片 {} 的哈希与现有记录不一致", source.img_id));
            }
        }
    }

    for source in &batch.tenant_images {
        let tenant = ctx
            .db
            .rental_tenant()
            .rental_tenant_id()
            .find(source.rental_tenant_id)
            .ok_or_else(|| format!("合同 {} 不存在", source.rental_tenant_id))?;
        if tenant.customer_id != customer_id {
            return Err(format!("合同 {} 已属于其他租户", source.rental_tenant_id));
        }
        let image_in_batch = batch
            .images
            .iter()
            .any(|image| image.img_id == source.img_id);
        let image_in_database = ctx
            .db
            .image()
            .img_id()
            .find(source.img_id)
            .is_some_and(|image| image.customer_id == customer_id);
        if !image_in_batch && !image_in_database {
            return Err(format!("合同图片 {} 不存在", source.img_id));
        }
    }

    for source in batch.images {
        let row = Image {
            img_id: source.img_id,
            customer_id: customer_id.clone(),
            img_url: source.img_url,
            hash: source.hash,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        if ctx.db.image().img_id().find(row.img_id).is_some() {
            ctx.db.image().img_id().update(row);
        } else {
            ctx.db.image().insert(row);
        }
    }

    for source in batch.tenant_images {
        if ctx
            .db
            .tenant_image()
            .tenant_image_by_pair()
            .filter((source.rental_tenant_id, source.img_id))
            .next()
            .is_some()
        {
            continue;
        }
        ctx.db.tenant_image().insert(TenantImage {
            id: 0,
            customer_id: customer_id.clone(),
            rental_tenant_id: source.rental_tenant_id,
            img_id: source.img_id,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        });
    }

    align_media_import_sequences(ctx);
    Ok(())
}
