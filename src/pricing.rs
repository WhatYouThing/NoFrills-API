use std::collections::HashMap;
use std::sync::LazyLock;

use actix_web::body::BoxBody;
use serde_json::{Value, json};
use tokio::sync::{Mutex, MutexGuard};

use crate::util;

static PRICING: LazyLock<Mutex<HashMap<String, Value>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub async fn get() -> MutexGuard<'static, HashMap<String, Value>> {
    return PRICING.lock().await;
}

pub fn get_pricing(map: &MutexGuard<'static, HashMap<String, Value>>, pricing_type: &str) -> Value {
    if map.contains_key(pricing_type) {
        return map.get(pricing_type).unwrap().to_owned();
    }
    return json!({});
}

pub async fn update_pricing(pricing_type: &str, json: Value) {
    get().await.insert(pricing_type.to_owned(), json);
}

pub async fn get_pricing_json() -> BoxBody {
    let map = get().await;
    let json = json!({
        "auction": get_pricing(&map, "auction"),
        "bazaar": get_pricing(&map, "bazaar"),
        "attribute": get_pricing(&map, "attribute"),
        "npc": get_pricing(&map, "npc")
    });
    return BoxBody::new(json.to_string());
}

pub async fn fetch_auctions_list() -> Vec<Value> {
    let mut page = 0;
    let mut max_pages = 50;
    let mut auctions = Vec::new();
    while page <= max_pages {
        let url = format!("v2/skyblock/auctions?page={}", page);
        let req = util::make_request(url.as_str()).await;
        if req.is_err() {
            println!(
                "Panicked while refreshing Auction House data, page {}/{}:\n{}",
                page,
                max_pages,
                req.unwrap_err()
            );
            return Vec::new();
        }
        let json = util::parse_json(req.unwrap());
        let auction_list = json["auctions"].as_array().unwrap();
        auctions.append(&mut auction_list.to_owned());
        max_pages = json["totalPages"].as_i64().unwrap() - 1;
        page += 1;
    }
    return auctions;
}

pub async fn refresh_auction_house() {
    let auctions = fetch_auctions_list().await;
    let mut auction_prices = json!({});
    let mut attribute_prices = json!({}); // soon to be scrapped
    for auction in &auctions {
        if auction["bin"].as_bool().unwrap() {
            let bytes = auction["item_bytes"].as_str().unwrap();
            let nbt = util::parse_item_nbt(bytes).await;
            let tag = nbt.get_compound("tag").unwrap();
            if let Some(extra) = tag.get_compound("ExtraAttributes") {
                let mut item_id = extra.get_string("id").unwrap().to_owned();
                if item_id.eq("PET") {
                    let pet_info = util::parse_json_str(extra.get_string("petInfo").unwrap());
                    item_id = format!(
                        "{}_PET_{}",
                        pet_info["type"].as_str().unwrap(),
                        pet_info["tier"].as_str().unwrap()
                    );
                }
                if item_id.eq("RUNE") {
                    let rune_info = extra
                        .get_compound("runes")
                        .unwrap()
                        .child_tags
                        .first()
                        .unwrap();
                    item_id = format!(
                        "{}_{}_RUNE",
                        rune_info.0,
                        rune_info.1.extract_int().unwrap()
                    );
                }
                let price = auction["starting_bid"].as_f64().unwrap();
                let current_price = auction_prices[&item_id].as_f64();
                auction_prices[&item_id] = json!(if current_price.is_some() {
                    current_price.unwrap().min(price)
                } else {
                    price
                });
                if let Some(attributes) = extra.get_compound("attributes") {
                    let mut list = Vec::new();
                    let tags = &attributes.child_tags;
                    for attribute in tags {
                        list.push(format!(
                            "{}{}",
                            attribute.0,
                            attribute.1.extract_int().unwrap()
                        ));
                    }
                    if tags.len() == 2 {
                        list.push(format!(
                            "{} {}",
                            tags.get(0).unwrap().0,
                            tags.get(1).unwrap().0
                        ));
                    }
                    for id in list {
                        if !attribute_prices[&id].is_object() {
                            attribute_prices[&id] = json!({});
                        }
                        let attribute_current = attribute_prices[&id][&item_id].as_f64();
                        attribute_prices[&id][&item_id] = json!(if attribute_current.is_some() {
                            attribute_current.unwrap().min(price)
                        } else {
                            price
                        });
                    }
                }
            }
        }
    }
    if !auctions.is_empty() {
        update_pricing("auction", auction_prices).await;
        update_pricing("attribute", attribute_prices).await;
    }
}

pub async fn refresh_bazaar() {
    let req = util::make_request("v2/skyblock/bazaar").await;
    if req.is_err() {
        println!(
            "Panicked while refreshing Bazaar data:\n{}",
            req.unwrap_err()
        );
    } else {
        let mut bazaar_prices = json!({});
        let json = util::parse_json(req.unwrap());
        let products = json["products"].as_object().unwrap();
        for (id, data) in products.iter() {
            let buy_summary = data["buy_summary"].as_array().unwrap();
            let sell_summary = data["sell_summary"].as_array().unwrap();
            if !bazaar_prices[id].is_object() {
                bazaar_prices[id] = json!({});
            }
            bazaar_prices[id]["buy"] = json!(if buy_summary.len() > 0 {
                buy_summary.first().unwrap()["pricePerUnit"]
                    .as_f64()
                    .unwrap()
            } else {
                0.0
            });
            bazaar_prices[id]["sell"] = json!(if sell_summary.len() > 0 {
                sell_summary.first().unwrap()["pricePerUnit"]
                    .as_f64()
                    .unwrap()
            } else {
                0.0
            });
        }
        update_pricing("bazaar", bazaar_prices).await;
    }
}

pub async fn refresh_npc() {
    let req = util::make_request("v2/resources/skyblock/items").await;
    if req.is_err() {
        println!("Panicked while refreshing NPC data:\n{}", req.unwrap_err());
    } else {
        let mut npc_prices = json!({});
        let json = util::parse_json(req.unwrap());
        let items = json["items"].as_array().unwrap();
        for item in items {
            let id = item["id"].as_str().unwrap();
            let coins_price = item["npc_sell_price"].as_f64();
            let motes_price = item["motes_sell_price"].as_f64();
            if coins_price.is_some() || motes_price.is_some() {
                npc_prices[id] = json!({});
                if coins_price.is_some() {
                    npc_prices[id]["coin"] = json!(coins_price.unwrap());
                }
                if motes_price.is_some() {
                    npc_prices[id]["mote"] = json!(motes_price.unwrap());
                }
            }
        }
        update_pricing("npc", npc_prices).await;
    }
}
