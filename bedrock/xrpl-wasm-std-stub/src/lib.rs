//! Stub for xrpl_wasm_std.
//!
//! On wasm32: wires directly to host_lib imports.
//! On native: in-memory stubs so unit tests compile and run without a node.

#[cfg(target_arch = "wasm32")]
extern crate alloc;

// ---------------------------------------------------------------------------
// host_lib foreign-function imports (wasm32 only)
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "host_lib")]
extern "C" {
    /// Read a ContractCall parameter by index.
    /// type_code: STI_UINT8=16, STI_UINT16=1, STI_UINT32=2, STI_UINT64=3
    /// Returns number of bytes written, or negative on error.
    fn function_param(index: i32, type_code: i32, out_buf: *mut u8, out_len: usize) -> i32;

    /// Read a field from the current ContractCall transaction by sfield ID.
    /// e.g. sfContractAccount = (8 << 16) | 27 = 524315
    /// Returns bytes written, or negative on error.
    fn get_tx_field(field_id: i32, buf: *mut u8, buf_len: i32) -> i32;

    /// Write raw data blob to the Contract entry's own Data field.
    /// Used by the official xrpl-wasm-stdlib for state persistence.
    /// Returns 0 on success, negative on error.
    fn update_data(data_ptr: *const u8, data_len: usize) -> i32;

    /// Read a field from the current ledger object (the Contract entry during execution).
    /// field_id: sfield ID (e.g. Data = 458779)
    /// Returns bytes written into buf, or negative on error.
    fn get_current_ledger_obj_field(field_id: i32, buf_ptr: *mut u8, buf_len: usize) -> i32;

    /// Write a named field on the ContractData object owned by `acc`.
    /// val_ptr must be VL-encoded with a 1-byte XRPL type header:
    ///   [type_byte=7, vl_len_byte(s), raw_data...]  for a Blob.
    /// Parsed via STJson::makeValueFromVLWithType on the node side.
    /// Returns bytes written, or negative on error.
    fn set_data_object_field(
        acc_ptr: *const u8, acc_len: i32,
        key_ptr: *const u8, key_len: i32,
        val_ptr: *const u8, val_len: i32,
    ) -> i32;

    /// Read a named field from the ContractData object owned by `acc`.
    /// Returns bytes written into buf (data in VL+type-header form), or negative on error.
    fn get_data_object_field(
        acc_ptr: *const u8, acc_len: i32,
        key_ptr: *const u8, key_len: i32,
        buf_ptr: *mut u8,   buf_len: i32,
    ) -> i32;

    /// Write to the node's trace log.
    fn trace(msg_ptr: *const u8, msg_len: usize, data_ptr: *const u8, data_len: usize, as_hex: i32) -> i32;

    /// Get the 20-byte account that signed the current transaction.
    fn get_sender(out_buf: *mut u8, out_len: usize) -> i32;
}

// ---------------------------------------------------------------------------
// Public wrappers — called by the wasm_export macro and by lib.rs
// ---------------------------------------------------------------------------

/// Read a ContractCall parameter from the host.
/// Called by the wasm_export macro on WASM builds.
#[cfg(target_arch = "wasm32")]
pub unsafe fn bedrock_function_param(index: i32, type_code: i32, out_buf: *mut u8, out_len: usize) {
    function_param(index, type_code, out_buf, out_len);
}

/// Get the 20-byte transaction sender from the host.
#[cfg(target_arch = "wasm32")]
pub fn bedrock_get_sender() -> [u8; 20] {
    let mut buf = [0u8; 20];
    unsafe { get_sender(buf.as_mut_ptr(), 20); }
    buf
}

/// Get the 20-byte pseudo-account of the currently executing contract.
/// Reads sfContractAccount = (8 << 16) | 27 from the current ContractCall tx.
#[cfg(target_arch = "wasm32")]
pub fn bedrock_get_current_account() -> [u8; 20] {
    const SF_CONTRACT_ACCOUNT: i32 = (8 << 16) | 27; // = 524315
    let mut buf = [0u8; 20];
    unsafe { get_tx_field(SF_CONTRACT_ACCOUNT, buf.as_mut_ptr(), 20); }
    buf
}

// ---------------------------------------------------------------------------
// VL + type-header codec helpers (wasm32 only)
//
// Format for set_data_object_field / get_data_object_field:
//   [type_byte=7 (Blob), vl_len_byte(s), raw_data...]
// VL length encoding (XRPL standard):
//   0-192:    1 byte  — the length itself
//   193-12480: 2 bytes — [193 + (n-193)/256, (n-193)%256]
// ---------------------------------------------------------------------------

/// Wrap raw bytes in the XRPL VL+type-header format expected by
/// set_data_object_field (STJson::makeValueFromVLWithType).
#[cfg(target_arch = "wasm32")]
fn encode_blob(data: &[u8]) -> alloc::vec::Vec<u8> {
    let mut out = alloc::vec::Vec::new();
    out.push(7u8); // Blob type
    let n = data.len();
    if n <= 192 {
        out.push(n as u8);
    } else {
        let n2 = n - 193;
        out.push((193 + n2 / 256) as u8);
        out.push((n2 % 256) as u8);
    }
    out.extend_from_slice(data);
    out
}

/// Strip the type+VL header from a buffer returned by get_data_object_field.
/// Returns the raw data bytes on success.
#[cfg(target_arch = "wasm32")]
fn decode_blob(buf: &[u8]) -> Option<&[u8]> {
    if buf.is_empty() { return None; }
    if buf[0] != 7 { return None; } // expecting Blob type
    let rest = &buf[1..];
    if rest.is_empty() { return None; }
    let (len, hdr_len) = if rest[0] <= 192 {
        (rest[0] as usize, 1usize)
    } else if rest.len() >= 2 {
        let n = (rest[0] as usize - 193) * 256 + rest[1] as usize + 193;
        (n, 2usize)
    } else {
        return None;
    };
    let start = 1 + hdr_len;
    if buf.len() < start + len { return None; }
    Some(&buf[start..start + len])
}

/// Write raw bytes directly to the Contract entry's Data field via update_data.
/// No separate ContractData object is created — avoids reserve requirements.
/// Returns 0 on success, negative on error.
#[cfg(target_arch = "wasm32")]
pub fn host_set_data(data: &[u8]) -> i32 {
    unsafe { update_data(data.as_ptr(), data.len()) }
}

/// Read raw bytes from the Contract entry's Data field via get_current_ledger_obj_field.
/// sfield::Data = 458779
/// Returns the bytes, or None if empty/error.
#[cfg(target_arch = "wasm32")]
pub fn host_get_data() -> Option<alloc::vec::Vec<u8>> {
    const SF_DATA: i32 = 458779; // sfield::Data from xrpl-wasm-stdlib
    const MAX: usize = 4096;
    let mut buf = [0u8; MAX];
    let n = unsafe { get_current_ledger_obj_field(SF_DATA, buf.as_mut_ptr(), MAX) };
    if n <= 0 { return None; }
    Some(buf[..n as usize].to_vec())
}

/// Persist raw bytes under a named key on the contract's own ContractData object.
/// Returns 0 on success, negative on error.
#[cfg(target_arch = "wasm32")]
pub fn host_set_field(account: &[u8; 20], key: &str, data: &[u8]) -> i32 {
    let encoded = encode_blob(data);
    unsafe {
        set_data_object_field(
            account.as_ptr(), 20,
            key.as_ptr() as *const u8, key.len() as i32,
            encoded.as_ptr(), encoded.len() as i32,
        )
    }
}

/// Read raw bytes from a named key on the contract's own ContractData object.
/// Returns the decoded bytes, or None on error / field not present.
///
/// get_data_object_field serializes via returnResult which copies the raw
/// STBlob bytes — without the type+VL header used on the write side.
/// We try stripping the header first; if that fails we use the raw bytes.
#[cfg(target_arch = "wasm32")]
pub fn host_get_field(account: &[u8; 20], key: &str) -> Option<alloc::vec::Vec<u8>> {
    const MAX: usize = 16384 + 4; // state + type/VL overhead
    let mut buf = [0u8; MAX];
    let n = unsafe {
        get_data_object_field(
            account.as_ptr(), 20,
            key.as_ptr() as *const u8, key.len() as i32,
            buf.as_mut_ptr(), MAX as i32,
        )
    };
    if n <= 0 { return None; }
    let slice = &buf[..n as usize];
    // Try stripping the type+VL header written by encode_blob.
    // If that fails, assume get_data_object_field returned raw bytes directly.
    if let Some(raw) = decode_blob(slice) {
        Some(raw.to_vec())
    } else {
        Some(slice.to_vec())
    }
}

// ---------------------------------------------------------------------------
// host module — mirrors the API shape the contract's lib.rs expects
// ---------------------------------------------------------------------------

pub mod host {
    pub mod trace {
        pub fn trace(_msg: &str) -> Result<(), ()> {
            #[cfg(target_arch = "wasm32")]
            unsafe {
                let msg = _msg.as_bytes();
                crate::trace(
                    msg.as_ptr(), msg.len(),
                    ::core::ptr::null(), 0usize,
                    0i32,
                );
            }
            Ok(())
        }
    }

    pub mod transaction {
        pub type AccountId = [u8; 20];

        pub fn sender() -> AccountId {
            crate::bedrock_get_sender()
        }
    }

    /// Simple key-value storage for the contract's own persistent data.
    /// On WASM: backed by set_data_object_field / get_data_object_field.
    /// On native: no-op (the manager uses its own thread_local for state).
    pub mod storage {
        #[cfg(target_arch = "wasm32")]
        pub fn set(key: &[u8], val: &[u8]) {
            let encoded = crate::encode_blob(val);
            let account = [0u8; 20]; // C++ uses contractCtx for the keylet
            unsafe {
                crate::set_data_object_field(
                    account.as_ptr(), 20,
                    key.as_ptr(), key.len() as i32,
                    encoded.as_ptr(), encoded.len() as i32,
                );
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        pub fn set(_key: &[u8], _val: &[u8]) {}

        #[cfg(target_arch = "wasm32")]
        pub fn get(key: &[u8]) -> Option<alloc::vec::Vec<u8>> {
            const MAX: usize = 4096;
            let mut buf = [0u8; MAX];
            let account = [0u8; 20];
            let n = unsafe {
                crate::get_data_object_field(
                    account.as_ptr(), 20,
                    key.as_ptr(), key.len() as i32,
                    buf.as_mut_ptr(), MAX as i32,
                )
            };
            if n <= 0 { return None; }
            let slice = &buf[..n as usize];
            if let Some(raw) = crate::decode_blob(slice) {
                Some(raw.to_vec())
            } else {
                Some(slice.to_vec())
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        pub fn get(_key: &[u8]) -> Option<Vec<u8>> { None }
    }

    pub mod contract {
        #[cfg(not(target_arch = "wasm32"))]
        pub fn invoke(_address: &[u8; 20], _function: &str, _params: &[u8]) -> Result<i32, ()> {
            Err(())
        }

        #[cfg(target_arch = "wasm32")]
        pub fn invoke(address: &[u8; 20], function: &str, params: &[u8]) -> Result<i32, ()> {
            #[link(wasm_import_module = "host_lib")]
            extern "C" {
                fn bedrock_invoke(
                    contract_ptr: *const u8, contract_len: u32,
                    fn_ptr: *const u8, fn_len: u32,
                    params_ptr: *const u8, params_len: u32,
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
}

// ---------------------------------------------------------------------------
// Native-only stubs (on WASM the real functions above are used)
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
pub unsafe fn bedrock_function_param(_index: i32, _type_code: i32, _out_buf: *mut u8, _out_len: usize) {
    // No-op on native — the macro generates typed-arg functions instead.
}

#[cfg(not(target_arch = "wasm32"))]
pub fn bedrock_get_sender() -> [u8; 20] {
    [0u8; 20]
}

#[cfg(not(target_arch = "wasm32"))]
pub fn bedrock_get_current_account() -> [u8; 20] {
    [0u8; 20]
}
