# NoFrills API

Wrapper for the Hypixel API, used by the
[NoFrills](https://github.com/WhatYouThing/NoFrills) mod.

Feel free to use this project as a base for your own Hypixel API wrapper.

## Features

- IP based rate limiting
- Lowest Bazaar/Auction item prices endpoint
- Active mayor election perks endpoint
- Hypixel API proxy endpoint with response caching

## Usage

Download `cargo` if you don't have it already, clone the repository, and run the
server with `cargo run --release`. Currently only running under reverse proxy is
supported, the API should not be used as a standalone web server due to not
implementing SSL.

## Configuration

The server can be configured with environment variables:

- `HYPIXEL_API_KEY=<api_key>`: Your Hypixel API key. Keep in mind that this is
  not supposed to be the temporary key you can generate on Hypixel's developer
  dashboard.
- `NF_API_PORT=<port>`: The port for the NoFrills API to run locally under,
  defaults to 4269 if not present.
- `NF_API_PROXY_PORT=<port>`: The port for the Hypixel API proxy to run locally
  under, defaults to 4270 if not present.
- `NF_API_CLOUDFLARE=true/false`: Tells the API to read the client IP addresses
  from the Cloudflare "connecting IP" header, defaults to false.
