//! The Lexical playground in GPUI: a document in `lexical-editor`, drawn
//! and edited in a native window. `editor.rs` is the editor; `app.rs`, the
//! window around it.

mod app;
#[cfg(feature = "e2e")]
mod e2e;
mod editor;

use gpui_kit::*;

actions!(lexical_playground, [Quit, Hide, HideOthers, ShowAll]);

/// The macOS app menu, as wevm/zero's zbugs-gpui has it: GPUI has no menu
/// bar of its own.
fn app_menu(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
    ]);
    cx.set_menus([Menu::new("Lexical Playground").items([
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action("Hide Lexical Playground", Hide),
        MenuItem::action("Hide Others", HideOthers),
        MenuItem::action("Show All", ShowAll),
        MenuItem::separator(),
        MenuItem::action("Quit Lexical Playground", Quit),
    ])]);
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            editor::bind_keys(cx);
            app_menu(cx);
            let bounds = Bounds::centered(None, size(px(1100.), px(720.)), cx);
            let (_window, _app) = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Lexical Playground (GPUI)".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| app::PlaygroundApp::new(window, cx)),
            )
            .expect("the window opens");
            cx.activate(true);
            #[cfg(feature = "e2e")]
            e2e::run(_window, _app, cx);
        });
}
