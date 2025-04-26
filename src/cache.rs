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
            .bind(&[key.into(), current_time().as_millis().into()])
            .await?;

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

        let tx = self.db.begin().await?;
        tx.exec("DELETE FROM cache_entries WHERE key = ?", &[&key])
            .await?;
        if let Some(exp) = expires_at {
            tx.exec(
                "INSERT INTO cache_entries (key, value, expires_at) VALUES (?, ?, ?)",
                &[&key, &value, &exp.timestamp_millis().to_string()],
            )
            .await?;
        } else {
            tx.exec(
                "INSERT INTO cache_entries (key, value, expires_at) VALUES (?, ?, NULL)",
                &[&key, &value],
            )
            .await?;
        }

        for tag in tags {
            tx.exec(
                "INSERT INTO cache_tags (tag, cache_key) VALUES (?, ?)",
                &[&tag, &key],
            )
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn invalidate_tags(&self, tags: Vec<String>) -> Result<()> {
        if tags.is_empty() {
            return Ok(());
        }

        let tx = self.db.begin().await?;
        let placeholders = (0..tags.len()).map(|_| "?").collect::<Vec<_>>().join(",");

        let query = format!(
            "DELETE FROM cache_entries WHERE key IN (
                SELECT DISTINCT cache_key FROM cache_tags WHERE tag IN ({})
            )",
            placeholders
        );
        let params: Vec<_> = tags.iter().map(|t| t.as_str().into()).collect();

        tx.exec(&query, &params).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn clean_expired(&self) -> Result<()> {
        self.db
            .exec(
                "DELETE FROM cache_entries WHERE expires_at IS NOT NULL AND expires_at <= ?",
                &[current_time().as_millis()],
            )
            .await?;
        Ok(())
    }
}
