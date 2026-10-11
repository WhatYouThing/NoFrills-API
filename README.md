# NoFrills API

Wrapper for the Hypixel API, used by the
[NoFrills](https://github.com/WhatYouThing/NoFrills) mod.

Feel free to use this project as a base for your own Hypixel API wrapper.

## Features

- IP based rate limiting
- Lowest Bazaar/Auction item prices endpoint
- Active mayor election perks endpoint

## Usage

Download `cargo` if you don't have it already, clone the repository, and run the
server with `cargo run --release`. Currently only running under reverse proxy is
supported, the API should not be used as a standalone web server due to not
implementing SSL.

## Configuration

The server can be configured with environment variables:

- `NF_API_PORT=<port>`: The port for the NoFrills API to run locally under,
  defaults to 4269 if not present.
- `NF_API_CLOUDFLARE=true/false`: Tells the API to read the client IP addresses
  from the Cloudflare "connecting IP" header, defaults to false.
