/// Stub for xrpl_wasm_std for native/test builds.
///
/// On-chain (WASM) the real crate provides host function bindings.
/// This stub provides no-op equivalents so native tests compile.

pub mod host {
    pub mod trace {
        /// No-op trace on native builds. On-chain this calls the XRPL host.
        pub fn trace(_msg: &str) -> Result<(), ()> {
            Ok(())
        }
    }
}
