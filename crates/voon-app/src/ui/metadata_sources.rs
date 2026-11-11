use super::{album_detail::FIELD_LABELS, theme::Theme};
use gpui::{FontWeight, IntoElement, div, prelude::*, rgb};
use voon_core::{FieldComparison, FieldSource, SourceStatus, TrackSource};

pub(super) fn render(comparisons: &[FieldComparison], theme: Theme) -> impl IntoElement {
    let mut panel = div().p_4().flex().flex_col().gap_4();
    for field in comparisons
        .iter()
        .filter(|field| ["title", "year"].contains(&field.name.as_str()))
    {
        let label = FIELD_LABELS
            .iter()
            .find(|(name, _)| *name == field.name)
            .map(|(_, label)| *label)
            .unwrap_or(&field.name);
        let value = |value: Option<&voon_core::Field<String>>| {
            value
                .map(|field| field.value.clone())
                .unwrap_or_else(|| "未知".into())
        };
        let mut row = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(label.to_string()),
            )
            .child(
                div()
                    .text_sm()
                    .child(format!("采用值：{}", value(field.effective.as_ref()))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(theme.secondary))
                    .child(format!(
                        "自动值：{} · 人工值：{}",
                        value(field.automatic.as_ref()),
                        field.user_value.as_deref().unwrap_or("未覆盖")
                    )),
            );
        if field.candidates.is_empty() {
            row = row.child(
                div()
                    .text_xs()
                    .text_color(rgb(theme.tertiary))
                    .child("尚无来源候选"),
            );
        }
        for candidate in &field.candidates {
            let source = match candidate.source {
                FieldSource::FolderName => "目录",
                FieldSource::Cue => "分轨表",
                FieldSource::Tag => "标签",
                FieldSource::UserEdit => "人工",
            };
            let status = match &candidate.status {
                SourceStatus::Current => "已确认".to_string(),
                SourceStatus::Unconfirmed { reason, .. } => {
                    format!("保留上次成功资料，尚未重新确认：{reason}")
                }
            };
            let scope = candidate
                .tracks
                .iter()
                .map(|track| match track {
                    TrackSource::File { path } => format!("{}（整文件）", path.display()),
                    TrackSource::Virtual {
                        image, start, end, ..
                    } => format!(
                        "{}（{}–{}）",
                        image.display(),
                        cue_time(*start),
                        end.map(cue_time).unwrap_or_else(|| "末尾".into())
                    ),
                })
                .collect::<Vec<_>>()
                .join("；");
            row = row.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_2()
                    .border_l_1()
                    .border_color(rgb(theme.line))
                    .child(
                        div()
                            .text_sm()
                            .child(format!("{} · {source}", candidate.value)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme.secondary))
                            .child(candidate.source_path.display().to_string()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme.tertiary))
                            .child(format!("{status} · {}", read_time(candidate.observed_at))),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(theme.tertiary))
                            .child(format!("适用范围：{scope}")),
                    ),
            );
        }
        panel = panel.child(row);
    }
    panel
}

fn cue_time(frames: u32) -> String {
    format!(
        "{}:{:02}.{:02}",
        frames / 4500,
        frames / 75 % 60,
        frames % 75
    )
}

fn read_time(observed_at: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let seconds = now.saturating_sub(observed_at).max(0) / 1000;
    if seconds < 60 {
        "刚刚读取".into()
    } else if seconds < 3600 {
        format!("{} 分钟前读取", seconds / 60)
    } else if seconds < 86400 {
        format!("{} 小时前读取", seconds / 3600)
    } else {
        format!("{} 天前读取", seconds / 86400)
    }
}
