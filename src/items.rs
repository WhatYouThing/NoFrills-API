use std::collections::HashSet;
use std::sync::LazyLock;

use actix_web::body::BoxBody;
use serde_json::{Value, json};
use tokio::sync::{Mutex, MutexGuard};

static NON_PLACEABLE: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

pub async fn get() -> MutexGuard<'static, HashSet<String>> {
    return NON_PLACEABLE.lock().await;
}

pub async fn get_attributes_json() -> BoxBody {
    let set = get().await;
    let mut list = Vec::new();
    for perk in set.iter() {
        list.push(perk);
    }
    let json = json!({
        "non_placeable": list
    });
    return BoxBody::new(json.to_string());
}

pub async fn refresh_items(json: &Value) {
    let mut set = get().await;
    set.clear();
    let items = json["items"].as_array().unwrap();
    for item in items {
        let can_place = item["can_place"].as_bool();
        if can_place.is_some() && !can_place.unwrap() {
            let id = item["id"].as_str().unwrap();
            set.insert(id.to_owned());
        }
    }
}
