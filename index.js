import * as fs from "fs"
import Bun from "bun"
import zlib from "zlib"
import nbtjs from "nbt-js"
import { URL } from "node:url"

const config = await Bun.file(`${__dirname}/config.json`).json()
const apiKey = !config.apiKey ? process.env.HYPIXEL_API_KEY : config.apiKey

class Limiter {
    ip; endpoint; key;

    constructor(ip = "", endpoint = "") {
        this.ip = ip;
        this.endpoint = endpoint;
        this.key = new Buffer.from(endpoint + ip).toString("base64")
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
        auction: new Map(),
        bazaar: new Map(),
        attribute: new Map()
    },
    rateLimits: new Map(),
    async refreshAuctions() {
        let maxPages = 200
        let auctions = []
        this.log(`Refreshing Auction House data...`)
        for (let page = 0; page < maxPages; page++) {
            const res = await this.makeRequest({
                url: `v2/skyblock/auctions?page=${page}`
            })
            if (res.status == 200) {
                const json = await res.json()
                maxPages = json.totalPages
                await Promise.all(json.auctions.map(auction => {
                    auctions.push(auction)
                }))
            }
            else {
                this.log(`Failed to refresh Auction House data, request for page #${page} returned code ${res.status}.`)
                this.sinceUpdate.auctions = 0
                return
            }
        }
        const prices = new Map()
        const attributePrices = new Map()
        await Promise.all(auctions.map(auction => {
            if (auction.bin) {
                const data = this.parseItemData(auction.item_bytes)
                const extra = data.tag.ExtraAttributes
                if (extra) {
                    let id = extra.id
                    if (id == "PET") {
                        let petInfo = JSON.parse(extra.petInfo)
                        id = `${petInfo.type}_PET_${petInfo.tier}`
                    }
                    if (id == "RUNE") {
                        let runeInfo = Object.entries(extra.runes).at(0)
                        id = `${runeInfo[0]}_${runeInfo[1]}_RUNE`
                    }
                    let attributes = extra.attributes
                    let price = Math.ceil(auction.starting_bid)
                    if (prices.has(id)) {
                        if (price < prices.get(id)) {
                            prices.set(id, price)
                        }
                    }
                    else {
                        prices.set(id, price)
                    }
                    if (attributes) {
                        let entries = Object.entries(attributes)
                        let rollID = ""
                        entries.forEach(([name, value]) => { // finds lowest price for every attribute and its level
                            let attrID = `${name}${value}`
                            if (entries.length == 2) {
                                rollID = `${rollID} ${name}`.trim()
                            }
                            if (attributePrices.has(attrID)) {
                                let pricing = attributePrices.get(attrID)
                                if (!pricing[id] || price < pricing[id]) {
                                    pricing[id] = price
                                    attributePrices.set(attrID, pricing)
                                }
                            }
                            else {
                                let pricing = {}
                                pricing[id] = price
                                attributePrices.set(attrID, pricing)
                            }
                        })
                        if (rollID) { // finds lowest price for any attribute combo
                            if (attributePrices.has(rollID)) {
                                let pricing = attributePrices.get(rollID)
                                if (!pricing[id] || price < pricing[id]) {
                                    pricing[id] = price
                                    attributePrices.set(rollID, pricing)
                                }
                            }
                            else {
                                let pricing = {}
                                pricing[id] = price
                                attributePrices.set(rollID, pricing)
                            }
                        }
                    }
                }
            }
        }))
        this.cache.auction = prices
        this.cache.attribute = attributePrices
        this.sinceUpdate.auctions = 0
        this.log(`Auction House data refreshed successfully. Cached LBIN prices: ${prices.size}.`)
    },
    async refreshBazaar() {
        this.log(`Refreshing Bazaar data...`)
        const res = await this.makeRequest({
            url: "v2/skyblock/bazaar"
        })
        if (res.status == 200) {
            const json = await res.json()
            const prices = new Map()
            Object.entries(json.products).forEach(([id, data]) => {
                let buy = Math.ceil(data.quick_status.buyPrice)
                let sell = Math.ceil(data.quick_status.sellPrice)
                prices.set(id, [buy, sell])
            })
            this.cache.bazaar = prices
            this.log(`Bazaar data refreshed successfully. Cached Instant Buy/Sell prices: ${prices.size}`)
        }
        else {
            this.log(`Failed to refresh Bazaar data, request returned code ${res.status}.`)
        }
        this.sinceUpdate.bazaar = 0
    },
    async makeRequest({ url = "", method = "GET", body = "" }) {
        return await fetch(`https://api.hypixel.net/${url}`, {
            method: method,
            body: body & method != "GET" ? body : null,
            headers: {
                "API-Key": apiKey
            }
        })
    },
    parseItemData(data = "") {
        const buffer = Buffer.from(data, "base64")
        const gzip = zlib.gunzipSync(buffer)
        const nbt = nbtjs.read(gzip)
        return nbt.payload[""]["i"].shift()
    },
    parseRequestPath(path = "") {
        if (path.includes("?")) {
            path = path.split("?").shift()
        }
        if (path.endsWith("/")) {
            path = path.substring(0, path.length - 1)
        }
        return path
    },
    stringifyMap(map) {
        return JSON.stringify(Object.fromEntries(map))
    },
    logFile: `${__dirname}/log.txt`,
    log(message = "") {
        if (!config.logging) {
            return;
        }
        if (!fs.existsSync(this.logFile)) {
            fs.writeFileSync(this.logFile, ``)
        }
        fs.appendFileSync(this.logFile, `[${new Date().toISOString()}] ${message}\n`)
    }
}

setInterval(async () => {
    if (util.sinceUpdate.auctions == 7) { // refesh auction house every 4m
        await util.refreshAuctions()
    }
    else {
        util.sinceUpdate.auctions++
    }
    if (util.sinceUpdate.bazaar == 3) { // refesh bazaar every 2m
        await util.refreshBazaar()
    }
    else {
        util.sinceUpdate.bazaar++
    }
    Bun.gc()
}, 30000)

await util.refreshAuctions()
await util.refreshBazaar()

Bun.serve({
    async fetch(req, server) {
        const reqIP = config.cloudflareMode ? req.headers.get("cf-connecting-ip") : server.requestIP(req).address
        const path = util.parseRequestPath(new URL(req.url).pathname)
        const limiter = new Limiter(reqIP, path)
        if (req.method == "GET") {
            if (path == "/v1/economy/get-item-pricing") {
                if (limiter.limited(2)) {
                    return new Response("", { status: 429 })
                }
                limiter.add(60000)
                return new Response(JSON.stringify({
                    auction: util.stringifyMap(util.cache.auction),
                    bazaar: util.stringifyMap(util.cache.bazaar)
                }), {
                    headers: {
                        "Content-Type": "application/json"
                    }
                })
            }
        }
        return new Response("", { status: 400 })
    },
    development: false,
    port: config.port
})

util.log(config.cloudflareMode ? `Server running on port ${config.port}, Cloudflare mode enabled.` : `Server running on port ${config.port}.`)