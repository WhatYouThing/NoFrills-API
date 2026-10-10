use std::{sync::LazyLock, time::Duration};

use actix_web::{HttpRequest, HttpResponse, Responder, http::StatusCode, web::Query};
use serde::Deserialize;
use stretto::{AsyncCacheBuilder, TokioCache, TokioRuntime};
use tokio::sync::Mutex;

use crate::util;

static CACHE: LazyLock<Mutex<TokioCache<String, (StatusCode, Vec<(String, String)>, Vec<u8>)>>> =
    LazyLock::new(|| {
        Mutex::new(
            AsyncCacheBuilder::new(5000, 50000)
                .build::<TokioRuntime>()
                .unwrap(),
        )
    }); // mutex is needed to ensure that consecutive requests dont miss the pending cache result

#[derive(Debug, Deserialize)]
pub struct Parameters {
    uuid: Option<String>,
    player: Option<String>,
    profile: Option<String>,
    page: Option<String>,
}

impl Parameters {
    pub fn to_list(&self) -> Vec<(String, String)> {
        let mut list = Vec::new();
        self.push_to_list(&mut list, "uuid", &self.uuid);
        self.push_to_list(&mut list, "player", &self.player);
        self.push_to_list(&mut list, "profile", &self.profile);
        self.push_to_list(&mut list, "page", &self.page);
        return list;
    }

    fn push_to_list(&self, list: &mut Vec<(String, String)>, key: &str, value: &Option<String>) {
        let value = value.to_owned().unwrap_or("".to_owned());
        if !value.is_empty() {
            list.push((key.to_owned(), value));
        }
    }

    pub fn to_string(&self) -> String {
        return self
            .to_list()
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<String>>()
            .join("&")
            .to_string();
    }
}

pub async fn handle(req: HttpRequest, params: Query<Parameters>) -> impl Responder {
    let path = req.path().trim_start_matches("/").to_owned();
    let key = format!("{}?{}", &path, params.to_string());
    let lock = CACHE.lock().await;
    let (status, headers, body) = if let Some(cached) = lock.get(&key).await {
        cached.value().to_owned()
    } else {
        let result = util::build_request(&path)
            .config()
            .http_status_as_error(false)
            .build()
            .query_pairs(params.to_list())
            .call();
        if result.is_err() {
            (StatusCode::INTERNAL_SERVER_ERROR, Vec::new(), Vec::new())
        } else {
            let res = result.unwrap();
            let status = StatusCode::from_u16(res.status().as_u16()).unwrap();
            let headers = res
                .headers()
                .iter()
                .filter(|(n, _)| !n.as_str().eq_ignore_ascii_case("connection"))
                .map(|(n, v)| (n.as_str().to_owned(), v.to_str().unwrap_or("").to_owned()))
                .collect::<Vec<(String, String)>>();
            let body = res.into_body().read_to_vec().unwrap_or(Vec::new());
            let _ = lock
                .insert_with_ttl(
                    key,
                    (status.to_owned(), headers.to_owned(), body.to_owned()),
                    1,
                    Duration::from_secs(60),
                )
                .await;
            (status, headers, body)
        }
    };
    let mut response = HttpResponse::build(status);
    for header in headers {
        response.append_header(header.to_owned());
    }
    return response.body(body);
}
