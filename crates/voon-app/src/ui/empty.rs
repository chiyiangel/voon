use gpui::{Div, FontWeight, div, prelude::*, px, rgb};

use super::{icons::icon, theme::Theme};
use crate::view_model::{AppState, Navigation};

pub fn albums(state: &AppState, theme: Theme) -> Div {
    let (title, body) = match state.navigation() {
        Navigation::AllAlbums => (
            "添加你的音乐库",
            "选择存放音乐的文件夹。读取标签、分轨表和目录名，建立一份本地索引；音乐库不在线时也能浏览。",
        ),
        Navigation::Collections => ("还没有套装", "添加音乐库后，在这里按目录结构浏览套装。"),
        Navigation::Composers => ("还没有作曲家", "添加音乐库后，在这里浏览作曲家名下的专辑。"),
        Navigation::Facets => (
            "还没有可筛选的专辑",
            "添加音乐库后，在这里按格式、规格、语种和年代筛选。",
        ),
        Navigation::UserTags => ("还没有标签", "在这里用自己的关键词整理专辑。"),
        Navigation::ManualCollections => ("还没有合集", "在这里把喜欢的专辑整理成自己的合集。"),
        Navigation::NeedsAttention => ("没有待处理项目", "需要你核对的专辑会集中显示在这里。"),
    };
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(14.))
        .p(px(40.))
        .child(
            div()
                .size(px(84.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(22.))
                .bg(rgb(theme.accent))
                .child(icon("music", 40., theme.window)),
        )
        .child(
            div()
                .text_size(px(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .max_w(px(420.))
                .text_center()
                .text_size(px(13.))
                .line_height(px(19.))
                .text_color(rgb(theme.secondary))
                .child(body),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(px(11.5))
                .text_color(rgb(theme.secondary))
                .child(icon("eye", 13., theme.secondary))
                .child("只读：不会修改、移动或删除音乐库中的任何文件"),
        )
}

pub fn detail(theme: Theme) -> Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .p(px(28.))
        .child(icon("music", 36., theme.tertiary))
        .child(
            div()
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .child("选择一张专辑"),
        )
        .child(
            div()
                .text_size(px(12.5))
                .text_color(rgb(theme.secondary))
                .child("在这里查看专辑信息和曲目"),
        )
}
