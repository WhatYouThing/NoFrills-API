use std::collections::HashMap;
use std::env;
use std::sync::LazyLock;

use actix_web::HttpRequest;
use tokio::sync::{Mutex, MutexGuard};

use crate::util;

static RATE_LIMITS: LazyLock<Mutex<HashMap<String, Vec<Limit>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct Limit {
    time: u128,
    ttl: u128,
}

impl Limit {
    pub fn new(ttl: u128) -> Self {
        return Limit {
            time: util::get_timestamp(),
            ttl: ttl,
        };
    }
}

pub async fn get() -> MutexGuard<'static, HashMap<String, Vec<Limit>>> {
    let mut map = RATE_LIMITS.lock().await;
    let timestamp = util::get_timestamp();
    map.retain(|_key, value| {
        value.retain(|limit| timestamp - limit.time < limit.ttl);
        return value.len() > 0;
    });
    return map;
}

pub fn get_ip(request: HttpRequest) -> String {
    if env::var("NF_API_CLOUDFLARE").is_ok_and(|var| var.eq("true")) {
        return request
            .headers()
            .get("cf-connecting-ip")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
    } else {
        return request
            .connection_info()
            .peer_addr()
            .or(Some("unknown"))
            .unwrap()
            .to_string();
    }
}

pub async fn new_key(endpoint: &str, request: HttpRequest) -> String {
    let key = get_ip(request);
    let mut map = get().await;
    if !map.contains_key(&key) {
        map.insert(key.to_owned(), Vec::new());
    }
    return format!("{}+{}", endpoint, key);
}

// adding authentication to your API to stop "freeloaders" while freeloading the NEU api yourself is some high tier projection
pub async fn is_limited(key: &String, ttl: u128, limit: usize) -> bool {
    let mut map = get().await;
    if map.contains_key(key) {
        if map.get(key).unwrap().len() >= limit {
            return true;
        }
    } else {
        map.insert(key.to_owned(), Vec::new());
    }
    map.get_mut(key).unwrap().push(Limit::new(ttl));
    return false;
}
