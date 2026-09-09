use std::{env, sync::LazyLock};

use actix_web::body::BoxBody;
use serde_json::{Value, json};
use tokio::sync::{Mutex, MutexGuard};
use ureq::http::{HeaderValue, StatusCode};

use crate::util;

static STREAMS: LazyLock<Mutex<Vec<Value>>> = LazyLock::new(|| Mutex::new(Vec::new()));

pub async fn get() -> MutexGuard<'static, Vec<Value>> {
    return STREAMS.lock().await;
}

pub async fn get_streams_json() -> BoxBody {
    let lock = get().await;
    let mut list = lock.to_vec();
    list.sort_by(|first, second| {
        first["viewers"]
            .as_u64()
            .unwrap()
            .cmp(&second["viewers"].as_u64().unwrap())
    });
    list.reverse();
    let json = json!({
        "list": list
    });
    return BoxBody::new(json.to_string());
}

async fn get_twitch_oauth_token() -> Option<String> {
    let agent = util::get_http_agent();
    let oauth_url = format!(
        "https://id.twitch.tv/oauth2/token?client_id={}&client_secret={}&grant_type=client_credentials",
        env::var("TWITCH_CLIENT_ID").unwrap(),
        env::var("TWITCH_CLIENT_SECRET").unwrap()
    );
    if let Ok(oauth) = agent.post(oauth_url).send_empty() {
        if oauth.status() != StatusCode::OK {
            return None;
        }
        if let Some(oauth_json) = util::parse_json(oauth) {
            if let Some(token) = oauth_json["access_token"].as_str() {
                return Some(token.to_string());
            }
        }
    }
    return None;
}

async fn get_with_auth(url: String, token: &String) -> Option<Value> {
    let agent = util::get_http_agent();
    let mut req = agent.get(url);
    let headers = req.headers_mut().unwrap();
    headers.append(
        "Authorization",
        HeaderValue::from_str(format!("Bearer {}", token).as_str()).unwrap(),
    );
    headers.append(
        "Client-Id",
        HeaderValue::from_str(env::var("TWITCH_CLIENT_ID").unwrap().as_str()).unwrap(),
    );
    if let Ok(res) = req.call() {
        if res.status() != StatusCode::OK {
            return None;
        }
        if let Some(json) = util::parse_json(res) {
            return Some(json);
        }
    }
    return None;
}

pub async fn refresh_streams() {
    if let Some(token) = get_twitch_oauth_token().await {
        let mut cursor = "".to_string();
        let mut page = 0;
        let mut lock = get().await;
        lock.clear();
        loop {
            if page > 0 && cursor.is_empty() {
                break;
            }
            let url = format!(
                "https://api.twitch.tv/helix/streams?game_id=27471&first=100&type=live{}",
                if !cursor.is_empty() {
                    format!("&after={}", cursor)
                } else {
                    "".to_string()
                }
            );
            if let Some(json) = get_with_auth(url, &token).await {
                for stream in json["data"].as_array().unwrap_or(&Vec::new()) {
                    let title = stream["title"].as_str().unwrap();
                    if !title.to_lowercase().contains("hypixel skyblock") {
                        continue;
                    }
                    let tags: Vec<&str> = if let Some(tags_array) = stream["tags"].as_array() {
                        tags_array.iter().map(|tag| tag.as_str().unwrap()).collect()
                    } else {
                        Vec::new()
                    };
                    let user_name = stream["user_name"].as_str().unwrap();
                    let search = lock
                        .iter()
                        .find(|s| s["name"].as_str().unwrap().eq(user_name));
                    if search.is_some() {
                        continue;
                    }
                    let user_url = format!(
                        "https://api.twitch.tv/helix/users?id={}",
                        stream["user_id"].as_str().unwrap()
                    );
                    if let Some(user) = get_with_auth(user_url, &token).await {
                        let data = json!({
                            "title": title,
                            "name": user_name,
                            "viewers": stream["viewer_count"].as_u64().unwrap(),
                            "tags": tags,
                            "thumbnail": stream["thumbnail_url"].as_str().unwrap().replace("{width}", "1920").replace("{height}", "1080"),
                            "avatar": user["data"][0]["profile_image_url"].as_str().unwrap()
                        });
                        lock.push(data);
                    }
                }
                if let Some(key) = json["pagination"]["cursor"].as_str() {
                    cursor = key.to_owned();
                } else {
                    cursor = "".to_string();
                }
            }
            page += 1;
        }
    }
}
