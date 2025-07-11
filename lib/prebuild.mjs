import zlib from 'zlib'
import stream from 'stream'
import crypto from 'crypto'
import child_process from 'child_process'
import {createReadStream, createWriteStream} from 'fs'
import {readFile, writeFile, rm} from 'fs/promises'
import {resolve} from 'path'
import {promisify} from 'util'
import fetch from 'cross-fetch'
import {family} from 'detect-libc'
import { HttpsProxyAgent } from 'https-proxy-agent'

const pipeline = promisify(stream.pipeline)
const exec = promisify(child_process.exec);

const ROOT = resolve(`${import.meta.dirname}/..`)
const BINARY_HOST = "https://github.com/samizdatco/skia-canvas/releases/download"
const BINARY_PATH = `${ROOT}/lib/skia.node`
const PACKAGE_JSON = `${ROOT}/package.json`
const PROXY_URL = process.env.http_proxy || process.env.HTTP_PROXY || process.env.npm_config_proxy

async function snapshot(version){
    let json = (await exec(`gh release view v${version} --json assets`)).stdout,
        {assets} = JSON.parse(json),
        hashes = Object.fromEntries(assets.map(({name, digest}) => [name, digest]))
    exec(`npm pkg set prebuild='${JSON.stringify(hashes)}' --json`)
}

async function upload(version, triplet){
    let artifact = `${ROOT}/${triplet}.gz`

    try{
        await pipeline( createReadStream(BINARY_PATH), zlib.createGzip(), createWriteStream(artifact) )
        await exec(`gh release upload v${version} ${artifact}`)
    }catch(e){
        console.error(e.message)
        process.exit(1)
    }
}

async function download(version, triplet, prebuild){
    let url = `${BINARY_HOST}/v${version}/${triplet}.gz`,
        agent = PROXY_URL ? new HttpsProxyAgent(proxy) : undefined

    try{
        let {ok, status, body} = await fetch(url, {agent})
        if (!ok){
            if (status==404) throw Error(`Prebuilt library not found at "${url}" (HTTP error ${status})`)
            else throw Error(`Failed to load prebuilt binary from "${url}" (HTTP error ${status})`)
        }

        // write to /lib/skia.node while also hashing the .gz file
        let [digest, _] = await Promise.all([
            pipeline( body, crypto.createHash('sha256').setEncoding('hex'), digest => digest.toArray() ),
            pipeline( body, zlib.createGunzip(), createWriteStream(BINARY_PATH) )
        ])

        // verify hash if `prebuild` obj exists in package.json (i.e., this is a published module, not a repo copy)
        let official = (prebuild || {})[`${triplet}.gz`],
            actual = `sha256:${digest}`
        if (official && actual != official){
            await rm(BINARY_PATH, {force:true})
            throw Error(`Prebuilt library file '${triplet}.gz' failed integrity check\nDownloaded: ${url}\nExpected: ${official}\nReceived: ${actual}`)
        }
    }catch(e){
        // package.json's `install` script falls back to calling `make` on failure
        console.warn(e.message)
        console.log("\nAttempting to rebuild locally...")
        process.exit(1)
    }
}

function usage(version, triplet){
    console.log("usage: prebuild.mjs <action>")
    console.log("\nactions:")
    console.log(`  snapshot - add hashes of all release assets to package.json (for publishing)`)
    console.log(`  download - fetch precompiled /lib/skia.node appropriate for this platform (${triplet})`)
    console.log(`    upload - post this platform's skia.node to the ${version} release on GitHub`)
}

async function main(){
    let package_json = JSON.parse(await readFile(PACKAGE_JSON)),
        {platform, arch} = process,
        libc = await family()

    let cmd = process.argv[2],
        {version, prebuild} = package_json,
        triplet = [platform, arch, libc].filter(t=>t).join('-')

    await ({upload, download, snapshot}[cmd] || usage)(version, triplet, prebuild)
}
main()
