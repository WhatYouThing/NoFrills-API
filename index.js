import * as fs from "fs"
import Bun from "bun"
import zlib from "zlib"
import nbtjs from "nbt-js"
import { urlToHttpOptions, URL } from "node:url"

const config = await Bun.file(`${__dirname}/config.json`).json()
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
    sinceUpdate: {
        auctions: 0,
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
    },
    async refreshAuctions() {
        const res = await makeRequest({
            url: "v2/skyblock/auctions"
        })
        if (res.status == 200) {
            this.cache.auctionHouse = await res.json()
        }
    },
    async refreshBazaar() {
        const res = await makeRequest({
            url: "v2/skyblock/bazaar"
        })
        if (res.status == 200) {
            this.cache.bazaar = await res.json()
        }
    },
    logFile: `${__dirname}/log.txt`,
    log(message = "") {
        if (!config.logRequests) {
            return;
        }
        if (!fs.existsSync(this.logFile)) {
            fs.writeFileSync(this.logFile, `${message}\n`)
        }
        else {
            fs.appendFileSync(this.logFile, `${message}\n`)
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
    const buffer = Buffer.from(data, "base64")
    const gzip = zlib.gunzipSync(buffer)
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

setInterval(async () => {
    if (util.sinceUpdate.auctions == 4) { // refesh auctions every 2m30s
        await util.refreshAuctions()
        util.sinceUpdate.auctions = 0
    }
    else {
        util.sinceUpdate.auctions += 1
    }
    if (util.sinceUpdate.bazaar == 3) { // refesh bazaar every 2m
        await util.refreshBazaar()
        util.sinceUpdate.bazaar = 0
    }
    else {
        util.sinceUpdate.bazaar += 1
    }
    Bun.gc()
}, 30000)

await util.refreshAuctions()
await util.refreshBazaar()

Bun.serve({
    async fetch(req, server) {
        const reqIP = config.cloudflareMode ? req.headers.get("cf-connecting-ip") : server.requestIP(req).address
        const options = urlToHttpOptions(new URL(req.url))
        const path = parseRequestPath(options.path)
        const time = new Date().getTime();
        const limiter = new Limiter(reqIP, path, time)
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
            const json = await req.json().catch()
            if (!json) {
                return util.responses.badRequest
            }
            if (path == "/v1/economy/get-attribute-price") {

            }
            if (path == "/v1/economy/get-items-price") {
                let auctionItems = []
                let bazaarItems = []
                await Promise.all(util.cache.auctionHouse.auctions.map(auction => {
                    if (auction.bin) {
                        let data = parseItemData(auction.item_bytes)
                        let id = data.ExtraAttributes.id
                        if (json.items.includes(id)) {
                            let item = auctionItems.find(item => item.id == id)
                            if (item) {
                                if (auction.starting_bid < item.price) {
                                    item.price = Math.ceil(auction.starting_bid)
                                }
                            }
                            else {
                                auctionItems.push({ id: id, price: Math.ceil(auction.starting_bid) })
                            }
                        }
                    }
                }))
                await Promise.all(json.items.map(item => {
                    let data = util.cache.bazaar.products.item
                    if (data) {
                        bazaarItems.push({ id: item, buy: Math.ceil(data.quick_status.buyPrice), sell: Math.ceil(data.quick_status.sellPrice) })
                    }
                }))
                return new Response(JSON.stringify({
                    auction: auctionItems,
                    bazaar: bazaarItems
                }))
            }
        }
        return util.responses.badRequest
    },
    development: false,
    port: config.port
})