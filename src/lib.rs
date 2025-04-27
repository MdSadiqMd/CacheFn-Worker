use cache::CacheStorage;
use models::CacheRequest;
use serde_json::json;
use utils::{error_response, is_authorized, success_response};
use worker::*;

mod cache;
mod models;
mod utils;

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    Router::new()
        .options_async("/*_path", |_, _| async {
            let headers = utils::cors_header();
            Response::empty().map(|r| r.with_headers(headers))
        })
        .get_async("/", |_, _| async { Response::ok("OK") })
        .get_async("/health", |_req, ctx| async move {
            let environment = ctx
                .var("ENVIRONMENT")
                .map(|env| env.to_string())
                .unwrap_or_else(|_| "production".to_string());

            Response::from_json(&json!({
                "status": {
                    "overall": "healthy",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                },
                "version": {
                    "api": "1.0.0",
                    "environment": environment,
                },
                "dependencies": {
                    "github_api": {
                        "status": "configured",
                        "endpoint": "https://api.github.com"
                    }
                },
                "worker_info": {
                    "datacenter": ctx.var("CF_WORKER_DATACENTER")
                        .map(|d| d.to_string())
                        .unwrap_or_else(|_| "unknown".to_string()),
                    "runtime": "workers",
                }
            }))
        })
        .get_async("/get/:key", |req, ctx| async move {
            let api_key = ctx.var("API_KEY")?.to_string();
            if !is_authorized(&req, &api_key) {
                return error_response(401, "Unauthorized");
            }

            let key = ctx.param("key").ok_or("Missing key parameter")?;
            let db = ctx.env.d1("DB")?;
            let cache = CacheStorage::new(db);

            cache
                .setup()
                .await
                .map_err(|e| format!("DB setup failed: {e}"))?;
            match cache.get(key).await {
                Ok(Some(value)) => success_response(Some(value)),
                Ok(None) => success_response(None),
                Err(e) => error_response(500, &format!("Cache error: {e}")),
            }
        })
        .post_async("/set", |mut req, ctx| async move {
            let api_key = ctx.var("API_KEY")?.to_string();
            if !is_authorized(&req, &api_key) {
                return error_response(401, "Unauthorized");
            }

            let cache_req: CacheRequest = req
                .json()
                .await
                .map_err(|e| format!("Invalid request: {e}"))?;
            let db = ctx.env.d1("DB")?;
            let cache = CacheStorage::new(db);

            cache
                .setup()
                .await
                .map_err(|e| format!("DB setup failed: {e}"))?;
            cache
                .set(cache_req)
                .await
                .map(|_| success_response(None))
                .unwrap_or_else(|e| error_response(500, &format!("Cache error: {e}")))
        })
        .post_async("/invalidate", |mut req, ctx| async move {
            let api_key = ctx.var("API_KEY")?.to_string();
            if !is_authorized(&req, &api_key) {
                return error_response(401, "Unauthorized");
            }

            let tags: Vec<String> = req
                .json()
                .await
                .map_err(|e| format!("Invalid request: {e}"))?;
            let db = ctx.env.d1("DB")?;
            let cache = CacheStorage::new(db);

            cache
                .setup()
                .await
                .map_err(|e| format!("DB setup failed: {e}"))?;
            cache
                .invalidate_tags(tags)
                .await
                .map(|_| success_response(None))
                .unwrap_or_else(|e| error_response(500, &format!("Cache error: {e}")))
        })
        .run(req, env)
        .await
}
