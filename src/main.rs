mod betas;
mod election;
mod items;
mod pricing;
mod streams;
mod util;

use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder,
    body::BoxBody,
    dev::{Response, ServiceRequest, ServiceResponse},
    get,
    http::{
        StatusCode,
        header::{CONTENT_TYPE, HeaderValue},
    },
    middleware::{self, Next},
    mime::APPLICATION_JSON,
    post,
    web::{Bytes, PayloadConfig},
};
use actix_web_ratelimit::{RateLimit, config::RateLimitConfig, store::MemoryStore};
use serde_json::{Value, json};
use std::{
    env::{self, current_dir},
    fs,
    sync::Arc,
    time::Duration,
};
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

fn find_version(list: &Vec<Value>, value: &str) -> bool {
    return list
        .iter()
        .find(|v| v.as_str().unwrap().eq(value))
        .is_some();
}

async fn authenticate(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<BoxBody>, actix_web::Error> {
    if req.path().starts_with("/v1/misc/") {
        return next.call(req).await;
    }
    let headers = req.headers();
    let mod_ver = headers
        .get("X-NoFrills-ModVer")
        .map(|h| h.to_str().unwrap_or(""))
        .unwrap_or("");
    let game_ver = headers
        .get("X-NoFrills-GameVer")
        .map(|h| h.to_str().unwrap_or(""))
        .unwrap_or("");
    let path = current_dir().unwrap().join("versions.json");
    if !fs::exists(&path).unwrap_or(false) {
        let json = json!({
            "mod": [],
            "mc": []
        });
        let _ = fs::write(&path, json.to_string()).unwrap();
    }
    let file = fs::read_to_string(&path).unwrap();
    if let Some(json) = util::parse_json_str(&file) {
        let mod_versions = json["mod"].as_array().unwrap();
        let mc_versions = json["mc"].as_array().unwrap();
        if find_version(mod_versions, mod_ver) && find_version(mc_versions, game_ver) {
            return next.call(req).await;
        }
    }
    return Ok(req.into_response(HttpResponse::Unauthorized().finish()));
}

#[get("/v2/economy/get-item-pricing/")]
async fn get_item_pricing_v2(_: HttpRequest) -> impl Responder {
    return response_ok(pricing::get_pricing_json().await);
}

#[get("/v1/election/get-active-perks/")]
async fn get_active_perks(_: HttpRequest) -> impl Responder {
    return response_ok(election::get_perks_json().await);
}

#[get("/v1/items/get-non-placeable/")]
async fn get_non_placeable(_: HttpRequest) -> impl Responder {
    return response_ok(items::get_non_placeable_json().await);
}

#[get("/v1/items/get-museum-data/")]
async fn get_museum_data(_: HttpRequest) -> impl Responder {
    return response_ok(items::get_museum_data_json().await);
}

#[get("/v1/items/get-item-textures/")]
async fn get_item_textures(_: HttpRequest) -> impl Responder {
    if let Ok(file) = fs::read_to_string(current_dir().unwrap().join("skyblockItemTextures.json")) {
        return response_ok(BoxBody::new(file));
    }
    return Response::new(StatusCode::INTERNAL_SERVER_ERROR);
}

#[post("/v1/misc/post-beta-build/")]
async fn post_beta_build(payload: Bytes, req: HttpRequest) -> impl Responder {
    return betas::post(payload, req).await;
}

#[get("/v1/misc/get-skyblock-streams/")]
async fn get_skyblock_streams(_: HttpRequest) -> impl Responder {
    return response_ok(streams::get_streams_json().await);
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    util::load_env_file();

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
            let req = util::make_request("v2/resources/skyblock/items").await;
            if req.is_err() {
                println!("Panicked while refreshing NPC data:\n{}", req.unwrap_err());
            } else {
                if let Some(json) = util::parse_json(req.unwrap()) {
                    pricing::refresh_npc(&json).await;
                    items::refresh_items(&json).await;
                }
            }
            sleep(Duration::from_millis(1800000)).await;
        }
    });

    task::spawn(async {
        loop {
            election::refresh_perks().await;
            sleep(Duration::from_millis(180000)).await;
        }
    });

    task::spawn(async {
        loop {
            betas::cleanup().await;
            sleep(Duration::from_millis(3600000)).await;
        }
    });

    task::spawn(async {
        loop {
            streams::refresh_streams().await;
            sleep(Duration::from_millis(300000)).await;
        }
    });

    HttpServer::new(|| {
        App::new()
            .app_data(PayloadConfig::new(2000000))
            .wrap(middleware::from_fn(authenticate))
            .wrap(RateLimit::new(
                RateLimitConfig::default()
                    .max_requests(6)
                    .window_secs(30)
                    .id(|req| util::get_request_ip(req)),
                Arc::new(MemoryStore::new()),
            ))
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Always,
            ))
            .service(get_item_pricing_v2)
            .service(get_active_perks)
            .service(get_non_placeable)
            .service(get_museum_data)
            .service(get_item_textures)
            .service(post_beta_build)
            .service(get_skyblock_streams)
    })
    .bind(("0.0.0.0", get_port()))?
    .run()
    .await
}
