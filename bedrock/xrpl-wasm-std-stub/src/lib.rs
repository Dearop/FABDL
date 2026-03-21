/// Stub for xrpl_wasm_std for native/test builds.
///
/// On-chain (WASM) the real crate provides host function bindings.
/// This stub provides no-op or in-memory equivalents so native tests compile.

pub mod host {
    pub mod trace {
        /// No-op trace on native builds. On-chain this calls the XRPL host.
        pub fn trace(_msg: &str) -> Result<(), ()> {
            Ok(())
        }
    }

    pub mod storage {
        use std::cell::RefCell;
        use std::collections::BTreeMap;

        thread_local! {
            static STORE: RefCell<BTreeMap<Vec<u8>, Vec<u8>>> = RefCell::new(BTreeMap::new());
        }

        /// Read a value by key. Returns None if not found.
        pub fn get(key: &[u8]) -> Option<Vec<u8>> {
            STORE.with(|s| s.borrow().get(key).cloned())
        }

        /// Write a value. Overwrites any existing value.
        pub fn set(key: &[u8], value: &[u8]) {
            STORE.with(|s| {
                s.borrow_mut().insert(key.to_vec(), value.to_vec());
            });
        }

        /// Clear all storage (used by test helpers to reset state).
        pub fn clear() {
            STORE.with(|s| s.borrow_mut().clear());
        }
    }
}
