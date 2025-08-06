use std::collections::HashMap;
use std::env;
use std::sync::LazyLock;
use std::time::Duration;

use actix_web::HttpRequest;
use tokio::sync::{Mutex, MutexGuard};
use tokio::task;
use tokio::time::{Instant, sleep};

static RATE_LIMITS: LazyLock<Mutex<HashMap<String, Vec<Instant>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

async fn get() -> MutexGuard<'static, HashMap<String, Vec<Instant>>> {
    return RATE_LIMITS.lock().await;
}

fn get_ip(request: HttpRequest) -> String {
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
pub async fn is_limited(key: &String, ttl: u64, limit: usize) -> bool {
    let delay = Duration::from_millis(ttl);
    let mut map = get().await;
    let now = Instant::now();
    let zero_duration = Duration::from_millis(0);
    if map.contains_key(key) {
        let list = map.get_mut(key).unwrap();
        list.retain(|instant| now.duration_since(*instant) == zero_duration);
        if list.len() >= limit {
            return true;
        }
    } else {
        map.insert(key.to_owned(), Vec::new());
    }
    let instant = Instant::now().checked_add(delay).unwrap();
    map.get_mut(key).unwrap().push(instant);
    return false;
}

pub async fn clear_expired() {
    let mut map = get().await;
    let now = Instant::now();
    let zero_duration = Duration::from_millis(0);
    for key in map.to_owned().keys() {
        let list = map.get_mut(key).unwrap();
        list.retain(|instant| now.duration_since(*instant) == zero_duration);
        if list.len() == 0 {
            map.remove(key.as_str()).unwrap();
        }
    }
}

pub fn init() {
    task::spawn(async {
        let duration = Duration::from_millis(300000);
        loop {
            clear_expired().await;
            sleep(duration).await;
        }
    });
}
