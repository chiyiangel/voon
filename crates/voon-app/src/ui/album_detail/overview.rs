use super::*;

impl AlbumDetail {
    pub(super) fn overview(
        &mut self,
        album: &Album,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let mut body = div()
            .flex()
            .flex_col()
            .child(metadata(album, theme, self.metadata_open))
            .child(
                div()
                    .id("toggle-album-metadata")
                    .debug_selector(|| "toggle-album-metadata".into())
                    .cursor_pointer()
                    .px_4()
                    .py_3()
                    .text_color(rgb(theme.accent))
                    .child(if self.metadata_open {
                        "⌄ 收起完整资料"
                    } else {
                        "› 更多专辑信息"
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.metadata_open = !this.metadata_open;
                        cx.notify();
                    })),
            );
        body = body.child(
            div()
                .id("toggle-metadata-sources")
                .debug_selector(|| "toggle-metadata-sources".into())
                .cursor_pointer()
                .px_4()
                .py_2()
                .text_color(rgb(theme.accent))
                .child(if self.sources_open {
                    "收起字段来源"
                } else {
                    "比较字段来源"
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.sources_open = !this.sources_open;
                    cx.notify();
                })),
        );
        if self.sources_open {
            match self.state.read(cx).album_metadata_comparison() {
                Ok(comparison) => {
                    body = body.child(crate::ui::metadata_sources::render(&comparison, theme))
                }
                Err(_) => {
                    body = body.child(div().p_4().child("无法读取本地字段来源，请重新打开专辑"))
                }
            }
        }

        let state = self.state.read(cx);
        let tags = state
            .album_user_tags(&album.path)
            .iter()
            .map(|tag| tag.name.as_str())
            .collect::<Vec<_>>()
            .join(" · ");
        let collections = state
            .album_manual_collections(&album.path)
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>()
            .join(" · ");
        body.child(
            div()
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("我的整理"))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(theme.secondary))
                        .child(if tags.is_empty() {
                            "尚未添加标签".into()
                        } else {
                            tags
                        }),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(rgb(theme.secondary))
                        .child(if collections.is_empty() {
                            "尚未加入手动合集".into()
                        } else {
                            format!("合集 · {collections}")
                        }),
                ),
        )
        .into_any_element()
    }
}

fn metadata(album: &Album, theme: Theme, expanded: bool) -> impl IntoElement {
    let fields = album.metadata.entries();
    let mut section = div()
        .flex_none()
        .p_4()
        .border_b_1()
        .border_color(rgb(theme.line))
        .flex()
        .flex_col()
        .gap_3()
        .child(div().font_weight(FontWeight::SEMIBOLD).child("专辑信息"));
    for &(name, label) in FIELD_LABELS {
        if !expanded && !["composer", "performers", "year", "catalog_number"].contains(&name) {
            continue;
        }
        let field = fields.iter().find(|(field, _, _)| *field == name);
        let row = div()
            .flex()
            .items_start()
            .gap_2()
            .text_size(px(11.5))
            .child(
                div()
                    .w(px(66.))
                    .flex_none()
                    .text_color(rgb(theme.secondary))
                    .child(label),
            )
            .child(
                div().flex_1().min_w_0().child(
                    field
                        .map(|(_, value, _)| value.clone())
                        .unwrap_or_else(|| "未填写".into()),
                ),
            );
        section = section.child(row);
    }
    if let Some(collection) = &album.collection {
        section = section.child(
            div()
                .text_size(px(11.5))
                .text_color(rgb(theme.secondary))
                .child(format!("所属合集 · {}", collection.display())),
        );
    }
    section
}
