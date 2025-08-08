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
use std::{env, time::Duration};
use tokio::{task, time::sleep};

fn get_port() -> u16 {
    if let Ok(port_secret) = env::var("NF_API_PORT") {
        return port_secret.parse().unwrap();
    }
    return 4269;
}

fn response_ok(body: BoxBody) -> Response<BoxBody> {
    let mut res = Response::new(StatusCode::OK).set_body(body);
    res.headers_mut().append(
        CONTENT_TYPE,
        HeaderValue::from_static(APPLICATION_JSON.essence_str()),
    );
    return res;
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
    tracking::add_usage("pricing").await;
    return response_ok(BoxBody::new(json.to_string()));
}

#[get("/v2/economy/get-item-pricing/")]
async fn get_item_pricing_v2(req: HttpRequest) -> impl Responder {
    let key = limiter::new_key("get-item-pricing", req).await;
    if limiter::is_limited(&key, 30000, 1).await {
        return Response::new(StatusCode::TOO_MANY_REQUESTS);
    }
    tracking::add_usage("pricing").await;
    return response_ok(pricing::get_pricing_json().await);
}

#[get("/v1/misc/get-api-usage/")]
async fn get_api_usage(req: HttpRequest) -> impl Responder {
    let key = limiter::new_key("get-api-usage", req).await;
    if limiter::is_limited(&key, 1000, 1).await {
        return Response::new(StatusCode::TOO_MANY_REQUESTS);
    }
    return response_ok(tracking::get_usage_json().await);
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    util::load_env_file();

    task::spawn(async {
        let duration = Duration::from_millis(240000);
        loop {
            pricing::refresh_auction_house().await;
            sleep(duration).await;
        }
    });

    task::spawn(async {
        let duration = Duration::from_millis(120000);
        loop {
            pricing::refresh_bazaar().await;
            sleep(duration).await;
        }
    });

    task::spawn(async {
        let duration = Duration::from_millis(1800000);
        loop {
            pricing::refresh_npc().await;
            sleep(duration).await;
        }
    });

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Always,
            ))
            .service(get_item_pricing_v2)
            .service(get_item_pricing)
            .service(get_api_usage)
    })
    .bind(("0.0.0.0", get_port()))?
    .run()
    .await
}
