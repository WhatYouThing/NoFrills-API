use std::{collections::HashMap, ops::Add, sync::LazyLock, time::Duration};

use actix_web::body::BoxBody;
use serde_json::json;
use tokio::{
    sync::{Mutex, MutexGuard},
    task,
    time::sleep,
};

static USERS: LazyLock<Mutex<HashMap<String, i64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

async fn get() -> MutexGuard<'static, HashMap<String, i64>> {
    return USERS.lock().await;
}

pub async fn get_usage_json() -> BoxBody {
    let map = get().await;
    let json = json!({
        "pricing": get_count(&map, "pricing").await
    });
    return BoxBody::new(json.to_string());
}

pub async fn get_count(map: &MutexGuard<'static, HashMap<String, i64>>, key: &str) -> i64 {
    if map.contains_key(key) {
        return *map.get(key).unwrap();
    }
    return 0;
}

pub async fn add_count(key: &str) {
    let mut map = get().await;
    if !map.contains_key(key) {
        map.insert(key.to_owned(), 1);
    } else {
        let value = map.get(key).unwrap().add(1);
        &map.insert(key.to_owned(), value);
    }
}

pub fn init() {
    task::spawn(async {
        let duration = Duration::from_millis(3600000);
        loop {
            get().await.clear();
            sleep(duration).await;
        }
    });
}
