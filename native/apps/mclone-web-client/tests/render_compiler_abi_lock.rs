//! 067 Stage 5: single-source lock for the render-compile SharedArrayBuffer ring ABI.
//!
//! The JS side authors the constants once in `www/mclone-render-compiler-abi.js` (imported by
//! the render-compile worker producer and the app/smoke consumers). The Rust main-wasm reader
//! keeps its own copy in `src/web_canvas.rs` because it cannot import the JS file at compile
//! time. This host test parses the numeric `RENDER_COMPILER_*` constants out of *both* files
//! and asserts they agree, so any drift between the producer/consumer ABI and the Rust reader
//! is a failing test instead of a silent SAB corruption (e.g. a wrong status-word index that
//! makes a compile never complete or decode garbage).
//!
//! It lives in `tests/` (a host-compiled integration crate) on purpose: `web_canvas.rs` is
//! `#[cfg(target_arch = "wasm32")]`-gated, so a `#[cfg(test)]` unit test placed inside it would
//! never run under `cargo test` on the host.

const RUST_SRC: &str = include_str!("../src/web_canvas.rs");
const JS_ABI: &str = include_str!("../www/mclone-render-compiler-abi.js");

/// The numeric SAB-ring constants that exist on both sides and must agree. The
/// `RENDER_COMPILER_TRANSPORT_KIND` string is a JS-only diagnostic label with no Rust `const`,
/// so it is asserted separately below rather than parsed as an integer.
const ABI_NAMES: &[&str] = &[
    "RENDER_COMPILER_SHARED_RESULT_CONTROL_WORDS",
    "RENDER_COMPILER_SHARED_RESULT_STATUS_INDEX",
    "RENDER_COMPILER_SHARED_RESULT_BYTES_INDEX",
    "RENDER_COMPILER_SHARED_RESULT_CAPACITY_INDEX",
    "RENDER_COMPILER_SHARED_RESULT_PENDING",
    "RENDER_COMPILER_SHARED_RESULT_COMPLETE",
    "RENDER_COMPILER_SHARED_RESULT_OVERFLOW",
    "RENDER_COMPILER_SHARED_RESULT_FAILED",
    "RENDER_COMPILER_SHARED_INPUT_CONTROL_WORDS",
    "RENDER_COMPILER_SHARED_INPUT_STATUS_INDEX",
    "RENDER_COMPILER_SHARED_INPUT_BYTES_INDEX",
    "RENDER_COMPILER_SHARED_INPUT_CAPACITY_INDEX",
    "RENDER_COMPILER_SHARED_INPUT_READY",
    "RENDER_COMPILER_DEFAULT_SHARED_RESULT_CAPACITY",
    "RENDER_COMPILER_DEFAULT_SHARED_INPUT_CAPACITY",
];

/// Find a `const`/`export const` declaration of exactly `name` in `src` and evaluate its
/// right-hand side. Panics (loudly) if the declaration is missing or not an integer product,
/// so a renamed/removed constant is a test failure rather than a silent pass.
fn const_value(src: &str, name: &str, what: &str) -> i64 {
    let rhs = src
        .lines()
        .find_map(|line| {
            let line = line.trim();
            // Accept both the Rust (`const NAME: T = V;`) and JS (`export const NAME = V;`) forms.
            let after = line
                .strip_prefix("const ")
                .or_else(|| line.strip_prefix("export const "))?;
            let rest = after.strip_prefix(name)?;
            // The char immediately after the name must end the identifier, so a name that is a
            // strict prefix of a longer constant does not match by accident.
            match rest.chars().next()? {
                ':' | ' ' | '=' => {}
                _ => return None,
            }
            let value = line.split_once('=')?.1.trim().trim_end_matches(';').trim();
            Some(value.to_string())
        })
        .unwrap_or_else(|| panic!("{what}: missing `const {name}`"));
    // Values are either a plain integer or a `A * B * C` product (e.g. 16 * 1024 * 1024).
    rhs.split('*')
        .map(|term| {
            term.trim().parse::<i64>().unwrap_or_else(|_| {
                panic!(
                    "{what}: non-integer term `{}` in `{name}` (rhs `{rhs}`)",
                    term.trim()
                )
            })
        })
        .product()
}

#[test]
fn js_and_rust_render_compiler_abi_agree() {
    for name in ABI_NAMES {
        let rust = const_value(RUST_SRC, name, "src/web_canvas.rs");
        let js = const_value(JS_ABI, name, "www/mclone-render-compiler-abi.js");
        assert_eq!(
            rust, js,
            "render-compile ABI drift for `{name}`: web_canvas.rs has {rust}, \
             mclone-render-compiler-abi.js has {js}"
        );
    }
}

#[test]
fn js_abi_declares_the_shared_result_buffer_transport_kind() {
    // The one non-numeric ABI value: the transport-kind diagnostic label the worker reports and
    // the app/smoke metrics default to. Rust references the same string literal directly.
    assert!(
        JS_ABI.contains(r#"export const RENDER_COMPILER_TRANSPORT_KIND = "shared-result-buffer";"#),
        "mclone-render-compiler-abi.js must export RENDER_COMPILER_TRANSPORT_KIND = \"shared-result-buffer\""
    );
}
