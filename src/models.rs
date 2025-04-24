use chrono::DateTime;
use serde::{Deserialize, Serialize};
use worker::Date;

mod date_serde {
    use super::*;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(date: &Option<Date>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match date {
            Some(date) => {
                let timestamp = date.as_millis();
                serializer.serialize_u64(timestamp)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Date>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let timestamp = Option::<u64>::deserialize(deserializer)?;
        match timestamp {
            Some(ts) => {
                let date_string = format!("{:?}", js_sys::Date::new(&(ts as f64).into()));
                match DateTime::parse_from_str(&date_string, "%Y-%m-%dT%H:%M:%S%.fZ") {
                    Ok(date) => Ok(Some(date.into())),
                    Err(err) => Err(serde::de::Error::custom(format!("Invalid date: {:?}", err))),
                }
            }
            None => Ok(None),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub value: String,
    #[serde(with = "date_serde", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<Date>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheTag {
    pub tag: String,
    pub cache_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheRequest {
    pub key: String,
    pub value: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<String>,
    pub ttl: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
