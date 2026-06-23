//! 070 Stage 1: single-source lock for the server-worker SharedArrayBuffer ring ABI.
//!
//! The JS side authors the control-word constants once in `www/mclone-runner-shared-abi.js`
//! (imported by both the integrated-server worker and the worldgen/light job worker). The Rust
//! main-wasm runner keeps its own copy in `src/web_server_worker.rs` because it cannot import
//! the JS file at compile time. This host test parses the numeric `RUNNER_SHARED_*` constants
//! out of *both* files and asserts they agree, so any drift between the worker ABI and the Rust
//! reader is a failing test instead of a silent SAB corruption (e.g. a wrong status code or
//! byte-index that makes a server job silently never complete or decode garbage, in a worker,
//! in the browser).
//!
//! This mirrors the render lane's `tests/render_compiler_abi_lock.rs` (067 Stage 5) and reuses
//! its parser. Parser caveat: `const_value` evaluates integer literals and `A * B` products but
//! NOT const references, so the Rust `RUNNER_SHARED_CONTROL_BYTES` is authored as the literal
//! `16` (4 i32 control words × 4 bytes) rather than a derived `SLOTS * 4`.
//!
//! It lives in `tests/` (a host-compiled integration crate) on purpose: `web_server_worker.rs`
//! is `#[cfg(target_arch = "wasm32")]`-gated, so a `#[cfg(test)]` unit test placed inside it
//! would never run under `cargo test` on the host.

const RUST_SRC: &str = include_str!("../src/web_server_worker.rs");
const JS_ABI: &str = include_str!("../www/mclone-runner-shared-abi.js");

/// The numeric control-word constants that exist on both sides and must agree. The status enum
/// is normalized so PENDING/COMPLETE/FAILED all exist on both sides; the worker-local pool knobs
/// (`MAX_RUNNER_SHARED_POOL_SLOTS`, `DEFAULT_RUNNER_SHARED_RESPONSE_BYTES`) are intentionally
/// *not* locked — each side sizes its own buffer pool independently of the other.
const ABI_NAMES: &[&str] = &[
    "RUNNER_SHARED_CONTROL_BYTES",
    "RUNNER_SHARED_STATUS_INDEX",
    "RUNNER_SHARED_REQUEST_BYTES_INDEX",
    "RUNNER_SHARED_RESPONSE_BYTES_INDEX",
    "RUNNER_SHARED_STATUS_PENDING",
    "RUNNER_SHARED_STATUS_COMPLETE",
    "RUNNER_SHARED_STATUS_FAILED",
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
    // Values are either a plain integer (possibly negative) or a `A * B * C` product.
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
fn js_and_rust_runner_shared_abi_agree() {
    for name in ABI_NAMES {
        let rust = const_value(RUST_SRC, name, "src/web_server_worker.rs");
        let js = const_value(JS_ABI, name, "www/mclone-runner-shared-abi.js");
        assert_eq!(
            rust, js,
            "runner shared ABI drift for `{name}`: web_server_worker.rs has {rust}, \
             mclone-runner-shared-abi.js has {js}"
        );
    }
}
