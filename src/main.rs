mod limiter;
mod pricing;
mod util;

use actix_web::{
    App, HttpRequest, HttpServer, Responder,
    body::BoxBody,
    dev::Response,
    get,
    http::{
        StatusCode,
        header::{CONTENT_TYPE, HeaderValue},
    },
    mime::APPLICATION_JSON,
};
use dotenvy::dotenv;
use serde_json::json;
use std::{env, time::Duration};
use tokio::{task, time::sleep};

fn get_port() -> u16 {
    if let Ok(port_secret) = env::var("NF_API_PORT") {
        return port_secret.parse().unwrap();
    }
    return 4269;
}

#[get("/v1/economy/get-item-pricing/")] // compatibility with outdated NoFrills builds
async fn get_item_pricing(req: HttpRequest) -> impl Responder {
    let key = limiter::new_key("get-item-pricing", req).await;
    if limiter::is_limited(&key, 30000, 1).await {
        return Response::new(StatusCode::TOO_MANY_REQUESTS);
    }
    let map = pricing::get().await;
    let bazaar_prices = pricing::get_pricing(&map, "bazaar");
    let mut bazaar_sorted = json!({});
    for (id, prices) in bazaar_prices.as_object().unwrap().iter() {
        let mut price_array = vec![0.0, 0.0];
        if prices["buy"].is_f64() {
            *price_array.get_mut(0).unwrap() = prices["buy"].as_f64().unwrap();
        }
        if prices["sell"].is_f64() {
            *price_array.get_mut(1).unwrap() = prices["sell"].as_f64().unwrap();
        }
        bazaar_sorted[id] = json!(price_array);
    }
    let json = json!({
        "auction": pricing::get_pricing(&map, "auction").to_string(),
        "bazaar": bazaar_sorted.to_string(),
        "attribute": pricing::get_pricing(&map, "attribute").to_string(),
        "npc": pricing::get_pricing(&map, "npc").to_string()
    });
    let mut res = Response::new(StatusCode::OK).set_body(BoxBody::new(json.to_string()));
    res.headers_mut().append(
        CONTENT_TYPE,
        HeaderValue::from_static(APPLICATION_JSON.essence_str()),
    );
    return res;
}

#[get("/v2/economy/get-item-pricing/")]
async fn get_item_pricing_v2(req: HttpRequest) -> impl Responder {
    let key = limiter::new_key("get-item-pricing", req).await;
    if limiter::is_limited(&key, 30000, 1).await {
        return Response::new(StatusCode::TOO_MANY_REQUESTS);
    }
    let mut res = Response::new(StatusCode::OK).set_body(pricing::get_pricing_json().await);
    res.headers_mut().append(
        CONTENT_TYPE,
        HeaderValue::from_static(APPLICATION_JSON.essence_str()),
    );
    return res;
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _ = dotenv().unwrap();
    task::spawn(async {
        loop {
            pricing::refresh_auction_house().await;
            sleep(Duration::from_millis(240000)).await;
        }
    });
    task::spawn(async {
        loop {
            pricing::refresh_bazaar().await;
            sleep(Duration::from_millis(120000)).await;
        }
    });
    task::spawn(async {
        loop {
            pricing::refresh_npc().await;
            sleep(Duration::from_millis(1800000)).await;
        }
    });
    task::spawn(async {
        loop {
            limiter::clear_expired().await;
            sleep(Duration::from_millis(60000)).await;
        }
    });
    HttpServer::new(|| {
        App::new()
            .service(get_item_pricing_v2)
            .service(get_item_pricing)
    })
    .bind(("0.0.0.0", get_port()))?
    .run()
    .await
}
