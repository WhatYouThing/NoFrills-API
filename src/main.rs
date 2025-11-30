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
        header::{CONTENT_TYPE, HeaderName, HeaderValue},
    },
    middleware,
    mime::APPLICATION_JSON,
    post,
    web::{Bytes, PayloadConfig},
};
use serde_json::{Value, json};
use std::{
    env, fs,
    sync::LazyLock,
    time::{Duration, SystemTime},
};
use tokio::{task, time::sleep};
use ureq::{Agent, AsSendBody};

static BETA_AUTH: LazyLock<String> =
    LazyLock::new(|| env::var("NF_API_BETA_AUTH").unwrap_or(String::new()));

fn get_port() -> u16 {
    if let Ok(port_secret) = env::var("NF_API_PORT") {
        return port_secret.parse().unwrap();
    }
    return 4269;
}

fn get_header(req: &HttpRequest, name: &'static str) -> String {
    let headers = req.headers();
    let key = HeaderName::from_static(name);
    if headers.contains_key(&key) {
        return headers
            .get(&key)
            .unwrap()
            .to_str()
            .unwrap_or("")
            .to_string();
    }
    return "".to_string();
}

fn response_ok(body: BoxBody) -> Response<BoxBody> {
    let mut res = Response::new(StatusCode::OK).set_body(body);
    res.headers_mut().append(
        CONTENT_TYPE,
        HeaderValue::from_static(APPLICATION_JSON.essence_str()),
    );
    return res;
}

async fn http_post(
    url: String,
    body: impl AsSendBody,
) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    Agent::new_with_defaults()
        .post(url)
        .header("Content-Type", "application/json")
        .send(body)
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

#[post("/v1/misc/post-beta-build/")]
async fn post_beta_build(payload: Bytes, req: HttpRequest) -> impl Responder {
    let header = get_header(&req, "nf-beta-auth");
    if !BETA_AUTH.is_empty() && BETA_AUTH.eq(&header) {
        let path = env::var("NF_API_BETA_PATH");
        let webhook = env::var("NF_API_BETA_WEBHOOK");
        if path.is_err() || webhook.is_err() {
            return Response::internal_server_error();
        }
        let data = String::from_utf8(payload.to_vec());
        if data.is_err() {
            return Response::new(StatusCode::BAD_REQUEST);
        }
        let json: Result<Value, serde_json::Error> = serde_json::from_str(&data.unwrap());
        if json.is_err() {
            return Response::new(StatusCode::BAD_REQUEST);
        }
        let body = json.unwrap();
        let hash = body["hash"].as_str().unwrap_or("");
        let version = body["version"].as_str().unwrap_or("");
        let message = body["message"].as_str().unwrap_or("");
        let bytes = body["bytes"].as_array();
        if hash.is_empty() || version.is_empty() || message.is_empty() || bytes.is_none() {
            return Response::new(StatusCode::BAD_REQUEST);
        }
        let bytes_raw: Vec<u8> = bytes
            .unwrap()
            .iter()
            .map(|byte| byte.as_u64().unwrap() as u8)
            .collect(); // converts serde_json values to raw bytes
        let hash_short = hash.split_at(8).0;
        let file_name = format!("nofrills-{}-{}.jar", version, hash_short);
        let write = fs::write(format!("{}/{}", path.unwrap(), file_name), bytes_raw);
        if write.is_ok() {
            let message = json!({
                "embeds": [
                    {
                        "title": format!("Beta Buikd for Minecraft {}", version),
                        "description": format!("[**Click here to download**]({})\n\nCommit: [`{}`]({})\nChanges:\n```{}```",
                            format!("https://whatyouth.ing/beta/{}", file_name),
                            hash_short,
                            format!("https://github.com/WhatYouThing/NoFrills/commit/{}", hash),
                            message
                        ),
                        "color": 0x5ca0bf
                    }
                ]
            });
            let _ = http_post(webhook.unwrap(), message.to_string()).await;
            return Response::ok();
        }
    }
    return Response::new(StatusCode::UNAUTHORIZED);
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

    task::spawn(async {
        let duration = Duration::from_millis(3600000);
        loop {
            if let Ok(path) = env::var("NF_API_BETA_PATH") {
                if let Ok(dir) = fs::read_dir(path) {
                    for entry in dir {
                        if !entry.is_ok() {
                            continue;
                        }
                        let file = entry.unwrap();
                        let metadata = file.metadata();
                        if metadata.is_ok() {
                            let now = SystemTime::now();
                            let expiry = Duration::from_millis(1209600000);
                            let created = metadata.unwrap().created().unwrap_or(now);
                            if created.elapsed().unwrap_or(Duration::from_millis(0)) >= expiry {
                                let _ = fs::remove_file(file.path()); // automatically clean up 2 week old builds
                            }
                        }
                    }
                }
            }
            sleep(duration).await;
        }
    });

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Always,
            ))
            .app_data(PayloadConfig::new(10000000))
            .service(get_item_pricing_v2)
            .service(get_item_pricing)
            .service(get_api_usage)
            .service(post_beta_build)
    })
    .bind(("0.0.0.0", get_port()))?
    .run()
    .await
}
