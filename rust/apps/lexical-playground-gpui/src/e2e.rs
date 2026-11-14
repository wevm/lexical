//! An end-to-end run for CI, as wevm/zero's zbugs-gpui has: built with the
//! `e2e` feature and run with `LEXICAL_PLAYGROUND_E2E=<file>`, the app
//! drives its own window once it's up, the way macOS does — text through
//! the window's input handler (typing and an input method's composition)
//! and keys through their bindings, undo and redo and the markdown
//! shortcuts included — checks the
//! document after each step,
//! writes the session to `<file>` as a vector, and exits 0, or 1 with the
//! step that failed. CI then replays the file in JS Lexical
//! (`packages/lexical-conformance/src/replay.test.ts`), which compares
//! every step's state and selection with JS's.
//!
//! A screenshot of the window at the end goes to
//! `LEXICAL_PLAYGROUND_E2E_SHOT`, when it's set.

use crate::app::PlaygroundApp;
use crate::editor::*;
use gpui_kit::*;
use std::time::{Duration, Instant};

type Step = Result<(), String>;

pub fn run(window: AnyWindowHandle, app: Entity<PlaygroundApp>, cx: &mut App) {
    let Some(path) = std::env::var_os("LEXICAL_PLAYGROUND_E2E") else {
        return;
    };
    cx.spawn(async move |cx| {
        let result = drive(window, &app, cx).await.and_then(|()| {
            let file = app.read_with(cx, |app, cx| {
                let playground = &app.editor().read(cx).surface.playground;
                playground.export_vector_file("gpui end-to-end run")
            });
            std::fs::write(&path, file).map_err(|e| format!("can't write the session: {e}"))
        });
        if let Some(shot) = std::env::var_os("LEXICAL_PLAYGROUND_E2E_SHOT") {
            let _ = std::process::Command::new("screencapture")
                .arg("-x")
                .arg(shot)
                .status();
        }
        match result {
            Ok(()) => {
                eprintln!("e2e: passed");
                std::process::exit(0);
            }
            Err(error) => {
                eprintln!("e2e: {error}");
                std::process::exit(1);
            }
        }
    })
    .detach();
}

async fn drive(window: AnyWindowHandle, app: &Entity<PlaygroundApp>, cx: &mut AsyncApp) -> Step {
    let editor = app.read_with(cx, |app, _| app.editor().clone());
    // Keys move the caret through the last paint's layout.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !editor.read_with(cx, |e, _| e.is_laid_out()) {
        if Instant::now() > deadline {
            return Err("timed out waiting for the first paint".into());
        }
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
    }

    let line = |cx: &mut AsyncApp, n: usize| -> Result<String, String> {
        let text = editor.read_with(cx, |e, _| e.surface.display_text());
        Ok(text.split('\n').nth(n).unwrap_or_default().to_owned())
    };
    // One step: a key (its action) or input, then the check.
    macro_rules! step {
        ($what:expr, $input:expr, $check:expr) => {{
            cx.update_window(window, |_, window, cx| $input(window, cx))
                .map_err(|e| e.to_string())?;
            // Actions run when the window next handles events.
            cx.background_executor()
                .timer(Duration::from_millis(30))
                .await;
            if let Some(error) = editor.read_with(cx, |e, _| e.error.clone()) {
                return Err(format!("{}: the edit failed: {error}", $what));
            }
            let check: Step = $check(&mut *cx);
            check.map_err(|e| format!("{}: {e}", $what))?;
        }};
    }
    let key = |action: Box<dyn Action>| {
        move |window: &mut Window, cx: &mut App| window.dispatch_action(action, cx)
    };
    let typed = |s: &'static str| {
        let editor = editor.clone();
        move |window: &mut Window, cx: &mut App| {
            editor.update(cx, |e, cx| e.replace_text_in_range(None, s, window, cx))
        }
    };
    let expect = |n: usize, want: &'static str| {
        move |cx: &mut AsyncApp| -> Step {
            let got = line(cx, n)?;
            if got == want {
                Ok(())
            } else {
                Err(format!("line {n} is {got:?}, not {want:?}"))
            }
        }
    };
    let bold = |want: bool| {
        let editor = editor.clone();
        move |cx: &mut AsyncApp| -> Step {
            let got = editor.read_with(cx, |e, _| e.surface.playground.has_format("bold"));
            if got == want {
                Ok(())
            } else {
                Err(format!("bold is {got}, not {want}"))
            }
        }
    };

    const FIRST: &str = "Hello from Rust. This document lives in a lexical-editor editor.";
    step!("End of the line", key(Box::new(LineEnd)), expect(0, FIRST));
    step!("Return", key(Box::new(InsertParagraph)), expect(1, ""));
    step!("⌘B", key(Box::new(Bold)), bold(true));
    step!(
        "Typing bold",
        typed("Typed in GPUI"),
        expect(1, "Typed in GPUI")
    );
    step!("⌘B again", key(Box::new(Bold)), bold(false));
    step!(
        "Typing plain",
        typed(", then plain wordy"),
        expect(1, "Typed in GPUI, then plain wordy")
    );
    step!(
        "⌥⌫",
        key(Box::new(DeleteWordBackward)),
        expect(1, "Typed in GPUI, then plain ")
    );
    step!(
        "Composing",
        {
            let editor = editor.clone();
            move |window: &mut Window, cx: &mut App| {
                editor.update(cx, |e, cx| {
                    e.replace_and_mark_text_in_range(None, "にほ", None, window, cx)
                })
            }
        },
        expect(1, "Typed in GPUI, then plain にほ")
    );
    step!(
        "Committing the composition",
        typed("日本"),
        expect(1, "Typed in GPUI, then plain 日本")
    );
    step!(
        "Typing an emoji",
        typed("👍🏽"),
        expect(1, "Typed in GPUI, then plain 日本👍🏽")
    );
    step!(
        "⌫ over the emoji",
        key(Box::new(Backspace)),
        expect(1, "Typed in GPUI, then plain 日本")
    );
    step!("⇧Return", key(Box::new(InsertLineBreak)), expect(2, ""));
    step!("Typing after the break", typed("next"), expect(2, "next"));
    step!("⌘Z", key(Box::new(Undo)), expect(2, ""));
    step!("⌘⇧Z", key(Box::new(Redo)), expect(2, "next"));
    // ⌘B over the end of a word: on, then off. "xt" becomes a bold node
    // after "ne", so the selection's start is both the end of "ne" and the
    // start of "xt"; the second ⌘B keeps Lexical's point (the start of
    // "xt") and unbolds in one step. (The first also gives Lexical the new
    // selection.)
    let next_bold = |want: bool| {
        let editor = editor.clone();
        move |cx: &mut AsyncApp| -> Result<usize, String> {
            let (format, bold, steps) = editor.read_with(cx, |e, _| {
                let p = &e.surface.playground;
                let run = p.document().blocks[1]
                    .runs
                    .iter()
                    .find(|r| r.text.ends_with("xt"))
                    .filter(|r| r.text == if want { "xt" } else { "next" });
                (
                    run.map(|r| r.has_format("bold")),
                    p.has_format("bold"),
                    p.step_count(),
                )
            });
            if format != Some(want) || bold != want {
                return Err(format!(
                    "\"xt\" bold is {format:?}, the toolbar's {bold}, not {want}"
                ));
            }
            Ok(steps)
        }
    };
    step!("⇧←", key(Box::new(SelectLeft)), expect(2, "next"));
    step!(
        "⇧← again",
        key(Box::new(SelectLeft)),
        |cx: &mut AsyncApp| -> Step { next_bold(false)(cx).map(drop) }
    );
    step!(
        "⌘B over it",
        key(Box::new(Bold)),
        |cx: &mut AsyncApp| -> Step { next_bold(true)(cx).map(drop) }
    );
    let bolded = next_bold(true)(cx)?;
    step!(
        "⌘B again",
        key(Box::new(Bold)),
        |cx: &mut AsyncApp| -> Step {
            match next_bold(false)(cx)? - bolded {
                1 => Ok(()),
                n => Err(format!("{n} steps, not 1")),
            }
        }
    );
    // The block menu and Tab, on the paragraph typed above (lines 1 and 2,
    // split by the line break).
    let block_menu = |block_type: &'static str| {
        let editor = editor.clone();
        move |_: &mut Window, cx: &mut App| {
            editor.update(cx, |e, cx| e.edit(cx, |p| p.set_block_type(block_type)))
        }
    };
    let block_is = |want: &'static str, depth: usize| {
        let editor = editor.clone();
        move |cx: &mut AsyncApp| -> Step {
            let (got, got_depth) = editor.read_with(cx, |e, _| {
                let p = &e.surface.playground;
                let depth = p.document().blocks[1].list.as_ref().map_or(0, |l| l.depth);
                (p.document().blocks[1].block_type().to_owned(), depth)
            });
            if (got.as_str(), got_depth) == (want, depth) {
                Ok(())
            } else {
                Err(format!(
                    "the block is {got} at depth {got_depth}, not {want} at {depth}"
                ))
            }
        }
    };
    step!(
        "H1 from the block menu",
        block_menu("h1"),
        block_is("h1", 0)
    );
    step!(
        "• List from the block menu",
        block_menu("bullet"),
        block_is("bullet", 0)
    );
    step!(
        "↑ to the item's first line",
        key(Box::new(Up)),
        expect(2, "next")
    );
    step!(
        "To the line's start",
        key(Box::new(LineStart)),
        expect(1, "Typed in GPUI, then plain 日本")
    );
    step!(
        "Tab nests the item",
        key(Box::new(InsertTab)),
        block_is("bullet", 1)
    );
    step!(
        "⇧Tab brings it back",
        key(Box::new(Outdent)),
        block_is("bullet", 0)
    );
    step!(
        "• List again ends the list",
        block_menu("bullet"),
        block_is("paragraph", 0)
    );
    step!(
        "The third paragraph is untouched",
        |_: &mut Window, _: &mut App| {},
        expect(
            3,
            "Type, delete, press Enter, or select text and make it bold."
        )
    );

    // Markdown shortcuts (`registerMarkdownShortcuts`), typed a key at a
    // time: `## ` before the paragraph's text makes it a heading, `**md**`
    // makes bold text, `- ` a list item, and `` ``` `` then Return a code
    // block.
    let block_at = |i: usize, want: &'static str| {
        let editor = editor.clone();
        move |cx: &mut AsyncApp| -> Step {
            let got = editor.read_with(cx, |e, _| {
                let blocks = &e.surface.playground.document().blocks;
                blocks.get(i).map(|b| match b.kind.as_str() {
                    "code" => "code".to_owned(),
                    _ => b.block_type().to_owned(),
                })
            });
            if got.as_deref() == Some(want) {
                Ok(())
            } else {
                Err(format!("block {i} is {got:?}, not {want}"))
            }
        }
    };
    let nothing = |_: &mut AsyncApp| -> Step { Ok(()) };
    for k in ["#", "#"] {
        step!("Typing ## ", typed(k), nothing);
    }
    step!("Typing ## ", typed(" "), block_at(1, "h2"));
    step!(
        "↓ to the heading's second line",
        key(Box::new(Down)),
        nothing
    );
    step!("To its end", key(Box::new(LineEnd)), expect(2, "next"));
    for k in [" ", "*", "*", "m", "d", "*"] {
        step!("Typing **md**", typed(k), nothing);
    }
    step!("Typing **md**", typed("*"), expect(2, "next md"));
    step!(
        "md is bold",
        |_: &mut Window, _: &mut App| {},
        |cx: &mut AsyncApp| -> Step {
            let bold = editor.read_with(cx, |e, _| {
                e.surface.playground.document().blocks[1]
                    .runs
                    .iter()
                    .any(|r| r.text == "md" && r.has_format("bold"))
            });
            if bold {
                Ok(())
            } else {
                Err("md is not bold".into())
            }
        }
    );
    step!(
        "Return",
        key(Box::new(InsertParagraph)),
        block_at(2, "paragraph")
    );
    for k in ["-", " "] {
        step!("Typing - ", typed(k), nothing);
    }
    step!("Typing an item", typed("item"), block_at(2, "bullet"));
    step!(
        "Return",
        key(Box::new(InsertParagraph)),
        block_at(3, "bullet")
    );
    step!(
        "Return ends the list",
        key(Box::new(InsertParagraph)),
        block_at(3, "paragraph")
    );
    for k in ["`", "`", "`"] {
        step!("Typing ```", typed(k), nothing);
    }
    step!(
        "Return makes a code block",
        key(Box::new(InsertParagraph)),
        block_at(3, "code")
    );
    step!("Typing code", typed("let x = 1;"), block_at(3, "code"));
    Ok(())
}
