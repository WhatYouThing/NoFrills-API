use std::{collections::HashSet, sync::LazyLock};

use actix_web::body::BoxBody;
use serde_json::json;
use tokio::sync::{Mutex, MutexGuard};

use crate::util;

static PERKS: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub async fn get() -> MutexGuard<'static, HashSet<String>> {
    return PERKS.lock().await;
}

pub async fn get_perks_json() -> BoxBody {
    let set = get().await;
    let mut list = Vec::new();
    for perk in set.iter() {
        list.push(perk);
    }
    let json = json!({
        "perks": list
    });
    return BoxBody::new(json.to_string());
}

pub async fn refresh_perks() {
    let req = util::make_request("v2/resources/skyblock/election").await;
    if req.is_err() {
        println!(
            "Panicked while refreshing election data:\n{}",
            req.unwrap_err()
        );
    } else {
        if let Some(json) = util::parse_json(req.unwrap()) {
            let mut set = get().await;
            set.clear();
            for perk in json["mayor"]["perks"].as_array().unwrap() {
                set.insert(perk["name"].as_str().unwrap().to_string());
            }
            if let Some(minister_perk) = json["mayor"]["minister"]["perk"]["name"].as_str() {
                set.insert(minister_perk.to_string());
            }
        }
    }
}
