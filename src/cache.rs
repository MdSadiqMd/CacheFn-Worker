use serde_json::Value;
use worker::{D1Database, Result};

use crate::{
    models::CacheRequest,
    utils::{current_time, future_time},
};

pub struct CacheStorage {
    db: D1Database,
}

impl CacheStorage {
    pub fn new(db: D1Database) -> Self {
        Self { db }
    }

    pub async fn setup(&self) -> Result<()> {
        self.db
            .exec(
                "CREATE TABLE IF NOT EXISTS cache_entries (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL,
                    expires_at INTEGER
                )",
            )
            .await?;

        self.db
            .exec(
                "CREATE TABLE IF NOT EXISTS cache_tags (
                    tag TEXT NOT NULL,
                    cache_key TEXT NOT NULL,
                    PRIMARY KEY (tag, cache_key),
                    FOREIGN KEY (cache_key) REFERENCES cache_entries(key) ON DELETE CASCADE
                )",
            )
            .await?;

        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<Value>> {
        self.clean_expired().await?;

        let stmt = self.db
            .prepare("SELECT value FROM cache_entries WHERE key = ? AND (expires_at IS NULL OR expires_at > ?)")
            .bind(&[key.into(), current_time().as_millis().into()])?;

        let result: Option<String> = stmt.first(None).await?;
        match result {
            Some(value) => Ok(Some(serde_json::from_str(&value)?)),
            None => Ok(None),
        }
    }

    pub async fn set(&self, req: CacheRequest) -> Result<()> {
        let key = req.key;
        let value = serde_json::to_string(&req.value)?;
        let expires_at = req.ttl.map(future_time);
        let tags = req.tags;

        self.db
            .prepare("DELETE FROM cache_entries WHERE key = ?")
            .bind(&[key.clone().into()])?
            .run()
            .await?;

        if let Some(exp) = expires_at {
            self.db
                .prepare("INSERT INTO cache_entries (key, value, expires_at) VALUES (?, ?, ?)")
                .bind(&[
                    key.clone().into(),
                    value.into(),
                    exp.timestamp_millis().to_string().into(),
                ])?
                .run()
                .await?;
        } else {
            self.db
                .prepare("INSERT INTO cache_entries (key, value, expires_at) VALUES (?, ?, NULL)")
                .bind(&[key.clone().into(), value.into()])?
                .run()
                .await?;
        }

        for tag in tags {
            self.db
                .prepare("INSERT INTO cache_tags (tag, cache_key) VALUES (?, ?)")
                .bind(&[tag.into(), key.clone().into()])?
                .run()
                .await?;
        }

        Ok(())
    }

    pub async fn invalidate_tags(&self, tags: Vec<String>) -> Result<()> {
        if tags.is_empty() {
            return Ok(());
        }

        let placeholders = (0..tags.len()).map(|_| "?").collect::<Vec<_>>().join(",");

        let query = format!(
            "DELETE FROM cache_entries WHERE key IN (
                SELECT DISTINCT cache_key FROM cache_tags WHERE tag IN ({})
            )",
            placeholders
        );

        let mut stmt = self.db.prepare(&query);
        for tag in &tags {
            stmt = match stmt.bind(&[tag.clone().into()]) {
                Ok(stmt) => stmt,
                Err(err) => return Err(err),
            }
        }

        stmt.run().await?;
        Ok(())
    }

    async fn clean_expired(&self) -> Result<()> {
        self.db
            .prepare("DELETE FROM cache_entries WHERE expires_at IS NOT NULL AND expires_at <= ?")
            .bind(&[current_time().as_millis().into()])?
            .run()
            .await?;
        Ok(())
    }
}
