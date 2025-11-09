use std::os::raw::{c_char, c_uchar};
use axum::{Json, response::{Html, Response, IntoResponse}};
use http::StatusCode;
use axum::body::Bytes;
use serde_json::Value;

pub type RawHandler = unsafe extern "C" fn() -> *mut c_char;
pub type RawHandlerWithBody = unsafe extern "C" fn(*const c_uchar, usize) -> *mut c_char;
pub type RawRoutePath = unsafe extern "C" fn() -> *mut c_char;
// Alias chung cho các symbol trả về chuỗi (vd: content_type)
pub type RawStr = RawRoutePath;

#[derive(Clone, Copy, Default)]
pub struct MethodSet {
    pub get: Option<RawHandler>,
    pub post: Option<RawHandlerWithBody>,
    pub put: Option<RawHandlerWithBody>,
    pub delete: Option<RawHandler>,
}

pub unsafe fn call_no_body(h: RawHandler) -> Response {
    let ptr = h();
    let s = std::ffi::CString::from_raw(ptr);
    let text = s.to_string_lossy().into_owned();
    to_response(text)
}

pub unsafe fn call_with_body(h: RawHandlerWithBody, body: &[u8]) -> Response {
    let ptr = h(body.as_ptr(), body.len());
    let s = std::ffi::CString::from_raw(ptr);
    let text = s.to_string_lossy().into_owned();
    to_response(text)
}

// Async-friendly wrappers: execute plugin handlers in a blocking task to avoid
// blocking the async runtime when plugins perform I/O or heavy CPU.
pub async fn call_no_body_async(h: RawHandler) -> Response {
    tokio::task::spawn_blocking(move || unsafe { call_no_body(h) })
        .await
        .unwrap_or_else(|_| Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Plugin panicked".into()).unwrap())
}

pub async fn call_with_body_async(h: RawHandlerWithBody, body: Bytes) -> Response {
    tokio::task::spawn_blocking(move || unsafe { call_with_body(h, &body) })
        .await
        .unwrap_or_else(|_| Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body("Plugin panicked".into()).unwrap())
}

fn to_response(text: String) -> Response {
    // Unified error channel: "error:<code>:<body>"
    if let Some(rest) = text.strip_prefix("error:") {
        let (code, body) = match rest.split_once(':') {
            Some((c, b)) => (c.trim(), b),
            None => ("500", rest),
        };
        let status = code.parse::<u16>().ok().and_then(|c| StatusCode::from_u16(c).ok()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let trimmed = body.trim_start();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
            let mut resp = Json(v).into_response();
            *resp.status_mut() = status;
            return resp;
        } else {
            return Response::builder()
                .status(status)
                .header("Content-Type", "text/plain; charset=utf-8")
                .body(body.to_string().into()).unwrap();
        }
    }

    // Status override: "status:<code>:<payload>"; payload can be any of prefixes below
    if let Some(rest) = text.strip_prefix("status:") {
        let (code, payload) = match rest.split_once(':') {
            Some((c, b)) => (c.trim(), b.to_string()),
            None => ("200", String::new()),
        };
        let status = code.parse::<u16>().ok().and_then(|c| StatusCode::from_u16(c).ok()).unwrap_or(StatusCode::OK);
        return to_response_with_status(payload, status);
    }

    // Normal content mapping (default 200)
    to_response_with_status(text, StatusCode::OK)
}

fn to_response_with_status(text: String, status: StatusCode) -> Response {
    if let Some(rest) = text.strip_prefix("html:") {
        let mut resp = Html(rest.to_string()).into_response();
        *resp.status_mut() = status;
        return resp;
    } else if let Some(rest) = text.strip_prefix("text:") {
        return Response::builder().status(status).header("Content-Type", "text/plain; charset=utf-8").body(rest.to_string().into()).unwrap();
    } else if let Some(rest) = text.strip_prefix("js:") {
        return Response::builder().status(status).header("Content-Type", "application/javascript; charset=utf-8").body(rest.to_string().into()).unwrap();
    } else if let Some(rest) = text.strip_prefix("css:") {
        return Response::builder().status(status).header("Content-Type", "text/css; charset=utf-8").body(rest.to_string().into()).unwrap();
    } else if let Some(rest) = text.strip_prefix("xml:") {
        return Response::builder().status(status).header("Content-Type", "application/xml; charset=utf-8").body(rest.to_string().into()).unwrap();
    } else if let Some(rest) = text.strip_prefix("json:") {
        let v: Value = serde_json::from_str(rest).unwrap_or(Value::Null);
        let mut resp = Json(v).into_response();
        *resp.status_mut() = status;
        return resp;
    } else {
        // Backward-compatible: auto-detect JSON by first char, else HTML
        let trimmed = text.trim_start();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
            let mut resp = Json(v).into_response();
            *resp.status_mut() = status;
            return resp;
        } else {
            let mut resp = Html(text).into_response();
            *resp.status_mut() = status;
            return resp;
        }
    }
}
