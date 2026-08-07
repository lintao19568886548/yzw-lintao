//! 边缘计算设备的建档、激活与健康上报。
//!
//! 这里有两类调用者，权限形状完全不同：
//!
//! - **后台的人**（建档、重发注册码、吊销、注销）：要求 `/device/gateway`
//!   菜单权限与园区数据范围，与其余业务 Reducer 一致；
//! - **边缘设备自己**（激活、上报健康）：不要求任何菜单权限——它不是人，没有角色。
//!   激活靠一次性注册码换取身份绑定，此后一律以 `ctx.sender()` 认它是谁。
//!
//! 在线状态不在这里维护，由 `lifecycle.rs` 的连接钩子翻转。
//! 完整设计见 `docs/边缘计算设备与摄像头接入.md`。

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{
            current_customer_id, require_menu_path, require_park, require_park_access,
            require_park_access_unless_archived,
        },
        validation::{limited_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

/// 注册码有效期：24 小时。
///
/// 短到「今天装不完明天要重发」，长到「上午生成下午装机也来得及」。装机是
/// 现场动作，码在微信里躺一周还能用，等于给一个长期后门。
const REGISTRATION_TTL_MICROS: i64 = 24 * 3_600 * 1_000_000;

/// 注册码长度。
const REGISTRATION_CODE_LEN: usize = 8;

/// 注册码字符集：去掉了 0/O、1/I/L 这些手输容易认错的字符。
///
/// 这个码要由装机人员看着屏幕在另一台机器上敲，认错一个字符就是一次白跑。
const REGISTRATION_CHARSET: &[u8] = b"ACDEFGHJKMNPQRTUVWXY34679";

#[derive(SpacetimeType)]
pub struct EdgeGatewayInput {
    pub park_id: u64,
    pub gateway_name: String,
    pub remark: Option<String>,
}

/// 建档并生成注册码。边缘设备还没来，先在台账里占好位置。
#[spacetimedb::reducer]
pub fn create_edge_gateway(ctx: &ReducerContext, input: EdgeGatewayInput) -> Result<(), String> {
    require_menu_path(ctx, "/device/gateway")?;
    require_park_access(ctx, input.park_id)?;
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    let (park_id, gateway_name, remark) = validated_input(ctx, &customer_id, 0, input)?;
    ctx.db.edge_gateway().insert(EdgeGateway {
        gateway_id: 0,
        customer_id,
        park_id,
        gateway_name,
        registration_code: Some(generate_registration_code(ctx)),
        registration_expires_at: Some(registration_deadline(ctx)),
        is_online: false,
        status_changed_at: ctx.timestamp,
        agent_version: None,
        health_level: HEALTH_UNKNOWN.into(),
        health_detail: None,
        health_changed_at: None,
        remark,
        is_deleted: false,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    Ok(())
}

#[spacetimedb::reducer]
pub fn update_edge_gateway(
    ctx: &ReducerContext,
    id: u64,
    input: EdgeGatewayInput,
) -> Result<(), String> {
    require_menu_path(ctx, "/device/gateway")?;
    let mut existing = require_edge_gateway(ctx, id)?;
    require_park_access(ctx, input.park_id)?;
    let customer_id = existing.customer_id.clone();
    let (park_id, gateway_name, remark) = validated_input(ctx, &customer_id, id, input)?;
    existing.park_id = park_id;
    existing.gateway_name = gateway_name;
    existing.remark = remark;
    existing.updated_at = Some(ctx.timestamp);
    ctx.db.edge_gateway().gateway_id().update(existing);
    Ok(())
}

/// 重发注册码，并解除现有身份绑定。
///
/// 换机器、重装系统、或者怀疑那台电脑不再可信时用。**解绑必须和重发同时发生**：
/// 只重发不解绑的话，旧机器还拿着有效身份，照样能往库里写——「电脑是客户的」
/// 这个前提下，这不是理论风险。
#[spacetimedb::reducer]
pub fn regenerate_edge_registration_code(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    require_menu_path(ctx, "/device/gateway")?;
    let mut existing = require_edge_gateway(ctx, id)?;
    require_park_access(ctx, existing.park_id)?;
    unbind_identity(ctx, id);
    existing.registration_code = Some(generate_registration_code(ctx));
    existing.registration_expires_at = Some(registration_deadline(ctx));
    existing.is_online = false;
    existing.status_changed_at = ctx.timestamp;
    existing.agent_version = None;
    existing.health_level = HEALTH_UNKNOWN.into();
    existing.health_detail = None;
    existing.health_changed_at = None;
    existing.updated_at = Some(ctx.timestamp);
    ctx.db.edge_gateway().gateway_id().update(existing);
    Ok(())
}

/// 注销边缘设备（软删除），同时解绑身份。
#[spacetimedb::reducer]
pub fn delete_edge_gateway(ctx: &ReducerContext, id: u64) -> Result<(), String> {
    require_menu_path(ctx, "/device/gateway")?;
    let mut existing = require_edge_gateway(ctx, id)?;
    // 园区已归档时放行范围检查，否则归档园区名下的边缘设备永远清不掉。
    require_park_access_unless_archived(ctx, existing.park_id)?;
    unbind_identity(ctx, id);
    existing.is_deleted = true;
    existing.is_online = false;
    existing.status_changed_at = ctx.timestamp;
    existing.registration_code = None;
    existing.registration_expires_at = None;
    existing.updated_at = Some(ctx.timestamp);
    ctx.db.edge_gateway().gateway_id().update(existing);
    Ok(())
}

/// 边缘设备首次上电时调用：用一次性注册码换取身份绑定。
///
/// 不要求任何菜单权限——边缘设备不是人。绑定成功后，这台机器的 `ctx.sender()`
/// 就是它的长期身份，注册码当场作废。
#[spacetimedb::reducer]
pub fn activate_edge_gateway(
    ctx: &ReducerContext,
    code: String,
    agent_version: String,
) -> Result<(), String> {
    let code = required_text(code, "注册码不能为空")?;
    let mut gateway = ctx
        .db
        .edge_gateway()
        .iter()
        .find(|row| {
            !row.is_deleted && row.registration_code.as_deref() == Some(code.trim())
        })
        .ok_or("注册码无效")?;
    // 过期与无效返回同一句话：分开说等于告诉试码的人「这个码存在过」。
    if gateway
        .registration_expires_at
        .is_none_or(|deadline| deadline < ctx.timestamp)
    {
        return Err("注册码无效".into());
    }
    // 同一台边缘设备重复激活时先清掉旧绑定，否则 gateway_id 的唯一约束会拦下来，
    // 表现成「重装系统后再也激活不了」。
    unbind_identity(ctx, gateway.gateway_id);
    ctx.db.edge_gateway_identity().insert(EdgeGatewayIdentity {
        identity: ctx.sender(),
        gateway_id: gateway.gateway_id,
        bound_at: ctx.timestamp,
    });
    gateway.registration_code = None;
    gateway.registration_expires_at = None;
    gateway.agent_version = limited_optional_text(
        Some(agent_version),
        30,
        "程序版本号不能超过30个字符",
    )?;
    gateway.is_online = true;
    gateway.status_changed_at = ctx.timestamp;
    gateway.updated_at = Some(ctx.timestamp);
    ctx.db.edge_gateway().gateway_id().update(gateway);
    Ok(())
}

/// 边缘设备上报机器健康。**只在等级或详情变化时才写库。**
///
/// 电脑是客户的，它会被当办公机用、被装别的软件、被杀毒软件拦、硬盘被塞满。
/// 这些都不是我们能控制的，但必须能看见——否则每一次故障都会先赖到我们头上。
///
/// 边缘设备本地持续采样，只有跨过阈值才调这个 Reducer。即便边缘设备实现有误、每分钟
/// 都调一次，这里的「无变化就不写」也会把它挡在库外：没有写入就没有订阅推送，
/// 界面不会因此重渲染。
#[spacetimedb::reducer]
pub fn report_edge_health(
    ctx: &ReducerContext,
    level: String,
    detail: Option<String>,
) -> Result<(), String> {
    let mut gateway = require_calling_gateway(ctx)?;
    let level = required_text(level, "健康等级不能为空")?;
    check_health_level(&level)?;
    let detail = limited_optional_text(detail, 200, "健康详情不能超过200个字符")?;
    if gateway.health_level == level && gateway.health_detail == detail {
        return Ok(());
    }
    gateway.health_level = level;
    gateway.health_detail = detail;
    gateway.health_changed_at = Some(ctx.timestamp);
    gateway.updated_at = Some(ctx.timestamp);
    ctx.db.edge_gateway().gateway_id().update(gateway);
    Ok(())
}

/// 连接生命周期调用：把边缘设备标成在线／离线。
///
/// 不是 Reducer，由 `lifecycle.rs` 的 `client_connected` / `client_disconnected`
/// 转调。返回是否真的改了状态——没改就不写，避免无谓的订阅推送。
pub(crate) fn set_gateway_online(ctx: &ReducerContext, online: bool) -> bool {
    let Some(binding) = ctx.db.edge_gateway_identity().identity().find(ctx.sender()) else {
        return false;
    };
    let Some(mut gateway) = ctx.db.edge_gateway().gateway_id().find(binding.gateway_id) else {
        return false;
    };
    if gateway.is_online == online {
        return false;
    }
    gateway.is_online = online;
    gateway.status_changed_at = ctx.timestamp;
    ctx.db.edge_gateway().gateway_id().update(gateway);
    true
}

/// 解除某台设备的身份绑定；没有绑定时什么也不做。
fn unbind_identity(ctx: &ReducerContext, gateway_id: u64) {
    // `gateway_id` 是唯一列，至多一条绑定——一台边缘设备同时只认一个身份。
    if let Some(binding) = ctx.db.edge_gateway_identity().gateway_id().find(gateway_id) {
        ctx.db
            .edge_gateway_identity()
            .identity()
            .delete(binding.identity);
    }
}

/// 调用者是哪台边缘设备。身份没绑定就不是边缘设备，直接拒绝。
fn require_calling_gateway(ctx: &ReducerContext) -> Result<EdgeGateway, String> {
    let binding = ctx
        .db
        .edge_gateway_identity()
        .identity()
        .find(ctx.sender())
        .ok_or("当前身份不是已激活的边缘计算设备")?;
    ctx.db
        .edge_gateway()
        .gateway_id()
        .find(binding.gateway_id)
        .filter(|row| !row.is_deleted)
        .ok_or("边缘计算设备不存在或已注销".into())
}

fn require_edge_gateway(ctx: &ReducerContext, id: u64) -> Result<EdgeGateway, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .edge_gateway()
        .gateway_id()
        .find(id)
        .filter(|row| row.customer_id == customer_id && !row.is_deleted)
        .ok_or("边缘计算设备不存在或已注销".into())
}

fn validated_input(
    ctx: &ReducerContext,
    customer_id: &str,
    gateway_id: u64,
    input: EdgeGatewayInput,
) -> Result<(u64, String, Option<String>), String> {
    if input.park_id == 0 {
        return Err("请选择所属园区".into());
    }
    require_park(ctx, input.park_id)?;
    let gateway_name = required_text(input.gateway_name, "设备名称不能为空")?;
    validate_max_length(&gateway_name, 50, "设备名称不能超过50个字符")?;
    let duplicated = ctx
        .db
        .edge_gateway()
        .edge_gateway_by_park()
        .filter(input.park_id)
        .any(|row| {
            !row.is_deleted
                && row.customer_id == customer_id
                && row.gateway_id != gateway_id
                && row.gateway_name == gateway_name
        });
    if duplicated {
        return Err(format!("园区内已存在名称「{gateway_name}」的边缘设备"));
    }
    let remark = limited_optional_text(input.remark, 200, "备注不能超过200个字符")?;
    Ok((input.park_id, gateway_name, remark))
}

fn registration_deadline(ctx: &ReducerContext) -> spacetimedb::Timestamp {
    spacetimedb::Timestamp::from_micros_since_unix_epoch(
        ctx.timestamp
            .to_micros_since_unix_epoch()
            .saturating_add(REGISTRATION_TTL_MICROS),
    )
}

fn generate_registration_code(ctx: &ReducerContext) -> String {
    (0..REGISTRATION_CODE_LEN)
        .map(|_| {
            let pick: u64 = ctx.random();
            REGISTRATION_CHARSET[(pick % REGISTRATION_CHARSET.len() as u64) as usize] as char
        })
        .collect()
}

/// 校验健康等级取值。
#[pure_function::pure]
pub(crate) fn check_health_level(level: &str) -> Result<(), String> {
    if matches!(level, HEALTH_NORMAL | HEALTH_WARNING | HEALTH_CRITICAL) {
        Ok(())
    } else {
        Err("健康等级只能是 normal、warning 或 critical".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 健康等级只接受三个取值() {
        assert!(check_health_level(HEALTH_NORMAL).is_ok());
        assert!(check_health_level(HEALTH_WARNING).is_ok());
        assert!(check_health_level(HEALTH_CRITICAL).is_ok());
        // unknown 是建档时的初始值，边缘设备不该上报它——上报「我不知道自己怎么样」
        // 和没上报没有区别，只会让界面上多一种说不清的状态。
        assert!(check_health_level(HEALTH_UNKNOWN).is_err());
        assert!(check_health_level("").is_err());
        assert!(check_health_level("正常").is_err());
    }

    #[test]
    fn 注册码字符集不含形近字符() {
        for ambiguous in [b'0', b'O', b'1', b'I', b'L', b'S', b'5', b'B', b'8', b'2', b'Z'] {
            assert!(
                !REGISTRATION_CHARSET.contains(&ambiguous),
                "{} 是手输容易认错的字符，不该进注册码",
                ambiguous as char
            );
        }
        assert!(REGISTRATION_CHARSET.len() >= 20, "字符集太小，码的强度不够");
    }
}
