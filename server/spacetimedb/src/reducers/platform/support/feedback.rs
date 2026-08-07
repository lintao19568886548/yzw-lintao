//! 用户反馈提交与图片关系维护。

use std::collections::BTreeSet;

use spacetimedb::{ReducerContext, SpacetimeType, Table};

use crate::{
    reducers::{
        access::{
            AdminContext, current_center_user_id, current_customer_id, current_user_id,
            require_image, require_user,
        },
        validation::{normalize_optional_text, required_text, validate_max_length},
    },
    tables::*,
};

const FEEDBACK_BIZ_TYPE: &str = "feedback";
const FEEDBACK_IMAGE_FIELD: &str = "gallery";

/// 客户端提交反馈时允许填写的字段。
#[derive(SpacetimeType)]
pub struct FeedbackInput {
    pub category: String,
    pub content: String,
    pub contact: Option<String>,
    pub client_platform: Option<String>,
    pub user_agent: Option<String>,
    pub image_ids: Vec<u64>,
}

fn validate_category(category: String) -> Result<String, String> {
    let category = required_text(category, "请选择反馈类型")?;
    matches!(
        category.as_str(),
        "bug" | "experience" | "feature" | "other"
    )
    .then_some(category)
    .ok_or("反馈类型无效".into())
}

fn validate_content(content: String) -> Result<String, String> {
    let content = required_text(content, "请输入反馈内容")?;
    let count = content.chars().count();
    (10..=500)
        .contains(&count)
        .then_some(content)
        .ok_or("反馈内容长度必须为 10 到 500 个字符".into())
}

fn validate_optional(
    value: Option<String>,
    max_chars: usize,
    message: &'static str,
) -> Result<Option<String>, String> {
    let value = normalize_optional_text(value);
    if let Some(text) = value.as_deref() {
        validate_max_length(text, max_chars, message)?;
    }
    Ok(value)
}

fn validated_images(ctx: &ReducerContext, image_ids: Vec<u64>) -> Result<Vec<u64>, String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for image_id in image_ids {
        if seen.insert(image_id) {
            require_image(ctx, image_id)?;
            result.push(image_id);
        }
    }
    Ok(result)
}

fn require_feedback(ctx: &ReducerContext, feedback_id: u64) -> Result<Feedback, String> {
    let customer_id = current_customer_id(ctx).ok_or("当前用户未选择租户")?;
    ctx.db
        .feedback()
        .id()
        .find(feedback_id)
        .filter(|row| row.customer_id == customer_id)
        .ok_or("反馈记录不存在".into())
}

#[spacetimedb::reducer]
pub fn create_feedback(ctx: &ReducerContext, input: FeedbackInput) -> Result<(), String> {
    let user_id = current_user_id(ctx).ok_or("当前身份未绑定用户")?;
    let user = require_user(ctx, user_id)?;
    if user.status != 1 {
        return Err("用户已被禁用".into());
    }
    let center_user_id = current_center_user_id(ctx).ok_or("当前身份未绑定中心用户")?;
    let category = validate_category(input.category)?;
    let content = validate_content(input.content)?;
    let contact = validate_optional(input.contact, 50, "联系方式不能超过 50 个字符")?;
    let client_platform =
        validate_optional(input.client_platform, 20, "客户端平台不能超过 20 个字符")?;
    let user_agent = validate_optional(input.user_agent, 500, "用户代理不能超过 500 个字符")?;
    let image_ids = validated_images(ctx, input.image_ids)?;

    let feedback = ctx.db.feedback().insert(Feedback {
        id: 0,
        customer_id: user.customer_id.clone(),
        category,
        content,
        contact,
        client_platform,
        source: "profile".into(),
        user_agent,
        user_id: user.id,
        username: user.username,
        real_name: user.real_name,
        center_user_id,
        created_at: ctx.timestamp,
        updated_at: None,
    });
    for (sort, img_id) in image_ids.into_iter().enumerate() {
        ctx.db.image_binding().insert(ImageBinding {
            id: 0,
            customer_id: feedback.customer_id.clone(),
            biz_type: FEEDBACK_BIZ_TYPE.into(),
            biz_id: feedback.id,
            img_id,
            field_key: FEEDBACK_IMAGE_FIELD.into(),
            sort: sort as i32,
            created_at: ctx.timestamp,
            updated_at: None,
        });
    }
    Ok(())
}

#[spacetimedb::reducer]
pub fn delete_feedback(ctx: &ReducerContext, feedback_id: u64) -> Result<(), String> {
    AdminContext::require(ctx)?;
    require_feedback(ctx, feedback_id)?;
    let links = ctx
        .db
        .image_binding()
        .image_binding_by_business()
        .filter((FEEDBACK_BIZ_TYPE, feedback_id))
        .collect::<Vec<_>>();
    for link in links {
        ctx.db.image_binding().id().delete(link.id);
    }
    ctx.db.feedback().id().delete(feedback_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 反馈类型仅允许约定值() {
        assert!(validate_category("feature".into()).is_ok());
        assert!(validate_category("unknown".into()).is_err());
    }

    #[test]
    fn 反馈正文长度使用字符数校验() {
        assert!(validate_content("这是十个中文字符的反馈内容".into()).is_ok());
        assert!(validate_content("太短".into()).is_err());
    }
}
