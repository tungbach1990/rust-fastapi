#![allow(non_snake_case)]
#[cfg(feature = "plugin")]
pub mod plugin {
    use libc::c_char;
    use std::ffi::CString;

    // Generic symbol để loader nhận diện không phụ thuộc tên crate
    #[no_mangle]
    pub extern "C" fn feature_name() -> *mut c_char {
        CString::new("cors").unwrap().into_raw()
    }

    // Giữ nguyên symbol dành riêng cho cors để tương thích ngược
    #[no_mangle]
    pub extern "C" fn feature_name_cors() -> *mut c_char {
        let s = CString::new("cors").unwrap();
        s.into_raw()
    }

    // Manifest mô tả UI cấu hình cho Admin (chuẩn hóa theo các feature khác)
    // - Cho phép bật/tắt CORS theo từng route bằng danh sách enabled_routes
    // - Cấu hình mặc định đặt ở các khóa phẳng: origins, methods, headers, expose_headers, allow_credentials, max_age
    fn manifest_json() -> String {
        r#"{
            "name": "cors",
            "settings": [
              {"key": "enabled_routes", "type": "route_list", "label": "Routes áp dụng CORS", "default": []},
              {"key": "origins", "type": "string_list", "label": "Default Origins", "default": ["*"]},
              {"key": "methods", "type": "string_list", "label": "Default Methods", "default": ["GET","POST","PUT","DELETE"]},
              {"key": "headers", "type": "string_list", "label": "Default Headers", "default": ["*"]},
              {"key": "expose_headers", "type": "string_list", "label": "Default Expose Headers", "default": []},
              {"key": "allow_credentials", "type": "number", "label": "Allow Credentials (0/1)", "default": 0},
              {"key": "max_age", "type": "number", "label": "Max Age (seconds)", "default": 0}
            ]
        }"#.to_string()
    }

    // Generic manifest symbol
    #[no_mangle]
    pub extern "C" fn feature_manifest() -> *mut c_char {
        CString::new(manifest_json()).unwrap().into_raw()
    }

    // Symbol dành riêng cho cors để tương thích ngược
    #[no_mangle]
    pub extern "C" fn feature_manifest_cors() -> *mut c_char {
        CString::new(manifest_json()).unwrap().into_raw()
    }
}

// Không cần logic runtime trong crate này; phần áp dụng layer sẽ xử lý ở app/router.rs