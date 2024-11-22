import * as fs from "fs"
import Bun from "bun"
import nbtjs from "nbt-js"
import { urlToHttpOptions, URL } from "node:url"

const config = JSON.parse(fs.readFileSync(`${__dirname}/config.json`).toString())
const apiKey = !config.apiKey ? process.env.HYPIXEL_API_KEY : config.apiKey

class Limiter {
    ip; endpoint; time

    constructor(ip="", endpoint="") {
        this.ip = ip;
        this.endpoint = endpoint;
        this.time = new Date().getTime();
    }

    isLimited(maxRequests = 10, minTime = 60000) {
        if (!this.exists()) {
            return false
        }
        const data = this.get()
        if (data.first + data.expiry >= time) {
            this.remove(ip)
            return false
        }
        if (data.count >= requestAmount && data.last - data.first <= timeframe) {
            return true
        }
    }
    get() {
        return util.rateLimits.get(this.ip)
    }
    add() {
        if (!this.exists(ip)) {
            const time = new Date().getTime();
            expiry *= 1000
            this.data[ip] = { count: 1, last: time, first: time, expiry: expiry }
        }
        else {
            this.data[ip].count += 1
            this.data[ip].last = new Date().getTime()
        }
    }
    remove() {
        if (this.exists(ip)) {
            delete this.data[ip]
        }
    }
    exists() {
        return util.rateLimits.has(this.ip)
    }
}

const util = {
    lastRefresh: {
        auctionHouse: 0,
        bazaar: 0
    },
    marketCache: {
        auctionHouse: {},
        bazaar: {}
    },
    priceCache: {
        auctionHouse: new Map(),
        bazaar: new Map()
    },
    rateLimits: new Map()
}

const endpoints = {
    v1: {
        economy: {
            auctionHouse: {
                checkAttributes: "/v1/economy/auction/attribute-value"
            },
            pricing: {
                getItemsLowestPrice: "/v1/economy/pricing/items-value"
            }
        },
        player: {
            profile: {
                getCurrent: "/v1/player/profile/get",
                listProfiles: "/v1/player/profile/list",
            },
            overview: {
                kuudra: "/v1/player/overview/kuudra",
                dungeons: "/v1/player/overview/dungeons",
            }
        }
    }
}

async function makeRequest({ url = "", method = "GET", body = "" }) {
    return await fetch(`https://api.hypixel.net/${url}`, {
        method: method,
        body: body & method != "GET" ? body : null,
        headers: {
            "API-Key": apiKey
        }
    })
}

function parseItemData(data = "") {
    const buffer = new Buffer.from(data, "base64")
    const gzip = Bun.gunzipSync(buffer)
    const nbt = nbtjs.read(gzip)
    return nbt.payload[""]["i"].shift()
}

function parseRequestPath(path = "") {
    if (path.includes("?")) {
        path = path.split("?").shift()
    }
    if (path.endsWith("/")) {
        path = path.substring(0, path.length - 1)
    }
    return path
}

Bun.serve({
    async fetch(req, server) {
        const reqIP = config.cloudflareMode ? req.headers.get("cf-connecting-ip") : server.requestIP(req).address
        const options = urlToHttpOptions(new URL(req.url))
        const path = parseRequestPath(options.path)
        if (req.method == "GET") {
            if (path.startsWith("/v1")) {

            }
        }
        if (req.method == "POST") {

        }
        return new Response("", {
            status: 400
        })
    },
    development: false,
    port: config.port
})