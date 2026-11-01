use std::{
    env, fs,
    time::{Duration, SystemTime},
};

use actix_multipart::form::MultipartForm;
use actix_web::{HttpRequest, Responder, dev::Response, http::StatusCode};
use serde_json::json;
use ureq::http::HeaderValue;

use crate::{BetaUploadForm, util};

pub async fn post(form: MultipartForm<BetaUploadForm>, req: HttpRequest) -> impl Responder {
    let auth_key = env::var("NF_API_BETA_AUTH").unwrap_or("".to_owned());
    if auth_key.is_empty() {
        return Response::internal_server_error();
    }
    let header = req
        .headers()
        .get("NF-Beta-Auth")
        .map(|h| h.to_str().unwrap_or(""))
        .unwrap_or("");
    if header.is_empty() {
        return Response::new(StatusCode::BAD_REQUEST);
    }
    if header.eq(&auth_key) {
        let path = env::var("NF_API_BETA_PATH").unwrap_or("".to_owned());
        let webhook = env::var("NF_API_BETA_WEBHOOK").unwrap_or("".to_owned());
        if path.is_empty() || webhook.is_empty() {
            return Response::internal_server_error();
        }
        let hash_short = form.hash.split_at(8).0;
        let file_name = format!("nofrills-{}-{}.jar", *form.version, hash_short);
        if let Ok(_) = fs::copy(form.file.file.path(), format!("{}/{}", path, file_name)) {
            let payload = json!({
                "embeds": [
                    {
                        "title": format!("Beta Build for Minecraft {}", *form.version),
                        "description": format!("[**Click here to download**]({})\n\nBranch: `{}`\nCommit: [`{}`]({})\n\nChanges\n```{}```",
                            format!("https://whatyouth.ing/beta/{}", file_name),
                            *form.branch,
                            hash_short,
                            format!("https://github.com/WhatYouThing/NoFrills/commit/{}", *form.hash),
                            if form.message.len() > 1000 {
                                form.message.split_at(1000).0.to_owned()
                            } else {
                                (*form.message).to_owned()
                            }
                        ),
                        "color": 0x5ca0bf
                    }
                ]
            });
            let mut builder = util::get_http_agent().post(webhook);
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
