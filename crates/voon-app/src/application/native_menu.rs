//! macOS application menus observe the same library as both windows.

use std::path::PathBuf;

use gpui::{App, Entity, Menu, MenuItem, actions};

use super::{CloseCurrentWindow, OpenSettings, Quit, RescanRoot};
use crate::view_model::AppState;

// GPUI 0.2 validates native items by action type. An unhandled action is
// disabled; using RescanRoot for a disabled item would leave it enabled.
actions!(voon, [DisabledMenuItem]);

#[derive(PartialEq, Eq)]
struct MenuState {
    roots: Vec<(PathBuf, bool)>,
    blocked_reason: Option<&'static str>,
}

impl MenuState {
    fn read(state: &AppState) -> Self {
        Self {
            roots: state
                .roots()
                .iter()
                .map(|root| {
                    (
                        root.path.clone(),
                        state.offline_roots().contains(&root.path),
                    )
                })
                .collect(),
            blocked_reason: if state.index_open_problem().is_some() {
                Some("索引不可用，请先新建索引")
            } else if state.root_changes_blocked_reason().is_some() {
                Some("正在扫描，结束后可重新扫描")
            } else {
                None
            },
        }
    }

    fn install(&self, cx: &App) {
        let mut rescans = Vec::new();
        if let Some(reason) = self.blocked_reason {
            rescans.push(MenuItem::action(reason, DisabledMenuItem));
            if !self.roots.is_empty() {
                rescans.push(MenuItem::separator());
            }
        }
        if self.roots.is_empty() && self.blocked_reason.is_none() {
            rescans.push(MenuItem::action(
                "尚未添加音乐目录，请前往设置…",
                DisabledMenuItem,
            ));
        }
        for (root, offline) in &self.roots {
            let label = if *offline {
                format!("{}（未挂载）", root.display())
            } else {
                root.display().to_string()
            };
            rescans.push(if *offline || self.blocked_reason.is_some() {
                MenuItem::action(label, DisabledMenuItem)
            } else {
                MenuItem::action(label, RescanRoot { root: root.clone() })
            });
        }
        cx.set_menus(vec![
            Menu {
                name: "Voon".into(),
                items: vec![
                    MenuItem::action("设置…", OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action("关闭当前窗口", CloseCurrentWindow),
                    MenuItem::separator(),
                    MenuItem::action("退出 Voon", Quit),
                ],
            },
            Menu {
                name: "资料库".into(),
                items: vec![MenuItem::submenu(Menu {
                    name: "重新扫描".into(),
                    items: rescans,
                })],
            },
        ]);
    }
}

pub(super) fn install(state: Entity<AppState>, cx: &mut App) {
    let mut previous = MenuState::read(state.read(cx));
    previous.install(cx);
    cx.observe(&state, move |state, cx| {
        let current = MenuState::read(state.read(cx));
        // Scanning and cover updates also notify the state. Rebuild native
        // menus only when their roots or command availability actually change.
        if current != previous {
            current.install(cx);
            previous = current;
        }
    })
    .detach();
}
