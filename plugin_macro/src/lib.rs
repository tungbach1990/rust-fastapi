use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, ItemFn};

/// Nhận #[get("/api/hello")]
#[proc_macro_attribute]
pub fn get(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_json(attr, item, "get")
}

#[proc_macro_attribute]
pub fn post(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_json(attr, item, "post_bytes")
}

#[proc_macro_attribute]
pub fn put(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_json(attr, item, "put_bytes")
}

#[proc_macro_attribute]
pub fn delete(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_json(attr, item, "delete")
}

/// Sinh code export
fn build_handler_json(attr: TokenStream, item: TokenStream, export_name: &str) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let block = &func.block;
    let inputs = &func.sig.inputs;
    let output = &func.sig.output;

    let args_str = attr.to_string();
    let path_str = if args_str.is_empty() {
        format!("/{}", fn_name)
    } else {
        args_str.trim_matches('"').to_string()
    };

    let export_ident = format_ident!("{}", export_name);

    // Xuất route và handler JSON: hàm trả về serde_json::Value
    // Không cần axum::Json hay ràng buộc Serialize trong chữ ký hàm.
    let gen = quote! {
        const __ROUTE_PATH: &str = #path_str;

        #vis fn #fn_name(#inputs) #output #block

        #[no_mangle]
        pub extern "C" fn #export_ident() -> *mut std::os::raw::c_char {
            use std::ffi::CString;
            let json_text = std::panic::catch_unwind(|| {
                let result = #fn_name();
                let body = serde_json::to_string(&result)
                    .unwrap_or_else(|_| "{\"error\":\"serialize\"}".into());
                format!("json:{}", body)
            }).unwrap_or_else(|_| "json:{\"error\":\"panic\"}".into());
            CString::new(json_text).unwrap().into_raw()
        }

        #[no_mangle]
        pub extern "C" fn content_type() -> *mut std::os::raw::c_char {
            use std::ffi::CString;
            CString::new("application/json").unwrap().into_raw()
        }

        #[no_mangle]
        pub extern "C" fn route_path() -> *mut std::os::raw::c_char {
            use std::ffi::CString;
            CString::new(__ROUTE_PATH).unwrap().into_raw()
        }
    };

    gen.into()
}

/// Nhận #[get_html("/path")], yêu cầu hàm trả về String (HTML)
#[proc_macro_attribute]
pub fn get_html(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_html(attr, item, "get")
}

#[proc_macro_attribute]
pub fn post_html(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_html(attr, item, "post_bytes")
}

#[proc_macro_attribute]
pub fn put_html(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_html(attr, item, "put_bytes")
}

#[proc_macro_attribute]
pub fn delete_html(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_html(attr, item, "delete")
}

fn build_handler_html(attr: TokenStream, item: TokenStream, export_name: &str) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let block = &func.block;
    let inputs = &func.sig.inputs;
    let output = &func.sig.output;

    let args_str = attr.to_string();
    let path_str = if args_str.is_empty() {
        format!("/{}", fn_name)
    } else {
        args_str.trim_matches('"').to_string()
    };

    let export_ident = format_ident!("{}", export_name);

    let gen = quote! {
        const __ROUTE_PATH: &str = #path_str;

        #vis fn #fn_name(#inputs) #output #block

        #[no_mangle]
        pub extern "C" fn #export_ident() -> *mut std::os::raw::c_char {
            use std::ffi::CString;
            let html_text = std::panic::catch_unwind(|| {
                let result = #fn_name();
                format!("html:{}", result)
            }).unwrap_or_else(|_| "html:<b>panic</b>".into());
            CString::new(html_text).unwrap().into_raw()
        }

        #[no_mangle]
        pub extern "C" fn content_type() -> *mut std::os::raw::c_char {
            use std::ffi::CString;
            CString::new("text/html").unwrap().into_raw()
        }

        #[no_mangle]
        pub extern "C" fn route_path() -> *mut std::os::raw::c_char {
            use std::ffi::CString;
            CString::new(__ROUTE_PATH).unwrap().into_raw()
        }
    };

    gen.into()
}

/// Các macro cho content-type khác: text, js, css, xml
#[proc_macro_attribute]
pub fn get_text(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_textual(attr, item, "get", "text:")
}

#[proc_macro_attribute]
pub fn get_js(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_textual(attr, item, "get", "js:")
}

#[proc_macro_attribute]
pub fn get_css(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_textual(attr, item, "get", "css:")
}

#[proc_macro_attribute]
pub fn get_xml(attr: TokenStream, item: TokenStream) -> TokenStream {
    build_handler_textual(attr, item, "get", "xml:")
}

fn build_handler_textual(attr: TokenStream, item: TokenStream, export_name: &str, prefix: &str) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let fn_name = &func.sig.ident;
    let vis = &func.vis;
    let block = &func.block;
    let inputs = &func.sig.inputs;
    let output = &func.sig.output;

    let args_str = attr.to_string();
    let path_str = if args_str.is_empty() { format!("/{}", fn_name) } else { args_str.trim_matches('"').to_string() };
    let export_ident = format_ident!("{}", export_name);

    let gen = quote! {
        const __ROUTE_PATH: &str = #path_str;

        #vis fn #fn_name(#inputs) #output #block

        #[no_mangle]
        pub extern "C" fn #export_ident() -> *mut std::os::raw::c_char {
            use std::ffi::CString;
            let text = std::panic::catch_unwind(|| {
                let result = #fn_name();
                format!("{}{}", #prefix, result)
            }).unwrap_or_else(|_| format!("{}panic", #prefix));
            CString::new(text).unwrap().into_raw()
        }

        #[no_mangle]
        pub extern "C" fn content_type() -> *mut std::os::raw::c_char {
            use std::ffi::CString;
            let ct = match #prefix {
                "text:" => "text/plain",
                "js:" => "application/javascript",
                "css:" => "text/css",
                "xml:" => "application/xml",
                _ => "text/plain",
            };
            CString::new(ct).unwrap().into_raw()
        }

        #[no_mangle]
        pub extern "C" fn route_path() -> *mut std::os::raw::c_char {
            use std::ffi::CString;
            CString::new(__ROUTE_PATH).unwrap().into_raw()
        }
    };

    gen.into()
}

// ===== New lightweight macros for multi-route modules =====
// These macros define handlers without exporting global symbols.
// They attach helper functions that the export_routes! macro can use to generate unique FFI wrappers.

#[proc_macro_attribute]
pub fn def_html(attr: TokenStream, item: TokenStream) -> TokenStream { def_with_method(attr, item, "get") }
#[proc_macro_attribute]
pub fn def_text(attr: TokenStream, item: TokenStream) -> TokenStream { def_with_method(attr, item, "get") }
#[proc_macro_attribute]
pub fn def_js(attr: TokenStream, item: TokenStream) -> TokenStream { def_with_method(attr, item, "get") }
#[proc_macro_attribute]
pub fn def_css(attr: TokenStream, item: TokenStream) -> TokenStream { def_with_method(attr, item, "get") }
#[proc_macro_attribute]
pub fn def_xml(attr: TokenStream, item: TokenStream) -> TokenStream { def_with_method(attr, item, "get") }
#[proc_macro_attribute]
pub fn def_json(attr: TokenStream, item: TokenStream) -> TokenStream { def_with_method(attr, item, "get") }

#[proc_macro_attribute]
pub fn def_get(attr: TokenStream, item: TokenStream) -> TokenStream { def_with_method(attr, item, "get") }

#[proc_macro_attribute]
pub fn def_post(attr: TokenStream, item: TokenStream) -> TokenStream { def_with_method(attr, item, "post") }

#[proc_macro_attribute]
pub fn def_put(attr: TokenStream, item: TokenStream) -> TokenStream { def_with_method(attr, item, "put") }

#[proc_macro_attribute]
pub fn def_delete(attr: TokenStream, item: TokenStream) -> TokenStream { def_with_method(attr, item, "delete") }

fn def_with_method(attr: TokenStream, item: TokenStream, method: &str) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let fn_name = &func.sig.ident;
    let route_path_fn = format_ident!("{}__route_path", fn_name);
    let method_fn = format_ident!("{}__method", fn_name);
    // Wrapper names
    let get_wrapper_name = format_ident!("get_{}", fn_name);
    let post_wrapper_name = format_ident!("post_{}", fn_name);
    let put_wrapper_name = format_ident!("put_{}", fn_name);
    let delete_wrapper_name = format_ident!("delete_{}", fn_name);
    let vis = &func.vis;
    let block = &func.block;
    let inputs = &func.sig.inputs;
    let output = &func.sig.output;
    let args_str = attr.to_string();
    let path_str = if args_str.is_empty() { format!("/{}", fn_name) } else { args_str.trim_matches('"').to_string() };
    let register_fn_name = format_ident!("__register_{}", fn_name);
    // Generate method-specific wrapper according to provided method
    let wrapper_tokens = match method {
        "get" => quote! {
            #[no_mangle]
            pub extern "C" fn #get_wrapper_name() -> *mut std::os::raw::c_char {
                use std::ffi::CString;
                let out = std::panic::catch_unwind(|| {
                    let result = #fn_name();
                    result.to_string()
                }).unwrap_or_else(|_| "error:500:panic".into());
                CString::new(out).unwrap().into_raw()
            }
        },
        "post" => quote! {
            #[no_mangle]
            pub extern "C" fn #post_wrapper_name(body_ptr: *const u8, body_len: usize) -> *mut std::os::raw::c_char {
                use std::ffi::CString;
                let out = std::panic::catch_unwind(|| {
                    let body_slice = unsafe { std::slice::from_raw_parts(body_ptr, body_len) };
                    let body_str = std::str::from_utf8(body_slice).unwrap_or("");
                    let result = #fn_name(body_str);
                    result.to_string()
                }).unwrap_or_else(|_| "error:500:panic".into());
                CString::new(out).unwrap().into_raw()
            }
        },
        "put" => quote! {
            #[no_mangle]
            pub extern "C" fn #put_wrapper_name(body_ptr: *const u8, body_len: usize) -> *mut std::os::raw::c_char {
                use std::ffi::CString;
                let out = std::panic::catch_unwind(|| {
                    let body_slice = unsafe { std::slice::from_raw_parts(body_ptr, body_len) };
                    let body_str = std::str::from_utf8(body_slice).unwrap_or("");
                    let result = #fn_name(body_str);
                    result.to_string()
                }).unwrap_or_else(|_| "error:500:panic".into());
                CString::new(out).unwrap().into_raw()
            }
        },
        "delete" => quote! {
            #[no_mangle]
            pub extern "C" fn #delete_wrapper_name(body_ptr: *const u8, body_len: usize) -> *mut std::os::raw::c_char {
                use std::ffi::CString;
                let out = std::panic::catch_unwind(|| {
                    let body_slice = unsafe { std::slice::from_raw_parts(body_ptr, body_len) };
                    let body_str = std::str::from_utf8(body_slice).unwrap_or("");
                    let result = #fn_name(body_str);
                    result.to_string()
                }).unwrap_or_else(|_| "error:500:panic".into());
                CString::new(out).unwrap().into_raw()
            }
        },
        _ => quote! {}
    };

    let gen = quote! {
        #vis fn #fn_name(#inputs) #output #block
        pub fn #route_path_fn() -> &'static str { #path_str }
        pub fn #method_fn() -> &'static str { #method }
        #wrapper_tokens

        // Auto-register this route into a module-local registry at library load
        #[ctor::ctor]
        fn #register_fn_name() {
            let method = #method_fn();
            let mut entry = serde_json::json!({
                "path": #route_path_fn(),
                "method": method
            });
            match method {
                "get" => { entry["get"] = serde_json::json!(stringify!(#get_wrapper_name)); },
                "post" => { entry["post_bytes"] = serde_json::json!(stringify!(#post_wrapper_name)); },
                "put" => { entry["put_bytes"] = serde_json::json!(stringify!(#put_wrapper_name)); },
                "delete" => { entry["delete"] = serde_json::json!(stringify!(#delete_wrapper_name)); },
                _ => { entry["get"] = serde_json::json!(stringify!(#get_wrapper_name)); }
            }
            // Push into registry declared by declare_routes!()
            crate::__plugin_routes::__push_route(entry);
        }
    };
    gen.into()
}

// declare_routes! macro: define a registry and export routes_manifest automatically.
#[proc_macro]
pub fn declare_routes(_input: TokenStream) -> TokenStream {
    let gen = quote! {
        #[allow(non_snake_case)]
        mod __plugin_routes {
            use std::sync::Mutex;
            static ROUTES: Mutex<Vec<serde_json::Value>> = Mutex::new(Vec::new());
            pub fn __push_route(v: serde_json::Value) { ROUTES.lock().unwrap().push(v); }
            #[no_mangle]
            pub extern "C" fn routes_manifest() -> *mut std::os::raw::c_char {
                use std::ffi::CString;
                let s = serde_json::to_string(&*ROUTES.lock().unwrap()).unwrap_or("[]".into());
                CString::new(s).unwrap().into_raw()
            }
        }
    };
    gen.into()
}
