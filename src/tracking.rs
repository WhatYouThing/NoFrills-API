use std::collections::HashMap;

use actix_web::body::BoxBody;
use serde_json::json;
use tokio::sync::MutexGuard;

use crate::limiter::{self, Limit};

pub async fn get_usage_json() -> BoxBody {
    let map = limiter::get().await;
    let json = json!({
        "pricing": get_usage(&map, "pricing").await
    });
    return BoxBody::new(json.to_string());
}

async fn get_usage(map: &MutexGuard<'static, HashMap<String, Vec<Limit>>>, key: &str) -> usize {
    if map.contains_key(key) {
        return map.get(key).unwrap().len();
    }
    return 0;
}

pub async fn add_usage(key: &str) {
    let mut map = limiter::get().await;
    if !map.contains_key(key) {
        map.insert(key.to_owned(), Vec::new());
    }
    map.get_mut(key).unwrap().push(Limit::new(3600000));
}
