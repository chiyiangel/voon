//! Native application and window menus keep settings in one place.

use super::{CloseCurrentWindow, OpenSettings, Quit};
use gpui::{App, KeyBinding, Menu, MenuItem, SystemMenuType, actions};

actions!(
    voon,
    [HideApplication, HideOtherApplications, ShowAllApplications]
);

pub(super) fn install(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-h", HideApplication, None),
        KeyBinding::new("cmd-alt-h", HideOtherApplications, None),
    ]);
    cx.on_action(|_: &HideApplication, cx| cx.hide());
    cx.on_action(|_: &HideOtherApplications, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAllApplications, cx| cx.unhide_other_apps());
    cx.set_menus(vec![
        Menu {
            name: "Voon".into(),
            items: vec![
                MenuItem::action("设置…", OpenSettings),
                MenuItem::separator(),
                MenuItem::os_submenu("服务", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("隐藏 Voon", HideApplication),
                MenuItem::action("隐藏其他", HideOtherApplications),
                MenuItem::action("显示全部", ShowAllApplications),
                MenuItem::separator(),
                MenuItem::action("退出 Voon", Quit),
            ],
        },
        Menu {
            name: "窗口".into(),
            items: vec![MenuItem::action("关闭窗口", CloseCurrentWindow)],
        },
    ]);
}
