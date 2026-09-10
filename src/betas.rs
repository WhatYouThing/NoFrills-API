use std::{
    env, fs,
    time::{Duration, SystemTime},
};

use actix_web::{HttpRequest, Responder, dev::Response, http::StatusCode, web::Bytes};
use serde_json::json;
use ureq::http::HeaderValue;

use crate::util;

pub async fn post(payload: Bytes, req: HttpRequest) -> impl Responder {
    let auth_key = env::var("NF_API_BETA_AUTH");
    if auth_key.is_err() {
        return Response::internal_server_error();
    }
    let header = req.headers().get("nf-beta-auth");
    if header.is_none() {
        return Response::new(StatusCode::BAD_REQUEST);
    }
    let header_value = header.unwrap().to_str().unwrap_or("");
    if header_value.eq(&auth_key.unwrap()) {
        let path = env::var("NF_API_BETA_PATH");
        let webhook = env::var("NF_API_BETA_WEBHOOK");
        if path.is_err() || webhook.is_err() {
            return Response::internal_server_error();
        }
        let data = String::from_utf8(payload.to_vec());
        if data.is_err() {
            return Response::new(StatusCode::BAD_REQUEST);
        }
        let json = util::parse_json_str(&data.unwrap());
        if json.is_none() {
            return Response::new(StatusCode::BAD_REQUEST);
        }
        let body = json.unwrap();
        let hash = body["hash"].as_str().unwrap_or("");
        let version = body["version"].as_str().unwrap_or("");
        let message = body["message"].as_str().unwrap_or("");
        let branch = body["branch"].as_str().unwrap_or("");
        let bytes = body["bytes"].as_array();
        if hash.is_empty()
            || version.is_empty()
            || message.is_empty()
            || branch.is_empty()
            || bytes.is_none()
        {
            return Response::new(StatusCode::BAD_REQUEST);
        }
        let bytes_raw: Vec<u8> = bytes
            .unwrap()
            .iter()
            .map(|byte| byte.as_u64().unwrap() as u8)
            .collect(); // converts serde_json values to raw bytes
        let hash_short = hash.split_at(8).0;
        let file_name = format!("nofrills-{}-{}.jar", version, hash_short);
        if let Ok(_) = fs::write(format!("{}/{}", path.unwrap(), file_name), bytes_raw) {
            let payload = json!({
                "embeds": [
                    {
                        "title": format!("Beta Build for Minecraft {}", version),
                        "description": format!("[**Click here to download**]({})\n\nBranch: `{}`\nCommit: [`{}`]({})\n\nChanges\n```{}```",
                            format!("https://whatyouth.ing/beta/{}", file_name),
                            branch,
                            hash_short,
                            format!("https://github.com/WhatYouThing/NoFrills/commit/{}", hash),
                            if message.len() > 1000 {
                                message.split_at(1000).0
                            } else {
                                message
                            }
                        ),
                        "color": 0x5ca0bf
                    }
                ]
            });
            let mut builder = util::get_http_agent().post(webhook.unwrap());
            let headers = builder.headers_mut().unwrap();
            headers.insert(
                "Content-Type",
                HeaderValue::from_str("application/json").unwrap(),
            );
            let _ = builder.send(payload.to_string()).unwrap();
            return Response::ok();
        }
    }
    return Response::new(StatusCode::UNAUTHORIZED);
}

pub async fn cleanup() {
    if let Ok(path) = env::var("NF_API_BETA_PATH") {
        if let Ok(dir) = fs::read_dir(path) {
            for file in dir.filter(|e| e.is_ok()).map(|e| e.unwrap()) {
                if let Ok(metadata) = file.metadata() {
                    let now = SystemTime::now();
                    let expiry = Duration::from_millis(1209600000);
                    let created = metadata.created().unwrap_or(now);
                    if created.elapsed().unwrap_or(Duration::from_millis(0)) >= expiry {
                        let _ = fs::remove_file(file.path()); // automatically clean up 2 week old builds
                    }
                }
            }
        }
    }
}
