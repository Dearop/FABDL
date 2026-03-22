# Bedrock Contract — Debug Context (pre-compact dump)

## Goal
Fix state persistence between `ContractCall` invocations.
Every call starts with fresh state because the old `update_data` / `get_current_ledger_obj_field` host functions don't work.

---

## Current implementation (as of this session)

### Files changed
- `xrpl-wasm-std-stub/src/lib.rs` — stubs for host_lib imports
- `contract/src/lib.rs` — save/load state hooks + trace calls

### Host functions now imported in WASM (confirmed to exist in node)
```
function_param(index: i32, type_code: i32, out_buf: *mut u8, out_len: usize) -> i32
get_tx_field(field_id: i32, buf: *mut u8, buf_len: i32) -> i32
set_data_object_field(acc_ptr, acc_len: i32, key_ptr, key_len: i32, val_ptr, val_len: i32) -> i32
get_data_object_field(acc_ptr, acc_len: i32, key_ptr, key_len: i32, buf_ptr, buf_len: i32) -> i32
trace(msg_ptr, msg_len, data_ptr, data_len, as_hex: i32) -> i32
get_sender(out_buf: *mut u8, out_len: usize) -> i32
```

### How contract account is obtained
```rust
const SF_CONTRACT_ACCOUNT: i32 = (8 << 16) | 27; // = 524315
get_tx_field(SF_CONTRACT_ACCOUNT, buf.as_mut_ptr(), 20)
```
Source: `WasmVM.cpp` line 92: `WASM_IMPORT_FUNC2(i, getDataObjectField, "get_data_object_field", hfs, 70)`
C++ uses `contractCtx.result.contractAccount` for the keylet.

### Value encoding for set_data_object_field
```
[type_byte=7 (Blob), vl_len_byte(s), raw_bytes...]
```
Write: `encode_blob(data)` → `[0x07, len, data...]`
Read: `get_data_object_field` returns data via `returnResult` — format may differ (see issue below).

---

## Current symptom

`initialize_pool` → Return Code 0 (SUCCESS)
`mint`           → Return Code 8 (PoolNotInitialized) ← state not persisting

No `ContractData` ledger entry is visible via `account_objects` after `initialize_pool`.
The `Contract` ledger entry updates its `PreviousTxnID` but no data field appears.

---

## Trace calls added (latest WASM build)

`save_state_to_host` traces:
- `"save_state: got account"` — always
- `"save_state: account is all-zeros!"` — if get_tx_field returned nothing
- `"save_state: set_data_object_field FAILED"` — if return < 0
- `"save_state: set_data_object_field ok"` — if return >= 0

`load_state_from_host` traces:
- `"load_state: got account"`
- `"load_state: account is all-zeros!"`
- `"load_state: got field data"` / `"load_state: host_get_field returned None"`
- `"load_state: decoded state ok"` / `"load_state: decode_state FAILED"`

---

## How to read the trace output

The node is xrpld running as a local binary:
```
./xrpld -a --conf ./config/xrpld.cfg --ledgerfile ./config/genesis.json
```
PID: 30996
Trace output goes to rippled's debug.log. Find it with:
```bash
cat ./config/xrpld.cfg | grep -i "debug_logfile\|log"
# Then:
tail -f <logfile> | grep -i "trace\|contract\|wasm"
```

After finding the log, call `initialize_pool` and look for the trace lines above.

---

## Hypotheses (in order of likelihood)

### 1. `get_tx_field(524315)` returns 0 bytes / error
If it fails silently, `account = [0u8; 20]`.
`set_data_object_field` with a zero account might fail because `[0u8;20]` isn't on the ledger.
**Trace "save_state: account is all-zeros!" would confirm this.**

**Fix if true:** Try a different field ID.
In XRPL binary encoding, sfield IDs use a different encoding than `(type << 16) | field_code`.
The wire encoding for AccountID type (8), field 27 would be:
- Type >= 16: `0xE1` byte (high nibble 0xE = "type in next byte"), then type byte, then field byte
- Actually: for type_code > 14 AND field_code > 14: 3 bytes total
- For type=8, field=27: type fits in nibble (8 < 16), field > 14 → 2-byte prefix: `0x8E, 0x1B`
- The integer representation: need to check rippled SField.h for exact definition

Alternative field IDs to try:
- `(8 << 8) | 27 = 2075` (if it's type<<8 | field)
- `(8 << 4) | 27 = 155` — unlikely, field > 15
- Check `sfContractAccount` in Bedrock's SField.h directly

### 2. `set_data_object_field` fails due to reserve (0 XRP on contract account)
Contract pseudo-account has 0 XRP + DepositAuth.
Creating a new ContractData ledger entry requires owner reserve.
**Trace "save_state: set_data_object_field FAILED" would confirm this.**

### 3. Read format mismatch
`get_data_object_field` returns raw bytes without the `[0x07, vl_len]` header.
Current code handles this via fallback in `host_get_field` — should be OK.
But if `decode_blob` fails AND the raw bytes happen to fail `decode_state`, state still won't load.

---

## Current WASM exports (ABI — 7 functions, spec compliant)
```
initialize_pool(initial_tick: UINT32, fee_bps: UINT16, protocol_fee_share_bps: UINT16) -> UINT32
mint(lower_tick: UINT32, upper_tick: UINT32, liquidity_delta: UINT64) -> UINT32
burn(lower_tick: UINT32, upper_tick: UINT32, liquidity_delta: UINT64) -> UINT32
collect(lower_tick: UINT32, upper_tick: UINT32, max_amount_0: UINT32, max_amount_1: UINT32) -> UINT32
swap_exact_in(amount_in: UINT32, min_amount_out: UINT32, zero_for_one: UINT8) -> UINT32
set_protocol_fee(protocol_fee_share_bps: UINT16) -> UINT32
set_pause(paused: UINT8) -> UINT32
```
`collect_protocol` is intentionally excluded (8th function would exceed < 8 limit).

---

## Latest deployed contract
- **Pool:** `rGidePnyi3smGXhMhKJqLT4977XYxPYtku`  (has trace calls, latest WASM)
- **Wallet seed:** `snuFTAkDRWxMZdE1APubjQzeS7EcS`
- **Wallet address:** `rKeLZAQ14cbpDHGQaRLvSBjeHNrAm6HU1M`
- **Network:** local `ws://localhost:6006`, HTTP RPC `http://localhost:5005`

---

## Next steps
1. Find xrpld debug.log path (`grep` the config file)
2. Call `initialize_pool` on `rGidePnyi3smGXhMhKJqLT4977XYxPYtku`
3. Read trace output → determine which hypothesis is correct
4. Fix accordingly (most likely fix: correct sfield ID or handle zero-account fallback)
