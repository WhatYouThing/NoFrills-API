import * as fs from "fs"
import nbtjs from "nbt-js"
import { URL } from "node:url"
import zlib from "zlib"

const config = await Bun.file(`${__dirname}/config.json`).json()
const apiKey = !config.apiKey ? process.env.HYPIXEL_API_KEY : config.apiKey

class Limiter {
    ip; endpoint; key;

    constructor(ip = "", endpoint = "") {
        this.ip = ip;
        this.endpoint = endpoint;
        this.key = `${ip}+${endpoint}`
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
        bazaar: 0,
        npc: 0
    },
    cache: {
        auction: new Map(),
        bazaar: new Map(),
        attribute: new Map(),
        npc: new Map()
    },
    rateLimits: new Map(),
    async refreshAuctions() {
        let maxPages = 200
        let auctions = []
        this.sinceUpdate.auctions = new Date().getTime()
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
        this.log(`Auction House data refreshed successfully. Cached LBIN prices: ${prices.size}, Total pages: ${maxPages}.`)
    },
    async refreshBazaar() {
        this.sinceUpdate.bazaar = new Date().getTime()
        this.log(`Refreshing Bazaar data...`)
        const res = await this.makeRequest({
            url: "v2/skyblock/bazaar"
        })
        if (res.status == 200) {
            const json = await res.json()
            const prices = new Map()
            Object.entries(json.products).forEach(([id, data]) => {
                let buy = data.buy_summary[0] ? data.buy_summary[0].pricePerUnit : 0
                let sell = data.sell_summary[0] ? data.sell_summary[0].pricePerUnit : 0
                prices.set(id, [buy, sell])
            })
            this.cache.bazaar = prices
            this.log(`Bazaar data refreshed successfully. Cached Instant Buy/Sell prices: ${prices.size}`)
        }
        else {
            this.log(`Failed to refresh Bazaar data, request returned code ${res.status}.`)
        }
    },
    async refreshNPC() {
        this.sinceUpdate.npc = new Date().getTime()
        this.log(`Refreshing NPC price data...`)
        const res = await this.makeRequest({
            url: "v2/resources/skyblock/items"
        })
        if (res.status == 200) {
            const json = await res.json()
            const prices = new Map()
            await Promise.all(json.items.map(item => {
                const coins = item.npc_sell_price
                const motes = item.motes_sell_price
                let pricing = {}
                if (typeof coins == "number") {
                    pricing.coin = coins
                }
                if (typeof motes == "number") {
                    pricing.mote = motes
                }
                if (typeof pricing.coin == "number" || typeof pricing.mote == "number") {
                    prices.set(item.id, pricing)
                }
            }))
            this.cache.npc = prices
            this.log(`NPC price data refreshed successfully. Cached prices: ${prices.size}`)
        }
        else {
            this.log(`Failed to refresh NPC price data, request returned code ${res.status}.`)
        }
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
    const time = new Date().getTime()
    if (time - util.sinceUpdate.auctions >= 240000) {
        await util.refreshAuctions()
    }
    if (time - util.sinceUpdate.bazaar >= 120000) {
        await util.refreshBazaar()
    }
    if (time - util.sinceUpdate.npc >= 1800000) {
        await util.refreshNPC()
    }
}, 1000)

Bun.serve({
    async fetch(req, server) {
        const reqIP = config.cloudflareMode ? req.headers.get("cf-connecting-ip") : server.requestIP(req).address
        const path = util.parseRequestPath(new URL(req.url).pathname)
        const limiter = new Limiter(reqIP, path)
        if (req.method == "GET") {
            if (path == "/v1/economy/get-item-pricing") {
                if (limiter.limited(3)) {
                    return new Response("", { status: 429 })
                }
                limiter.add(60000)
                return new Response(JSON.stringify({
                    auction: util.stringifyMap(util.cache.auction),
                    bazaar: util.stringifyMap(util.cache.bazaar),
                    attribute: util.stringifyMap(util.cache.attribute),
                    npc: util.stringifyMap(util.cache.npc),
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
    port: config.port,
    hostname: "127.0.0.1"
})

util.log(config.cloudflareMode ? `Server running on port ${config.port}, Cloudflare mode enabled.` : `Server running on port ${config.port}.`)