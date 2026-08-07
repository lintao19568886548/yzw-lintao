//! 菜单的前端展示元数据。

/// 对应 MySQL 业务库中的 `menu_meta` 表。
#[spacetimedb::table(
    accessor = menu_meta,
    index(accessor = menu_meta_by_customer, btree(columns = [customer_id])),
    index(accessor = menu_meta_by_menu, btree(columns = [menu_id]))
)]
pub struct MenuMeta {
    #[primary_key]
    #[auto_inc]
    pub meta_id: u64,
    /// 原 MySQL 依靠物理业务库隔离，这里显式记录租户。
    pub customer_id: String,
    pub menu_id: u64,
    pub title: String,
    pub icon: Option<String>,
    pub order: i32,
    pub active_icon: Option<String>,
    pub active_path: Option<String>,
    pub affix_tab: Option<bool>,
    pub affix_tab_order: Option<i32>,
    pub badge_content: Option<String>,
    pub badge_type: Option<String>,
    pub badge_variants: Option<String>,
    pub hide_children_in_menu: Option<bool>,
    pub hide_in_breadcrumb: Option<bool>,
    pub hide_in_menu: Option<bool>,
    pub hide_in_tab: Option<bool>,
    pub iframe_src: Option<String>,
    pub keep_alive: Option<bool>,
    pub link: Option<String>,
    pub max_num_of_open_tab: Option<i32>,
    pub no_basic_layout: Option<bool>,
    pub open_in_new_window: Option<bool>,
    pub color: Option<String>,
    pub is_app: Option<bool>,
}
