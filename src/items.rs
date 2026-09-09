use std::collections::HashSet;
use std::sync::LazyLock;

use actix_web::body::BoxBody;
use serde_json::{Value, json};
use tokio::sync::{Mutex, MutexGuard};

static NON_PLACEABLE: LazyLock<Mutex<HashSet<String>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

static MUSEUM_DATA: LazyLock<Mutex<Value>> =
    LazyLock::new(|| Mutex::new(json!({})));

pub async fn get_non_placeable() -> MutexGuard<'static, HashSet<String>> {
    return NON_PLACEABLE.lock().await;
}

pub async fn get_museum_data() -> MutexGuard<'static, Value> {
    return MUSEUM_DATA.lock().await;
}

pub async fn get_non_placeable_json() -> BoxBody {
    let set = get_non_placeable().await;
    let mut list = Vec::new();
    for perk in set.iter() {
        list.push(perk);
    }
    return BoxBody::new(json!(list).to_string());
}

pub async fn get_museum_data_json() -> BoxBody {
    return BoxBody::new(get_museum_data().await.to_string());
}

pub async fn refresh_items(json: &Value) {
    let mut non_placeable = get_non_placeable().await;
    let mut museum_data = json!({});
    non_placeable.clear();
    let items = json["items"].as_array().unwrap();
    for item in items {
        if let Some(id) = item["id"].as_str() {
            let can_place = item["can_place"].as_bool();
            if can_place.is_some() && !can_place.unwrap() {
                non_placeable.insert(id.to_owned());
            }
            if let Some(museum) = item["museum_data"].as_object() {
                museum_data[id.to_owned()] = json!({
                    "category": museum["category"].as_str().unwrap()
                });
            }
        }
    }
    *get_museum_data().await = museum_data;
}
