//! 从原 MySQL 导入厂房与楼层资产关系。

use spacetimedb::{ReducerContext, SpacetimeType, Table, Timestamp};

use crate::{
    reducers::shared::access::{AdminContext, current_customer_id},
    tables::{Factory, FactoryFloor, *},
};

use super::sequence::align_factory_import_sequences;

#[derive(SpacetimeType)]
pub struct MysqlFactoryImport {
    pub factory_id: u64,
    pub factory_name: String,
    pub park_id: u64,
    pub build_date: Option<String>,
    pub address: String,
    pub contact: String,
    pub description: Option<String>,
    pub is_own: bool,
    pub is_deleted: bool,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlFactoryFloorImport {
    pub floor_id: u64,
    pub floor_name: String,
    pub factory_id: u64,
    pub floor_height_centi_metres: Option<i64>,
    pub load_bearing_centi_units: Option<i64>,
    pub rent_price_cents: i64,
    pub total_area_centi_square_metres: i64,
    pub used_area_centi_square_metres: i64,
    pub status: String,
    pub description: Option<String>,
    pub is_deleted: bool,
    pub created_at_micros: Option<i64>,
    pub updated_at_micros: Option<i64>,
}

#[derive(SpacetimeType)]
pub struct MysqlFactoryBatchImport {
    pub factories: Vec<MysqlFactoryImport>,
    pub floors: Vec<MysqlFactoryFloorImport>,
}

fn timestamp(value: Option<i64>, fallback: Timestamp) -> Timestamp {
    value
        .map(Timestamp::from_micros_since_unix_epoch)
        .unwrap_or(fallback)
}

fn optional_timestamp(value: Option<i64>) -> Option<Timestamp> {
    value.map(Timestamp::from_micros_since_unix_epoch)
}

/// 以原主键幂等导入厂房关系；任一父子关系异常时整个批次自动回滚。
#[spacetimedb::reducer]
pub fn import_mysql_factory_batch(
    ctx: &ReducerContext,
    batch: MysqlFactoryBatchImport,
) -> Result<(), String> {
    AdminContext::require(ctx)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    if batch.factories.len() > 2_000 || batch.floors.len() > 20_000 {
        return Err("单次厂房迁移数据量超过安全限制".into());
    }

    for source in batch.factories {
        // 原 MySQL 存在少量父园区已被物理删除的历史孤儿数据。
        // 这里按当前模型约定归入“未分配园区”，避免为不存在的园区伪造主记录。
        let park_id = if source.park_id == 0 {
            0
        } else {
            match ctx.db.park().park_id().find(source.park_id) {
                Some(park) if park.customer_id == customer_id => source.park_id,
                Some(_) => return Err(format!("园区 {} 已属于其他租户", source.park_id)),
                None => 0,
            }
        };
        let row = Factory {
            factory_id: source.factory_id,
            customer_id: customer_id.clone(),
            factory_name: source.factory_name,
            park_id,
            build_date: source.build_date,
            description: source.description,
            is_own: source.is_own,
            is_deleted: source.is_deleted,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx.db.factory().factory_id().find(row.factory_id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("厂房 {} 已属于其他租户", row.factory_id));
            }
            Some(_) => {
                ctx.db.factory().factory_id().update(row);
            }
            None => {
                ctx.db.factory().insert(row);
            }
        }
    }

    for source in batch.floors {
        let factory = ctx
            .db
            .factory()
            .factory_id()
            .find(source.factory_id)
            .ok_or_else(|| format!("楼层 {} 关联厂房不存在", source.floor_id))?;
        if factory.customer_id != customer_id {
            return Err(format!("厂房 {} 已属于其他租户", source.factory_id));
        }
        let row = FactoryFloor {
            floor_id: source.floor_id,
            customer_id: customer_id.clone(),
            factory_id: source.factory_id,
            floor_name: source.floor_name,
            floor_height_centi_metres: source.floor_height_centi_metres,
            load_bearing_centi_units: source.load_bearing_centi_units,
            rent_price_cents: source.rent_price_cents,
            total_area_centi_square_metres: source.total_area_centi_square_metres,
            description: source.description,
            is_deleted: source.is_deleted,
            created_at: timestamp(source.created_at_micros, ctx.timestamp),
            updated_at: optional_timestamp(source.updated_at_micros),
        };
        match ctx.db.factory_floor().floor_id().find(row.floor_id) {
            Some(existing) if existing.customer_id != customer_id => {
                return Err(format!("楼层 {} 已属于其他租户", row.floor_id));
            }
            Some(_) => {
                ctx.db.factory_floor().floor_id().update(row);
            }
            None => {
                ctx.db.factory_floor().insert(row);
            }
        }
    }

    // 显式写入历史主键不会自动推进 SpacetimeDB 序列，导入后必须校准。
    align_factory_import_sequences(ctx);
    Ok(())
}
