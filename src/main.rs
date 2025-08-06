mod limiter;
mod pricing;
mod tracking;
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
    middleware,
    mime::APPLICATION_JSON,
};
use serde_json::json;
use std::env;

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
        "attribute": json!({}).to_string(),
        "npc": pricing::get_pricing(&map, "npc").to_string()
    });
    let mut res = Response::new(StatusCode::OK).set_body(BoxBody::new(json.to_string()));
    res.headers_mut().append(
        CONTENT_TYPE,
        HeaderValue::from_static(APPLICATION_JSON.essence_str()),
    );
    tracking::add_pricing_count().await;
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
    tracking::add_pricing_count().await;
    return res;
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    util::load_env_file();
    pricing::init();
    limiter::init();
    tracking::init();
    HttpServer::new(|| {
        App::new()
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Always,
            ))
            .service(get_item_pricing_v2)
            .service(get_item_pricing)
    })
    .bind(("0.0.0.0", get_port()))?
    .run()
    .await
}
