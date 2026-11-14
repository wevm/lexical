//! The window: a toolbar, the editor, and a side panel with the editor's
//! tree, its JSON, and parity with JS Lexical.

use crate::editor::{EditorEvent, LexicalEditor};
use gpui_kit::component::button::*;
use gpui_kit::component::*;
use gpui_kit::*;
use lexical_playground_core::{Playground, example, format_flag, replay_vectors};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Tree,
    Json,
    Parity,
}

const FORMATS: [(&str, &str); 8] = [
    ("bold", "B"),
    ("italic", "I"),
    ("underline", "U"),
    ("strikethrough", "S"),
    ("code", "</>"),
    ("highlight", "Mark"),
    ("subscript", "x₂"),
    ("superscript", "x²"),
];

/// The block menu, as the JS playground's `BlockFormatDropDown`: each
/// button is a `blockType` and its label (the same as the Rune app's).
const BLOCKS: [(&str, &str); 8] = [
    ("paragraph", "Normal"),
    ("h1", "H1"),
    ("h2", "H2"),
    ("h3", "H3"),
    ("quote", "Quote"),
    ("bullet", "• List"),
    ("number", "1. List"),
    ("check", "☑ List"),
];

/// The URL a new link gets, as the JS playground's link button inserts
/// `https://` for the user to edit.
const NEW_LINK_URL: &str = "https://";

pub struct PlaygroundApp {
    editor: Entity<LexicalEditor>,
    panel: Panel,
    report: Option<(String, bool)>,
    status: Option<String>,
    _subscription: Subscription,
    _appearance: Subscription,
}

impl PlaygroundApp {
    #[cfg(feature = "e2e")]
    pub fn editor(&self) -> &Entity<LexicalEditor> {
        &self.editor
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            LexicalEditor::new(
                Playground::new(example()).expect("the example loads"),
                window,
                cx,
            )
        });
        // Light or dark like the Mac, switching live when the system
        // appearance changes, as the Rune app does.
        Theme::sync_system_appearance(Some(window), cx);
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        });
        let subscription = cx.subscribe(&editor, |this, _, _: &EditorEvent, cx| {
            this.status = None;
            cx.notify();
        });
        window.focus(&editor.focus_handle(cx), cx);
        PlaygroundApp {
            editor,
            panel: Panel::Tree,
            report: None,
            status: None,
            _subscription: subscription,
            _appearance: appearance,
        }
    }

    /// Runs a toolbar button's edit, then gives the keyboard back to the
    /// editor, as a toolbar in a web editor keeps the selection.
    fn toolbar_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Playground) -> lexical_playground_core::Outcome,
    ) {
        self.editor.update(cx, |editor, cx| editor.edit(cx, f));
        window.focus(&self.editor.focus_handle(cx), cx);
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let surface = &self.editor.read(cx).surface;
        // The surface's selection, which a click or an arrow key moves
        // before Lexical hears of it at the next edit.
        let format = surface.playground.format_at(surface.shown_selection());
        let mut bar = h_flex()
            .w_full()
            .gap_1()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mr_2()
                    .child("Lexical"),
            );
        for (name, label) in FORMATS {
            let on = format & format_flag(name) != 0;
            let button = Button::new(SharedString::from(format!("format-{name}")))
                .small()
                .label(label)
                .selected(on)
                .tooltip(format!("Format {name}"))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.toolbar_edit(window, cx, |p| p.format_text(name))
                }));
            // A format that's on is filled, as in the Rune app: the theme's
            // selected look barely shows in dark mode.
            bar = bar.child(if on { button.primary() } else { button });
        }
        bar.child(
            Button::new("select-all")
                .small()
                .ghost()
                .label("Select all")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.toolbar_edit(window, cx, Playground::select_all)
                })),
        )
        .child(div().flex_1())
        .child(
            Button::new("reset")
                .small()
                .ghost()
                .label("Reset")
                .on_click(cx.listener(|this, _, window, cx| {
                    let editor = cx.new(|cx| {
                        LexicalEditor::new(Playground::new(example()).unwrap(), window, cx)
                    });
                    this._subscription = cx.subscribe(&editor, |this, _, _: &EditorEvent, cx| {
                        this.status = None;
                        cx.notify();
                    });
                    window.focus(&editor.focus_handle(cx), cx);
                    this.editor = editor;
                    this.status = None;
                    cx.notify();
                })),
        )
    }

    /// The second row: the block menu, the link button, outdent and indent,
    /// and a check item's box.
    fn block_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let surface = &self.editor.read(cx).surface;
        let sel = surface.shown_selection();
        let playground = &surface.playground;
        let current = playground.block_type_at(sel).to_owned();
        let link = playground.link_at(sel).map(str::to_owned);
        let check = playground
            .document()
            .blocks
            .get(sel.anchor.block)
            .and_then(|b| b.list.as_ref())
            .filter(|l| l.list_type == "check")
            .map(|l| l.checked);
        let block = sel.anchor.block;
        let toggle = |id: String, label: &str, on: bool| {
            let button = Button::new(SharedString::from(id))
                .small()
                .label(label.to_owned())
                .selected(on);
            if on { button.primary() } else { button.ghost() }
        };
        let mut bar = h_flex()
            .w_full()
            .gap_1()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border);
        for (block_type, label) in BLOCKS {
            bar = bar.child(
                toggle(format!("block-{block_type}"), label, current == block_type)
                    .tooltip(format!("Format as {label}"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.toolbar_edit(window, cx, |p| p.set_block_type(block_type))
                    })),
            );
        }
        bar = bar
            .child(
                toggle("link".into(), "Link", link.is_some())
                    .tooltip(match &link {
                        Some(url) => format!("Remove the link to {url}"),
                        None => "Insert a link".to_owned(),
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        let surface = &this.editor.read(cx).surface;
                        let remove = surface
                            .playground
                            .link_at(surface.shown_selection())
                            .is_some();
                        this.toolbar_edit(window, cx, |p| {
                            p.toggle_link((!remove).then_some(NEW_LINK_URL))
                        })
                    })),
            )
            .child(
                Button::new("outdent")
                    .small()
                    .ghost()
                    .label("Outdent")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.toolbar_edit(window, cx, |p| p.indent(true))
                    })),
            )
            .child(
                Button::new("indent")
                    .small()
                    .ghost()
                    .label("Indent")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.toolbar_edit(window, cx, |p| p.indent(false))
                    })),
            );
        if let Some(checked) = check {
            bar = bar.child(
                toggle(
                    "check-item".into(),
                    if checked { "☑ Done" } else { "☐ Done" },
                    checked,
                )
                .tooltip("Check or uncheck this item")
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.toolbar_edit(window, cx, |p| p.toggle_checked(block))
                })),
            );
        }
        if let Some(url) = link {
            bar = bar.child(
                div()
                    .ml_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(url),
            );
        }
        bar
    }

    fn panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = |id: &'static str, label: &'static str, panel: Panel, cx: &mut Context<Self>| {
            Button::new(id)
                .small()
                .label(label)
                .selected(self.panel == panel)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.panel = panel;
                    cx.notify();
                }))
        };
        let playground = &self.editor.read(cx).surface.playground;
        let mono = cx.theme().mono_font_family.clone();
        let content: AnyElement = match self.panel {
            Panel::Tree => div()
                .font_family(mono)
                .text_sm()
                .child(playground.document().tree())
                .into_any_element(),
            Panel::Json => div()
                .font_family(mono)
                .text_sm()
                .child(playground.state_pretty_json())
                .into_any_element(),
            Panel::Parity => self.parity(cx).into_any_element(),
        };
        v_flex()
            .w(px(420.))
            .h_full()
            .min_h(px(0.))
            .gap_2()
            .child(
                h_flex()
                    .gap_1()
                    .child(tab("tab-tree", "Tree", Panel::Tree, cx))
                    .child(tab("tab-json", "JSON", Panel::Json, cx))
                    .child(tab("tab-parity", "Parity", Panel::Parity, cx)),
            )
            .child(
                div()
                    .id("panel")
                    .flex_1()
                    .min_h(px(0.))
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .overflow_y_scroll()
                    .child(content),
            )
    }

    fn parity(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mono = cx.theme().mono_font_family.clone();
        let mut out = v_flex()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "The editor here is lexical-editor. Replay runs the conformance vectors \
                     that JS Lexical recorded; Export writes this session as a vector that JS \
                     replays step by step, in packages/lexical-conformance:",
                    ),
            )
            .child(
                div()
                    .font_family(mono.clone())
                    .text_xs()
                    .child("LEXICAL_REPLAY=<file> pnpm exec vitest run src/replay.test.ts"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("replay")
                            .small()
                            .label("Replay vectors")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.report = Some(replay_vectors().unwrap_or_else(|e| (e, false)));
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("export")
                            .small()
                            .label("Export session")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.status =
                                    Some(export(&this.editor.read(cx).surface.playground));
                                cx.notify();
                            })),
                    ),
            );
        if let Some((report, ok)) = &self.report {
            out = out
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(if *ok {
                            cx.theme().success
                        } else {
                            cx.theme().danger
                        })
                        .child(if *ok {
                            "Every replayed case matches JS."
                        } else {
                            "Some cases differ from JS."
                        }),
                )
                .child(div().font_family(mono).text_xs().child(report.clone()));
        }
        out
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let playground = &editor.surface.playground;
        let (message, color) = match (&editor.error, &self.status) {
            (Some(error), _) => (format!("The last edit failed: {error}"), cx.theme().danger),
            (None, Some(status)) => (status.clone(), cx.theme().muted_foreground),
            (None, None) => (
                format!(
                    "lexical-editor · {} steps · {} blocks",
                    playground.step_count(),
                    playground.document().blocks.len()
                ),
                cx.theme().muted_foreground,
            ),
        };
        div()
            .px_4()
            .py_2()
            .text_xs()
            .text_color(color)
            .child(message)
    }
}

/// Writes the session's vector file; returns what to tell the user.
fn export(playground: &Playground) -> String {
    let path = std::env::var_os("LEXICAL_PLAYGROUND_EXPORT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("lexical-playground-session.json"));
    match std::fs::write(&path, playground.export_vector_file("gpui playground")) {
        Ok(()) => format!(
            "Exported {} steps to {}",
            playground.step_count(),
            path.display()
        ),
        Err(e) => format!("Can't write {}: {e}", path.display()),
    }
}

impl Render for PlaygroundApp {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.toolbar(cx))
            .child(self.block_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .w_full()
                    .gap_4()
                    .p_4()
                    .items_start()
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .p_5()
                            .rounded_lg()
                            .border_1()
                            .border_color(cx.theme().border)
                            .text_size(px(16.))
                            .line_height(px(24.))
                            .child(self.editor.clone()),
                    )
                    .child(self.panel(cx)),
            )
            .child(self.status_bar(cx))
    }
}
