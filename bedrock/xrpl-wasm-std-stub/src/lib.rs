/// Stub for xrpl_wasm_std for native/test builds.
///
/// On-chain (WASM) the real crate provides host function bindings.
/// This stub provides no-op or in-memory equivalents so native tests compile,
/// and declares the Bedrock host imports needed for WASM builds.

pub mod host {
    pub mod trace {
        /// No-op trace on native builds. On-chain this calls the XRPL host.
        pub fn trace(_msg: &str) -> Result<(), ()> {
            Ok(())
        }
    }

    pub mod storage {
        #[cfg(not(target_arch = "wasm32"))]
        use std::cell::RefCell;
        #[cfg(not(target_arch = "wasm32"))]
        use std::collections::BTreeMap;

        #[cfg(not(target_arch = "wasm32"))]
        thread_local! {
            static STORE: RefCell<BTreeMap<Vec<u8>, Vec<u8>>> = RefCell::new(BTreeMap::new());
        }

        /// Read a value by key. Returns None if not found.
        pub fn get(key: &[u8]) -> Option<Vec<u8>> {
            #[cfg(not(target_arch = "wasm32"))]
            {
                STORE.with(|s| s.borrow().get(key).cloned())
            }
            #[cfg(target_arch = "wasm32")]
            {
                // Real host call would go here; stub returns None for safety.
                let _ = key;
                None
            }
        }

        /// Write a value. Overwrites any existing value.
        pub fn set(key: &[u8], value: &[u8]) {
            #[cfg(not(target_arch = "wasm32"))]
            {
                STORE.with(|s| {
                    s.borrow_mut().insert(key.to_vec(), value.to_vec());
                });
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _ = (key, value);
            }
        }

        /// Clear all storage (used by test helpers to reset state).
        pub fn clear() {
            #[cfg(not(target_arch = "wasm32"))]
            STORE.with(|s| s.borrow_mut().clear());
        }
    }

    /// Cross-contract invocation.
    ///
    /// On WASM: calls the Bedrock host import `bedrock_invoke`, which executes
    /// the named function on the target contract within the same transaction.
    /// On native: returns Err(()) — callers supply direct function calls in tests.
    pub mod contract {
        /// Invoke a function on another deployed contract.
        ///
        /// - `address`  : 20-byte contract account id
        /// - `function` : function name (ASCII, no null terminator)
        /// - `params`   : serialised parameter blob (little-endian, matches ABI wire format)
        ///
        /// Returns the raw result bytes on success, or Err(()) on failure.
        #[cfg(not(target_arch = "wasm32"))]
        pub fn invoke(_address: &[u8; 20], _function: &str, _params: &[u8]) -> Result<i32, ()> {
            // Native: callers must simulate cross-contract calls via direct in-process calls.
            Err(())
        }

        #[cfg(target_arch = "wasm32")]
        pub fn invoke(address: &[u8; 20], function: &str, params: &[u8]) -> Result<i32, ()> {
            extern "C" {
                /// Bedrock host import: invoke a function on another contract.
                ///
                /// contract_ptr / contract_len : 20-byte account id
                /// fn_ptr / fn_len             : function name UTF-8
                /// params_ptr / params_len     : serialised params blob
                ///
                /// Returns the i32 result of the invoked function, or -1 on error.
                fn bedrock_invoke(
                    contract_ptr: *const u8,
                    contract_len: u32,
                    fn_ptr: *const u8,
                    fn_len: u32,
                    params_ptr: *const u8,
                    params_len: u32,
                ) -> i32;
            }
            let ret = unsafe {
                bedrock_invoke(
                    address.as_ptr(), 20,
                    function.as_ptr(), function.len() as u32,
                    params.as_ptr(), params.len() as u32,
                )
            };
            if ret < 0 { Err(()) } else { Ok(ret) }
        }
    }

    pub mod transaction {
        /// The account ID type: a 20-byte XRPL account hash.
        pub type AccountId = [u8; 20];

        /// Returns the account that signed the current transaction.
        ///
        /// On native builds returns all-zeros; tests can override state
        /// separately. On WASM builds calls the Bedrock host import
        /// `bedrock_get_sender` which writes the 20-byte account into
        /// the provided buffer.
        pub fn sender() -> AccountId {
            #[cfg(not(target_arch = "wasm32"))]
            {
                [0u8; 20]
            }
            #[cfg(target_arch = "wasm32")]
            {
                extern "C" {
                    /// Bedrock host import: writes the signer account (20 bytes)
                    /// into the buffer at `ptr`.
                    fn bedrock_get_sender(ptr: *mut u8, len: u32);
                }
                let mut account = [0u8; 20];
                unsafe { bedrock_get_sender(account.as_mut_ptr(), 20) };
                account
            }
        }
    }
}
