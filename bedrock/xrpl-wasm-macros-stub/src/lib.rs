/// Stub implementation of xrpl_wasm_macros.
///
/// Bedrock calling convention (real node):
///   - WASM-exported functions take NO parameters
///   - Each parameter is read from the host via `xrpl_wasm_std::bedrock_function_param`
///     which wraps host_lib::function_param(index, type_code, buf, len)
///   - u32  → STI_UINT32 = 2,  4 bytes LE
///   - u16  → STI_UINT16 = 1,  2 bytes LE
///   - u8   → STI_UINT8  = 16, 1 byte
///   - u64  → STI_UINT64 = 3,  8 bytes LE  (single ABI slot)
///   - i32  → STI_UINT32 = 2,  4 bytes LE  (reinterpreted)
///   - i64  → STI_UINT64 = 3,  8 bytes LE  (reinterpreted)
///   - AccountId ([u8;20]) → injected by host via bedrock_get_sender, 0 ABI slots
///   - Return value is i32 for all exported functions
///
/// Native / test builds keep the original function signatures so unit tests
/// can call functions directly with typed arguments.

extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ItemFn, Pat, PatType, ReturnType};

#[proc_macro_attribute]
pub fn wasm_export(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    generate_wasm_wrapper(func).into()
}

fn generate_wasm_wrapper(func: ItemFn) -> TokenStream2 {
    let fn_name = &func.sig.ident;
    let impl_name = format_ident!("__{}_impl", fn_name);
    let vis = &func.vis;

    // ---- Build the renamed `_impl` function (original sig + body) ----------
    let mut impl_fn = func.clone();
    impl_fn.sig.ident = impl_name.clone();
    impl_fn.attrs.retain(|a| {
        let seg = a.path().segments.last().map(|s| s.ident.to_string());
        !matches!(seg.as_deref(), Some("cfg_attr") | Some("wasm_export"))
    });
    impl_fn.vis = syn::parse_quote!();

    // ---- Classify parameters -----------------------------------------------
    let mut wasm_reads: Vec<TokenStream2> = vec![];
    let mut call_args: Vec<TokenStream2> = vec![];
    let mut abi_idx: usize = 0;

    // Native path: original typed args
    let native_params: Vec<_> = func.sig.inputs.iter().cloned().collect();
    let mut native_call_args: Vec<TokenStream2> = vec![];

    for param in &func.sig.inputs {
        let FnArg::Typed(PatType { pat, ty, .. }) = param else {
            continue;
        };
        let param_name = match pat.as_ref() {
            Pat::Ident(p) => p.ident.clone(),
            _ => panic!("wasm_export: only identifier patterns are supported"),
        };

        let ty_str = quote!(#ty).to_string().replace(' ', "");

        match ty_str.as_str() {
            // AccountId injected by host — not an ABI parameter.
            "AccountId" | "[u8;20]" => {
                call_args.push(quote! {
                    ::xrpl_wasm_std::bedrock_get_sender()
                });
                native_call_args.push(quote! { #param_name });
                // abi_idx unchanged
            }

            "u8" => {
                let idx = abi_idx as i32;
                wasm_reads.push(quote! {
                    let #param_name: u8 = {
                        let mut __buf = [0u8; 1];
                        unsafe { ::xrpl_wasm_std::bedrock_function_param(#idx, 16i32, __buf.as_mut_ptr(), 1usize); }
                        __buf[0]
                    };
                });
                call_args.push(quote! { #param_name });
                native_call_args.push(quote! { #param_name });
                abi_idx += 1;
            }

            "u16" => {
                let idx = abi_idx as i32;
                wasm_reads.push(quote! {
                    let #param_name: u16 = {
                        let mut __buf = [0u8; 2];
                        unsafe { ::xrpl_wasm_std::bedrock_function_param(#idx, 1i32, __buf.as_mut_ptr(), 2usize); }
                        u16::from_le_bytes([__buf[0], __buf[1]])
                    };
                });
                call_args.push(quote! { #param_name });
                native_call_args.push(quote! { #param_name });
                abi_idx += 1;
            }

            "u32" => {
                let idx = abi_idx as i32;
                wasm_reads.push(quote! {
                    let #param_name: u32 = {
                        let mut __buf = [0u8; 4];
                        unsafe { ::xrpl_wasm_std::bedrock_function_param(#idx, 2i32, __buf.as_mut_ptr(), 4usize); }
                        u32::from_le_bytes([__buf[0], __buf[1], __buf[2], __buf[3]])
                    };
                });
                call_args.push(quote! { #param_name });
                native_call_args.push(quote! { #param_name });
                abi_idx += 1;
            }

            "u64" => {
                // STI_UINT64 = 3, 8 bytes LE — single ABI slot.
                let idx = abi_idx as i32;
                wasm_reads.push(quote! {
                    let #param_name: u64 = {
                        let mut __buf = [0u8; 8];
                        unsafe { ::xrpl_wasm_std::bedrock_function_param(#idx, 3i32, __buf.as_mut_ptr(), 8usize); }
                        u64::from_le_bytes([__buf[0], __buf[1], __buf[2], __buf[3], __buf[4], __buf[5], __buf[6], __buf[7]])
                    };
                });
                call_args.push(quote! { #param_name });
                native_call_args.push(quote! { #param_name });
                abi_idx += 1;
            }

            "i32" => {
                let idx = abi_idx as i32;
                wasm_reads.push(quote! {
                    let #param_name: i32 = {
                        let mut __buf = [0u8; 4];
                        unsafe { ::xrpl_wasm_std::bedrock_function_param(#idx, 2i32, __buf.as_mut_ptr(), 4usize); }
                        i32::from_le_bytes([__buf[0], __buf[1], __buf[2], __buf[3]])
                    };
                });
                call_args.push(quote! { #param_name });
                native_call_args.push(quote! { #param_name });
                abi_idx += 1;
            }

            "i64" => {
                let idx = abi_idx as i32;
                wasm_reads.push(quote! {
                    let #param_name: i64 = {
                        let mut __buf = [0u8; 8];
                        unsafe { ::xrpl_wasm_std::bedrock_function_param(#idx, 3i32, __buf.as_mut_ptr(), 8usize); }
                        i64::from_le_bytes([__buf[0], __buf[1], __buf[2], __buf[3], __buf[4], __buf[5], __buf[6], __buf[7]])
                    };
                });
                call_args.push(quote! { #param_name });
                native_call_args.push(quote! { #param_name });
                abi_idx += 1;
            }

            other => panic!("wasm_export: unsupported type `{other}`"),
        }
    }

    // ---- Return type mapping -----------------------------------------------
    let orig_ret = &func.sig.output;
    let ret_cast = match orig_ret {
        ReturnType::Default => quote! {},
        ReturnType::Type(_, ty) => match quote!(#ty).to_string().replace(' ', "").as_str() {
            "i32" => quote! {},
            _ => quote! { as i32 },
        },
    };

    // ---- Emit ---------------------------------------------------------------
    quote! {
        // Renamed implementation — shared between both targets.
        #impl_fn

        // WASM target: no-arg C-ABI exported function; parameters read from
        // the Bedrock host via host_lib::function_param.
        #[cfg(target_arch = "wasm32")]
        #[no_mangle]
        #vis extern "C" fn #fn_name() -> i32 {
            #(#wasm_reads)*
            #impl_name(#(#call_args),*) #ret_cast
        }

        // Native / test target: re-export with original typed signature.
        #[cfg(not(target_arch = "wasm32"))]
        #vis fn #fn_name(#(#native_params),*) #orig_ret {
            #impl_name(#(#native_call_args),*)
        }
    }
}
