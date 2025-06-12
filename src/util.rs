use std::{env, io::Read};

use base64::{engine::general_purpose, Engine};
use crab_nbt::NbtCompound;
use flate2::bufread::GzDecoder;
use serde_json::Value;
use ureq::{Agent, Body};

fn get_http_agent() -> Agent {
    return Agent::config_builder().build().new_agent();
}

pub async fn make_request(url: &str) -> Result<ureq::http::Response<ureq::Body>, ureq::Error> {
    return get_http_agent()
        .get(format!("https://api.hypixel.net/{}", url))
        .header("API-Key", env::var("HYPIXEL_API_KEY").unwrap())
        .call();
}

pub fn parse_json(mut req: ureq::http::Response<Body>) -> Value {
    let json: Value = serde_json::from_reader(req.body_mut().as_reader()).unwrap();
    return json;
}

pub fn parse_json_str(json_str: &str) -> Value {
    let json: Value = serde_json::from_str(json_str).unwrap();
    return json;
}

pub fn decode_base64(b64: &str) -> Vec<u8> {
    return general_purpose::STANDARD.decode(b64).unwrap();
}

pub fn decode_gzip(bytes: Vec<u8>) -> Vec<u8> {
    let mut decoder = GzDecoder::new(&bytes[..]);
    let output = &mut Vec::new();
    let _ = decoder.read_to_end(output).unwrap();
    return output.to_vec();
}

pub async fn parse_item_nbt(gzip: &str) -> NbtCompound {
    let decoded = decode_gzip(decode_base64(gzip));
    let compound = NbtCompound::deserialize_content(&mut decoded.as_slice()).unwrap();
    let parent_tag = &compound.child_tags.first().unwrap().1;
    let tag_list = parent_tag
        .extract_compound()
        .unwrap()
        .get_list("i")
        .unwrap();
    return tag_list
        .first()
        .unwrap()
        .extract_compound()
        .unwrap()
        .to_owned();
}