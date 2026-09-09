mod betas;
mod election;
mod items;
mod limiter;
mod pricing;
mod streams;
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
    post,
    web::{Bytes, PayloadConfig},
};
use serde_json::json;
use std::{
    env::{self, current_dir},
    fs,
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

async fn authenticate(req: &HttpRequest) -> bool {
    let headers = req.headers();
    let mod_ver = headers
        .get("X-NoFrills-ModVer")
        .map(|h| h.to_str().unwrap_or(""));
    let game_ver = headers
        .get("X-NoFrills-GameVer")
        .map(|h| h.to_str().unwrap_or(""));
    if mod_ver.is_none() || game_ver.is_none() {
        return false;
    }
    let path = current_dir().unwrap().join("versions.json");
    if !fs::exists(&path).unwrap_or(false) {
        let _ = fs::write(&path, json!({"mod": [],"mc": []}).to_string()).unwrap();
    }
    let file = fs::read_to_string(&path).unwrap();
    if let Some(json) = util::parse_json_str(&file) {
        let mod_versions = json["mod"].as_array().unwrap();
        let mc_versions = json["mc"].as_array().unwrap();
        return mod_versions.contains(&json!(mod_ver.unwrap()))
            && mc_versions.contains(&json!(game_ver.unwrap()));
    }
    return false;
}

#[get("/v2/economy/get-item-pricing/")]
async fn get_item_pricing_v2(req: HttpRequest) -> impl Responder {
    let key = limiter::new_key("get-item-pricing", &req).await;
    if limiter::is_limited(&key, 10000, 1).await {
        return Response::new(StatusCode::TOO_MANY_REQUESTS);
    }
    if !authenticate(&req).await {
        return Response::new(StatusCode::UNAUTHORIZED);
    }
    return response_ok(pricing::get_pricing_json().await);
}

#[get("/v1/election/get-active-perks/")]
async fn get_active_perks(req: HttpRequest) -> impl Responder {
    let key = limiter::new_key("get-active-perks", &req).await;
    if limiter::is_limited(&key, 10000, 1).await {
        return Response::new(StatusCode::TOO_MANY_REQUESTS);
    }
    if !authenticate(&req).await {
        return Response::new(StatusCode::UNAUTHORIZED);
    }
    return response_ok(election::get_perks_json().await);
}

#[get("/v1/items/get-non-placeable/")]
async fn get_non_placeable(req: HttpRequest) -> impl Responder {
    let key = limiter::new_key("get-non-placeable", &req).await;
    if limiter::is_limited(&key, 10000, 1).await {
        return Response::new(StatusCode::TOO_MANY_REQUESTS);
    }
    if !authenticate(&req).await {
        return Response::new(StatusCode::UNAUTHORIZED);
    }
    return response_ok(items::get_non_placeable_json().await);
}

#[get("/v1/items/get-museum-data/")]
async fn get_museum_data(req: HttpRequest) -> impl Responder {
    let key = limiter::new_key("get-museum-data", &req).await;
    if limiter::is_limited(&key, 10000, 1).await {
        return Response::new(StatusCode::TOO_MANY_REQUESTS);
    }
    if !authenticate(&req).await {
        return Response::new(StatusCode::UNAUTHORIZED);
    }
    return response_ok(items::get_museum_data_json().await);
}

#[get("/v1/items/get-item-textures/")]
async fn get_item_textures(req: HttpRequest) -> impl Responder {
    let key = limiter::new_key("get-item-textures", &req).await;
    if limiter::is_limited(&key, 10000, 1).await {
        return Response::new(StatusCode::TOO_MANY_REQUESTS);
    }
    if !authenticate(&req).await {
        return Response::new(StatusCode::UNAUTHORIZED);
    }
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
async fn get_skyblock_streams(req: HttpRequest) -> impl Responder {
    let key = limiter::new_key("get-skyblock-streams", &req).await;
    if limiter::is_limited(&key, 200, 1).await {
        return Response::new(StatusCode::TOO_MANY_REQUESTS);
    }
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
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::Always,
            ))
            .app_data(PayloadConfig::new(10000000))
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
