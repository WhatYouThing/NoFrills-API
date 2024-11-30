import * as fs from "fs"
import Bun from "bun"
import nbtjs from "nbt-js"
import { urlToHttpOptions, URL } from "node:url"

const config = JSON.parse(fs.readFileSync(`${__dirname}/config.json`).toString())
const apiKey = !config.apiKey ? process.env.HYPIXEL_API_KEY : config.apiKey

class Limiter {
    ip; endpoint; key; time

    constructor(ip = "", endpoint = "", time = 0) {
        this.ip = ip;
        this.endpoint = endpoint;
        this.key = new Buffer.from(endpoint + ip).toString("base64")
        this.time = time;
    }

    limited(maxRequests = 10) {
        return this.exists() && this.get() >= maxRequests
    }
    add(ttl = 60000) {
        let value = this.exists() ? this.get() + 1 : 1
        this.set(value)
        setTimeout(() => {
            let valueNew = this.get() - 1
            if (valueNew > 0) {
                this.set(valueNew)
            }
            else {
                this.remove()
            }
        }, ttl);
    }
    get() {
        return util.rateLimits.get(this.key)
    }
    set(value) {
        util.rateLimits.set(this.key, value)
    }
    remove() {
        return util.rateLimits.delete(this.key)
    }
    exists() {
        return util.rateLimits.has(this.key)
    }
}

const util = {
    lastRefresh: {
        auctionHouse: 0,
        bazaar: 0
    },
    cache: {
        auctionHouse: undefined,
        bazaar: undefined,
        itemPrice: new Map()
    },
    rateLimits: new Map(),
    responses: {
        badRequest: new Response("{}", {
            status: 400
        }),
        tooManyRequests: new Response("{}", {
            status: 429
        })
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
        const time = new Date().getTime();
        const limiter = new Limiter(reqIP, path, time)
        if (util.lastRefresh.auctionHouse + 150000 > time) {
            const res = await makeRequest({
                url: "v2/skyblock/auctions"
            })
            util.cache.auctionHouse = await res.json()
        }
        if (util.lastRefresh.auctionHouse + 120000 > time) {
            const res = await makeRequest({
                url: "v2/skyblock/bazaar"
            })
            util.cache.bazaar = await res.json()
        }
        if (req.method == "GET") {
            if (path == "/v1/player/get-profile") {
                if (limiter.limited(5)) {
                    return util.responses.tooManyRequests
                }
                limiter.add(30000)
            }
            if (path == "/v1/player/list-profiles") {

            }
            if (path == "/v1/player/get-status") {

            }
        }
        if (req.method == "POST") {
            if (path == "/v1/economy/get-attribute-price") {

            }
            if (path == "/v1/economy/get-items-price") {

            }
        }
        return util.responses.badRequest
    },
    development: false,
    port: config.port
})