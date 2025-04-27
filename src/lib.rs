use cache::CacheStorage;
use utils::{error_response, is_authorized, success_response};
use worker::*;

mod cache;
mod models;
mod utils;

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    Router::new()
        .options("*", |_, _| {
            Response::empty().map(|resp| resp.with_headers(utils::cors_header()))
        })
        .get_async("/get/:key", |req, ctx| async move {
            let api_key = ctx.var("API_KEY")?.to_string();
            if !is_authorized(&req, &api_key) {
                return error_response(401, "Unauthorized");
            }

            let key = match ctx.param("key") {
                Some(k) => k,
                None => return error_response(400, "Missing key parameter"),
            };

            let db = ctx.env.d1("DB")?;
            let cache = CacheStorage::new(db);
            if let Err(e) = cache.setup().await {
                return error_response(500, &format!("Database setup error: {}", e));
            }

            match cache.get(key).await {
                Ok(Some(value)) => success_response(Some(value)),
                Ok(None) => success_response(None),
                Err(e) => error_response(500, &format!("Cache error: {}", e)),
            }
        })
        .run(req, env)
        .await
}
