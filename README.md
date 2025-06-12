# NoFrills API

Wrapper for the Hypixel API, used by the [NoFrills](https://github.com/WhatYouThing/NoFrills) mod.

Feel free to use this project as a base for your own Hypixel API wrapper.

## Features

- IP based rate limiting
- Automatic collection, filtering and caching of the prices of any Skyblock item

## Usage

Download `cargo` if you don't have it already, clone the repository, and run the server with `cargo run --release`.
Currently only running under reverse proxy with Cloudflare mode enabled is supported.

## Configuration

The server can be configured with environment variables:
- `HYPIXEL_API_KEY=<api_key>`: Your Hypixel API key. Keep in mind that this is not supposed to be 
the temporary key you can generate on Hypixel's developer dashboard.
- `NF_API_PORT=<port>`: The port for the API to run locally under, defaults to 4269 if not present.
- `NF_API_CLOUDFLARE=true/false`: Tells the API to read the client's IP address from the Cloudflare header, defaults to false if not present.