//! Q4 (the one program, 2026-09-08): the audio callback runs on a real-time thread and must
//! never wait on the game. The engine used to sit behind a `Mutex` the main thread held for the
//! whole control update every frame; the callback waited for it, and nothing but a docstring
//! said it would not. Now the main thread posts a `ControlFrame` into a bounded mailbox and the
//! callback owns the engine — and this test refuses a lock on that path: `audio_out.rs` names no
//! `Mutex`, no `RwLock`, no `.lock(` outside its comments, and the mailbox is `sync_channel`
//! read with `try_recv`.

use std::fs;
use std::path::Path;

fn code_without_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| match line.find("//") {
            Some(cut) => &line[..cut],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_audio_callback_path_holds_no_lock() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("audio_out.rs");
    let source = fs::read_to_string(&path).expect("audio_out.rs");
    let code = code_without_comments(&source);
    for forbidden in ["Mutex", "RwLock", ".lock(", "Condvar", "parking_lot"] {
        assert!(
            !code.contains(forbidden),
            "{}: `{forbidden}` on the audio thread's path — the engine belongs to the callback, the main thread posts frames into the mailbox",
            path.display()
        );
    }
    assert!(
        code.contains("sync_channel"),
        "the mailbox is bounded: a stalled device never grows the game's memory"
    );
    assert!(code.contains("try_recv"), "the callback drains without blocking");
    assert!(code.contains("try_send"), "the main thread posts without blocking");
    // The main thread never reaches the engine: the only engine calls are the frame's `apply`
    // and the callback's `render`.
    assert!(!code.contains("with_engine"), "no shared engine handle for the main thread");
}
