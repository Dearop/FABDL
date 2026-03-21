/// Stub implementation of xrpl_wasm_macros for native/test builds.
///
/// On WASM targets deployed via `bedrock build`, the real crate from
/// xrpl-commons/xrpl-wasm provides ABI generation and no_mangle exports.
/// This stub is a passthrough so native `cargo test` compiles cleanly.

extern crate proc_macro;
use proc_macro::TokenStream;

/// No-op passthrough: leaves the annotated function unchanged.
/// The real wasm_export adds `#[no_mangle]` and export metadata.
#[proc_macro_attribute]
pub fn wasm_export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
