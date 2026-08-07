//! 全站共用的分页条。
//!
//! 组件库的 `Pagination` 只提供无状态的外观件，页码窗口、边界处理和
//! 上一页/下一页的禁用态都要各页面自己算。这里封装成一个受控组件，
//! 传入当前页信号和总条数即可，避免每个列表页重复实现。

use dioxus::prelude::*;

use crate::components::pagination::{
    Pagination, PaginationContent, PaginationEllipsis, PaginationItem, PaginationLink,
    PaginationLinkKind,
};

/// 页码条最多同时显示的页码数量。
const PAGE_WINDOW: usize = 5;

/// 计算页码窗口，保证当前页尽量居中。
///
/// 返回值是闭区间 `[start, end]`；总页数不足一屏时直接返回全部页码。
pub fn page_window(current_page: usize, total_pages: usize) -> (usize, usize) {
    if total_pages <= PAGE_WINDOW {
        return (1, total_pages.max(1));
    }
    let half = PAGE_WINDOW / 2;
    let end = (current_page + half).min(total_pages);
    let start = end.saturating_sub(PAGE_WINDOW - 1).max(1);
    (start, (start + PAGE_WINDOW - 1).min(total_pages))
}

/// 按总条数和每页条数算出总页数，至少为一页。
pub fn total_pages(total_count: usize, page_size: usize) -> usize {
    if page_size == 0 {
        return 1;
    }
    total_count.div_ceil(page_size).max(1)
}

/// 受控分页条。
///
/// `page` 为一基页码信号，组件只负责把它约束在合法范围内并渲染；
/// 数据切片由调用方按 `page` 自行完成。
#[component]
pub fn Pager(page: Signal<usize>, total_pages: usize, total_count: usize) -> Element {
    let total_pages = total_pages.max(1);
    let current_page = page().clamp(1, total_pages);
    let (window_start, window_end) = page_window(current_page, total_pages);
    let at_first = current_page <= 1;
    let at_last = current_page >= total_pages;

    rsx! {
        div { class: "section-header",
            span { class: "hint", "共 {total_count} 条" }
            Pagination {
                PaginationContent {
                    PaginationItem {
                        PaginationLink {
                            data_kind: Some(PaginationLinkKind::Previous),
                            aria_label: "上一页",
                            aria_disabled: at_first,
                            onclick: move |_| {
                                if !at_first {
                                    page.set(current_page - 1);
                                }
                            },
                            "上一页"
                        }
                    }
                    if window_start > 1 {
                        PaginationItem { PaginationEllipsis {} }
                    }
                    for number in window_start..=window_end {
                        PaginationItem { key: "page-{number}",
                            PaginationLink {
                                is_active: number == current_page,
                                onclick: move |_| page.set(number),
                                "{number}"
                            }
                        }
                    }
                    if window_end < total_pages {
                        PaginationItem { PaginationEllipsis {} }
                    }
                    PaginationItem {
                        PaginationLink {
                            data_kind: Some(PaginationLinkKind::Next),
                            aria_label: "下一页",
                            aria_disabled: at_last,
                            onclick: move |_| {
                                if !at_last {
                                    page.set(current_page + 1);
                                }
                            },
                            "下一页"
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 总页数不足一屏时展示全部页码() {
        assert_eq!(page_window(1, 1), (1, 1));
        assert_eq!(page_window(2, 3), (1, 3));
        assert_eq!(page_window(1, PAGE_WINDOW), (1, PAGE_WINDOW));
    }

    #[test]
    fn 页码窗口保持当前页居中并贴合边界() {
        assert_eq!(page_window(1, 20), (1, 5));
        assert_eq!(page_window(10, 20), (8, 12));
        // 接近末页时窗口整体左移，始终维持固定宽度。
        assert_eq!(page_window(20, 20), (16, 20));
    }

    #[test]
    fn 总页数按每页条数向上取整且至少一页() {
        assert_eq!(total_pages(0, 10), 1);
        assert_eq!(total_pages(1, 10), 1);
        assert_eq!(total_pages(10, 10), 1);
        assert_eq!(total_pages(11, 10), 2);
        // 每页条数为零属于调用方错误，退化成单页而不是除零崩溃。
        assert_eq!(total_pages(50, 0), 1);
    }
}
