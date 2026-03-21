/// Stub implementation of xrpl_wasm_macros.
///
/// Always generates both a native-friendly and a WASM-friendly version of
/// the annotated function, choosing between them via `#[cfg(target_arch)]`
/// IN THE GENERATED CODE (not in the proc-macro itself).  This avoids
/// relying on `CARGO_CFG_TARGET_ARCH` at proc-macro-host runtime, which is
/// the reason the previous stub produced a 375-byte empty WASM.
///
/// Calling convention assumed for Bedrock:
///   AccountId ([u8;20])  →  NOT a WASM param; provided by host import
///                           `bedrock_get_sender(ptr: *mut u8, len: u32)`
///   u128                 →  two consecutive i64: `<name>_lo`, `<name>_hi`
///   u64                  →  i64
///   u32 / u16 / u8       →  i32
///   i64 / i32            →  kept as-is

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

// ---------------------------------------------------------------------------

fn generate_wasm_wrapper(func: ItemFn) -> TokenStream2 {
    let fn_name = &func.sig.ident;
    let impl_name = format_ident!("__{}_impl", fn_name);
    let vis = &func.vis;

    // ---- Build the renamed `_impl` function (original sig + body) ----------
    let mut impl_fn = func.clone();
    impl_fn.sig.ident = impl_name.clone();
    // Don't inherit wasm_export / cfg_attr on the impl copy.
    impl_fn.attrs.retain(|a| {
        let seg = a.path().segments.last().map(|s| s.ident.to_string());
        !matches!(seg.as_deref(), Some("cfg_attr") | Some("wasm_export"))
    });
    // impl function needs no visibility qualifier (called only from wrapper).
    impl_fn.vis = syn::parse_quote!();

    // ---- Classify parameters for the C-ABI wrapper -------------------------
    let mut wasm_params: Vec<TokenStream2> = vec![];
    let mut call_args: Vec<TokenStream2> = vec![];
    // Keep the original (native) sig inputs for the non-wasm forwarder.
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
            // Bedrock injects the signer — not passed as a WASM parameter.
            "AccountId" | "[u8;20]" => {
                call_args.push(quote! {
                    ::xrpl_wasm_std::host::transaction::sender()
                });
                native_call_args.push(quote! { #param_name });
            }

            // u128 → two i64 (lo, hi)
            "u128" => {
                let lo = format_ident!("{}_lo", param_name);
                let hi = format_ident!("{}_hi", param_name);
                wasm_params.push(quote! { #lo: i64 });
                wasm_params.push(quote! { #hi: i64 });
                call_args.push(quote! {
                    ((#lo as u64) as u128) | (((#hi as u64) as u128) << 64)
                });
                native_call_args.push(quote! { #param_name });
            }

            "u64" => {
                wasm_params.push(quote! { #param_name: i64 });
                call_args.push(quote! { #param_name as u64 });
                native_call_args.push(quote! { #param_name });
            }
            "u32" => {
                wasm_params.push(quote! { #param_name: i32 });
                call_args.push(quote! { #param_name as u32 });
                native_call_args.push(quote! { #param_name });
            }
            "u16" => {
                wasm_params.push(quote! { #param_name: i32 });
                call_args.push(quote! { #param_name as u16 });
                native_call_args.push(quote! { #param_name });
            }
            "u8" => {
                wasm_params.push(quote! { #param_name: i32 });
                call_args.push(quote! { #param_name as u8 });
                native_call_args.push(quote! { #param_name });
            }
            "i64" => {
                wasm_params.push(quote! { #param_name: i64 });
                call_args.push(quote! { #param_name });
                native_call_args.push(quote! { #param_name });
            }
            "i32" => {
                wasm_params.push(quote! { #param_name: i32 });
                call_args.push(quote! { #param_name });
                native_call_args.push(quote! { #param_name });
            }
            other => panic!("wasm_export: unsupported type `{other}`"),
        }
    }

    // ---- Return type mapping -----------------------------------------------
    let orig_ret = &func.sig.output;
    let (wasm_ret, ret_cast) = match orig_ret {
        ReturnType::Default => (quote! {}, quote! {}),
        ReturnType::Type(_, ty) => match quote!(#ty).to_string().replace(' ', "").as_str() {
            "u32" | "u16" | "u8" => (quote! { -> i32 }, quote! { as i32 }),
            "u64" => (quote! { -> i64 }, quote! { as i64 }),
            "i32" => (quote! { -> i32 }, quote! {}),
            "i64" => (quote! { -> i64 }, quote! {}),
            other => panic!("wasm_export: unsupported return type `{other}`"),
        },
    };

    // ---- Emit ---------------------------------------------------------------
    quote! {
        // Renamed implementation — shared between both targets.
        #impl_fn

        // WASM target: C-ABI exported function.
        // `#[cfg(target_arch = "wasm32")]` is evaluated by rustc when
        // compiling the CONTRACT (wasm32 target), not by the host proc-macro.
        #[cfg(target_arch = "wasm32")]
        #[no_mangle]
        #vis extern "C" fn #fn_name(#(#wasm_params),*) #wasm_ret {
            #impl_name(#(#call_args),*) #ret_cast
        }

        // Native / test target: re-export with original signature so tests
        // and the adapter crate can call it naturally.
        #[cfg(not(target_arch = "wasm32"))]
        #vis fn #fn_name(#(#native_params),*) #orig_ret {
            #impl_name(#(#native_call_args),*)
        }
    }
}
