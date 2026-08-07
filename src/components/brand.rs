//! 品牌标识：朱砂方印 + 小篆「云」。
//!
//! 侧边栏、登录页、登录恢复屏三处都用它，避免每处各写一份 img 标签后
//! 尺寸和间距各自漂移。图源 `logos/concepts/concept-25.svg`，由
//! `logos/tools/gen_seal.py` 生成，改图请改生成脚本而不是 assets 里的副本。

use dioxus::prelude::*;

pub const LOGO: Asset = asset!("/assets/logo.svg");

/// 品牌标识的尺寸档位。
#[derive(Clone, Copy, PartialEq)]
pub enum LogoSize {
    /// 侧边栏页头用。
    Small,
    /// 登录页与恢复屏用。
    Large,
}

impl LogoSize {
    fn class(self) -> &'static str {
        match self {
            Self::Small => "brand-logo is-small",
            Self::Large => "brand-logo is-large",
        }
    }
}

/// 印章标识。`alt` 留空——它总是紧挨着「云园慧控」字样出现，
/// 读屏软件重复念一遍品牌名反而啰嗦。
#[component]
pub fn BrandLogo(size: LogoSize) -> Element {
    rsx! {
        img { class: size.class(), src: LOGO, alt: "" }
    }
}
