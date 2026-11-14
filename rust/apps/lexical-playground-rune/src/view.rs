//! The window: a toolbar, the editor, and a side panel with the editor's
//! tree, its JSON, and parity with JS Lexical.

use crate::{Panel, State, decorations, on_change, on_key, on_paste};
use lexical_playground_core::{Playground, example, replay_vectors};
use rune::{Color, Cx, El, ScrollSpec, TextStyle, col, row, text, text_input};
use rune_kit::{Button, ButtonVariant, Kit, SegmentedControl};
use std::sync::OnceLock;

fn kit(dark: bool) -> &'static Kit {
    static DARK: OnceLock<Kit> = OnceLock::new();
    static LIGHT: OnceLock<Kit> = OnceLock::new();
    if dark {
        DARK.get_or_init(|| Kit::neutral(true))
    } else {
        LIGHT.get_or_init(|| Kit::neutral(false))
    }
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

pub fn view(s: &State, cx: &mut Cx<State>) -> El {
    let k = kit(cx.system_dark());
    col()
        .w_full()
        .h_full()
        .fill(k.c.page)
        .child(toolbar(s, k, cx.chrome_insets().leading))
        .child(
            row()
                .w_full()
                .grow(1.0)
                .gap(16.0)
                .padding(16.0)
                .child(editor(s, k))
                .child(panel(s, k)),
        )
        .child(status_bar(s, k))
}

/// `leading`: the macOS traffic lights' width, where the toolbar shares the
/// titlebar's band.
fn toolbar(s: &State, k: &Kit, leading: f32) -> El {
    let mut bar = row()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .pl(if leading > 0.0 { leading + 12.0 } else { 10.0 })
        .fill(k.c.bg_base);
    bar = bar.child(text("Lexical").font(k.ty.body_medium).color(k.c.text_base));
    for (name, label) in FORMATS {
        let on = s.playground.has_format(name);
        bar = bar.child(
            Button::new(label)
                .variant(if on {
                    ButtonVariant::Contrast
                } else {
                    ButtonVariant::Ghost
                })
                .help(format!("Format {name}"))
                .on_press(move |s: &mut State| s.toolbar_edit(|p| p.format_text(name)))
                .build(k)
                .key(format!("format-{name}")),
        );
    }
    bar.child(
        Button::new("Select all")
            .variant(ButtonVariant::Ghost)
            .on_press(|s: &mut State| s.toolbar_edit(Playground::select_all))
            .build(k)
            .key("select-all"),
    )
    .child(rune::spacer())
    .child(
        Button::new("Reset")
            .variant(ButtonVariant::Ghost)
            .on_press(|s: &mut State| {
                s.playground = Playground::new(example()).expect("the example loads");
                s.status = None;
                s.sync();
                s.edit.focused = true;
            })
            .build(k)
            .key("reset"),
    )
}

fn editor(s: &State, k: &Kit) -> El {
    // The field's own ink is white, for a dark window; the kit's follows
    // the system's appearance.
    let field = text_input(&s.edit, TextStyle::new(16.0, 400.0))
        .color(k.c.text_base)
        .caret(k.c.text_base, k.selection())
        .key("editor")
        .w_full()
        .grow(1.0)
        .decorations(decorations(s.playground.document()))
        .on_edit(|s: &mut State| &mut s.edit)
        .on_input_key(on_key)
        .on_input_paste(on_paste)
        .on_input_change(on_change);
    col()
        .grow(1.0)
        .h_full()
        .padding(20.0)
        .radius(10.0)
        .fill(k.c.bg_base)
        .border(1.0, k.c.border_base)
        .child(field)
}

fn panel(s: &State, k: &Kit) -> El {
    let tabs = [Panel::Tree, Panel::Json, Panel::Parity];
    let picked = tabs.iter().position(|p| *p == s.panel);
    let content = match s.panel {
        Panel::Tree => mono(&s.playground.document().tree(), k),
        Panel::Json => mono(&s.playground.state_pretty_json(), k),
        Panel::Parity => parity(s, k),
    };
    col()
        .w(420.0)
        .h_full()
        .gap(10.0)
        .child(
            SegmentedControl::new(&["Tree", "JSON", "Parity"], picked)
                .on_pick(move |s: &mut State, i, _| s.panel = tabs[i])
                .build(k),
        )
        .child(
            col()
                .w_full()
                .grow(1.0)
                .padding(12.0)
                .radius(10.0)
                .fill(k.c.bg_base)
                .border(1.0, k.c.border_base)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(content),
        )
}

fn mono(s: &str, k: &Kit) -> El {
    text(s.to_owned())
        .font(k.ty.mono)
        .color(k.c.text_base)
        .selectable(true)
        .wrap()
        .w_full()
}

fn parity(s: &State, k: &Kit) -> El {
    let explain = "The editor here is lexical-editor. Replay runs the conformance \
        vectors that JS Lexical recorded; Export writes this session as a vector \
        that JS replays step by step, in packages/lexical-conformance:\n\n\
        LEXICAL_REPLAY=<file> pnpm exec vitest run src/replay.test.ts";
    let mut out = col()
        .w_full()
        .gap(10.0)
        .child(
            text(explain)
                .font(k.ty.small)
                .color(k.c.text_muted)
                .selectable(true)
                .wrap()
                .w_full(),
        )
        .child(
            row()
                .gap(8.0)
                .child(
                    Button::new("Replay vectors")
                        .on_press(|s: &mut State| s.report = Some(replay()))
                        .build(k)
                        .key("replay"),
                )
                .child(
                    Button::new("Export session")
                        .on_press(|s: &mut State| s.status = Some(export(s)))
                        .build(k)
                        .key("export"),
                ),
        );
    if let Some((report, ok)) = &s.report {
        out = out.child(
            text(if *ok {
                "Every replayed case matches JS."
            } else {
                "Some cases differ from JS."
            })
            .font(k.ty.body_medium)
            .color(if *ok { ok_color() } else { error_color() }),
        );
        out = out.child(mono(report, k));
    }
    out
}

pub fn replay() -> (String, bool) {
    replay_vectors().unwrap_or_else(|e| (e, false))
}

/// Writes the session's vector file; returns what to tell the user.
fn export(s: &State) -> String {
    let path = std::env::var_os("LEXICAL_PLAYGROUND_EXPORT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("lexical-playground-session.json"));
    match std::fs::write(&path, s.playground.export_vector_file("rune playground")) {
        Ok(()) => format!(
            "Exported {} steps to {}",
            s.playground.step_count(),
            path.display()
        ),
        Err(e) => format!("Can't write {}: {e}", path.display()),
    }
}

fn status_bar(s: &State, k: &Kit) -> El {
    let (message, color) = match (&s.status, s.playground.last_error()) {
        (_, Some(error)) => (format!("The last edit failed: {error}"), error_color()),
        (Some(status), None) => (status.clone(), k.c.text_muted),
        (None, None) => (
            format!(
                "lexical-editor · {} steps · {} blocks",
                s.playground.step_count(),
                s.playground.document().blocks.len()
            ),
            k.c.text_muted,
        ),
    };
    row()
        .w_full()
        .px(16.0)
        .py(8.0)
        .child(text(message).font(k.ty.small).color(color).key("status"))
}

fn ok_color() -> Color {
    rune::oklch(0.63, 0.17, 149.0)
}

fn error_color() -> Color {
    rune::oklch(0.58, 0.22, 27.0)
}
