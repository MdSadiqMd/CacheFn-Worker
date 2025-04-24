use chrono::{DateTime, Utc};
use serde_json::json;
use worker::{Date, Request, Response, Result};

pub fn current_time() -> Date {
    return Date::now();
}

pub fn future_time(millis: u64) -> DateTime<Utc> {
    let now = Utc::now();
    let future = now + chrono::Duration::milliseconds(millis as i64);
    return future;
}

pub fn json_response(status: u16, data: serde_json::Value) -> Result<Response> {
    return Ok(Response::from_json(&data)?
        .with_status(status)
        .with_headers(cors_header()));
}

pub fn success_response(data: Option<serde_json::Value>) -> Result<Response> {
    let resp = json!({
        "success": true,
        "data": data
    });
    return json_response(200, resp);
}

pub fn error_response(status: u16, message: &str) -> Result<Response> {
    let resp = json!({
        "success": false,
        "message": message
    });
    return json_response(status, resp);
}

pub fn cors_header() -> worker::Headers {
    let mut headers = worker::Headers::new();
    let _ = headers.set("Access-Control-Allow-Origin", "*");
    let _ = headers.set("Access-Control-Allow-Methods", "GET, POST");
    let _ = headers.set("Access-Control-Allow-Headers", "Content-Type");
    return headers;
}

pub fn is_authorized(req: &Request, api_key: &str) -> bool {
    if let Some(auth) = req.headers().get("Authorization").unwrap_or(None) {
        if auth.starts_with("Bearer ") {
            let token = auth.trim_start_matches("Bearer ");
            return token == api_key;
        }
    }
    return false;
}
