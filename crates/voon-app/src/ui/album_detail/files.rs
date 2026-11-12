use super::*;

impl AlbumDetail {
    pub(super) fn files(
        &mut self,
        album: &Album,
        offline: bool,
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let (images, loading, image_error, scan_issues) = {
            let state = self.state.read(cx);
            (
                state.attachment_images().to_vec(),
                state.attachment_images_loading(),
                state.attachment_images_error().map(str::to_owned),
                state
                    .album_scan_issues(&album.path)
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        };
        let mut body = div().flex().flex_col().child(
            div()
                .id("toggle-reading-status")
                .debug_selector(|| "toggle-reading-status".into())
                .cursor_pointer()
                .p_4()
                .text_color(rgb(theme.accent))
                .child(if self.reading_open {
                    "⌄ 读取状态"
                } else {
                    "› 读取状态"
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.reading_open = !this.reading_open;
                    cx.notify();
                })),
        );
        if self.reading_open {
            for issue in scan_issues {
                body = body.child(
                    div()
                        .px_4()
                        .py_2()
                        .bg(rgb(theme.well))
                        .text_color(rgb(theme.accent))
                        .child(format!(
                            "{} · {}\n{}",
                            issue.title,
                            issue.path.display(),
                            issue.explanation
                        )),
                );
            }
            let observations = self.state.read(cx).album_tag_observations();
            let mut tag_status = div().flex_none().p_4().flex().flex_col().gap_2().child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("标签读取状态"),
            );
            match observations {
                Ok(observations) => {
                    for (position, (path, observation)) in observations.into_iter().enumerate() {
                        let label = observation.result.label();
                        tag_status = tag_status.child(
                            div()
                                .text_xs()
                                .text_color(rgb(theme.secondary))
                                .child(format!(
                                    "{label} · {}{}",
                                    path.display(),
                                    if observation.last_success.is_some() {
                                        " · 保留上次成功资料，尚未重新确认"
                                    } else {
                                        ""
                                    }
                                )),
                        );
                        if let Some(reason) = observation.result.reason() {
                            tag_status = tag_status.child(div().text_xs().child(reason.to_owned()));
                        }
                        if observation
                            .result
                            .failure_kind()
                            .is_some_and(|kind| kind.retryable())
                            && !offline
                        {
                            let retry_state = self.state.clone();
                            tag_status = tag_status.child(
                                div()
                                    .id(("retry-tag-read", position))
                                    .cursor_pointer()
                                    .text_color(rgb(theme.accent))
                                    .child("在设置中重试标签读取…")
                                    .on_click(move |_, _, cx| {
                                        crate::application::open_settings_page(
                                            retry_state.clone(),
                                            crate::ui::SettingsPage::Scanning,
                                            cx,
                                        );
                                    }),
                            );
                        }
                    }
                }
                Err(_) => tag_status = tag_status.child("无法读取本地标签状态，请重新打开专辑"),
            }
            body = body.child(tag_status);
            if let Some(notice) = self.state.read(cx).album_tag_refresh_notice() {
                body = body.child(
                    div()
                        .px_4()
                        .py_2()
                        .text_xs()
                        .text_color(rgb(theme.secondary))
                        .child(notice),
                );
            }
            if !offline {
                let state = self.state.clone();
                body = body.child(
                    div()
                        .id("force-album-tags")
                        .debug_selector(|| "detail-force-tags".into())
                        .cursor_pointer()
                        .px_4()
                        .py_2()
                        .text_color(rgb(theme.accent))
                        .child("在设置中重读此专辑标签…")
                        .on_click(move |_, _, cx| {
                            crate::application::open_settings_page(
                                state.clone(),
                                crate::ui::SettingsPage::Scanning,
                                cx,
                            );
                        }),
                );
            }
            if let Some(crate::view_model::cover_thumbnails::ThumbnailState::Failed(error)) = album
                .cover
                .as_ref()
                .and_then(|cover| self.state.read(cx).cover_thumbnail_state(cover))
            {
                body = body.child(
                    div()
                        .px_4()
                        .py_2()
                        .text_size(px(11.))
                        .text_color(rgb(theme.secondary))
                        .child(error.clone()),
                );
            }
        }
        let mut attachments = div()
            .id("detail-attachments")
            .debug_selector(|| "detail-attachments".into())
            .flex_none()
            .p_4()
            .border_b_1()
            .border_color(rgb(theme.line))
            .flex()
            .flex_col()
            .gap_2()
            .child(div().font_weight(FontWeight::SEMIBOLD).child("附件图片"));
        if loading {
            attachments = attachments.child(
                div()
                    .text_color(rgb(theme.secondary))
                    .child("正在读取图片列表…"),
            );
        } else if let Some(error) = image_error {
            let state = self.state.clone();
            attachments = attachments
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(theme.secondary))
                        .child(error),
                )
                .child(
                    div()
                        .id("reload-attachment-images")
                        .cursor_pointer()
                        .text_color(rgb(theme.accent))
                        .child("重新载入")
                        .on_click(move |_, _, cx| {
                            state.update(cx, |state, cx| {
                                if let Err(error) = state.reload_attachment_images(cx) {
                                    state.report_error(error, cx);
                                }
                            })
                        }),
                );
        } else if images.is_empty() {
            attachments =
                attachments.child(div().text_color(rgb(theme.tertiary)).child("没有附件图片"));
        } else {
            attachments =
                attachments.children(images.into_iter().enumerate().map(|(position, image)| {
                    let state = self.state.clone();
                    let label = image
                        .strip_prefix(&album.path)
                        .unwrap_or(&image)
                        .display()
                        .to_string();
                    div()
                        .id(("attachment-image", position))
                        .cursor_pointer()
                        .text_size(px(12.))
                        .text_color(rgb(theme.accent))
                        .child(format!("查看 · {label}"))
                        .on_click(move |_, _, cx| {
                            state.update(cx, |state, cx| {
                                if let Err(error) = state.open_attachment_image(image.clone(), cx) {
                                    state.report_error(error, cx);
                                }
                            })
                        })
                }));
        }
        body = body.child(attachments);
        let mut location = div()
            .flex_none()
            .p_4()
            .flex()
            .flex_col()
            .gap_2()
            .child(div().font_weight(FontWeight::SEMIBOLD).child("位置"))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(theme.secondary))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(
                        album
                            .physical_directories()
                            .into_iter()
                            .map(|path| div().child(path.display().to_string())),
                    ),
            );
        if offline {
            location = location.child(div().text_color(rgb(theme.tertiary)).child("音乐库未挂载"));
        } else {
            let state = self.state.clone();
            let label = if cfg!(target_os = "macos") {
                "在 Finder 中显示"
            } else {
                "在文件管理器中显示"
            };
            location = location.child(
                div()
                    .id("reveal-album")
                    .cursor_pointer()
                    .text_color(rgb(theme.accent))
                    .child(label)
                    .on_click(move |_, _, cx| {
                        state.update(cx, |state, cx| {
                            if let Err(error) = state.reveal_selected_album(cx) {
                                state.report_error(error, cx);
                            }
                        })
                    }),
            );
        }
        body.child(location).into_any_element()
    }
}
