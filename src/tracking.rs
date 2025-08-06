use std::{collections::HashMap, ops::Add, sync::LazyLock, time::Duration};

use tokio::{
    sync::{Mutex, MutexGuard},
    task,
    time::sleep,
};

static USERS: LazyLock<Mutex<HashMap<String, i64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

async fn get() -> MutexGuard<'static, HashMap<String, i64>> {
    return USERS.lock().await;
}

pub async fn get_pricing_count() -> i64 {
    let key = String::from("get-item-pricing");
    let map = get().await;
    if map.contains_key(&key) {
        return *map.get(&key).unwrap();
    }
    return 0;
}

pub async fn add_pricing_count() {
    let key = String::from("get-item-pricing");
    let mut map = get().await;
    if !map.contains_key(&key) {
        map.insert(key, 1);
    } else {
        let value = map.get(&key).unwrap().add(1);
        &map.insert(key, value);
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
