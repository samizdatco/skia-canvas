import zlib from 'zlib'
import stream from 'stream'
import crypto from 'crypto'
import child_process from 'child_process'
import {createReadStream, createWriteStream, existsSync} from 'fs'
import {readFile, writeFile, rm} from 'fs/promises'
import {resolve} from 'path'
import {promisify} from 'util'
import {family} from 'detect-libc'
import https from 'follow-redirects/https.js'
import {HttpsProxyAgent} from 'https-proxy-agent'

const pipeline = promisify(stream.pipeline)
const exec = promisify(child_process.exec);

const ROOT = resolve(`${import.meta.dirname}/..`)
const REPO_URL = "https://github.com/samizdatco/skia-canvas"
const BINARY_HOST = `${REPO_URL}/releases/download`
const BINARY_PATH = `${ROOT}/lib/skia.node`
const PACKAGE_JSON = `${ROOT}/package.json`
const PROXY_URL =
  process.env.https_proxy || process.env.HTTPS_PROXY ||
  process.env.http_proxy || process.env.HTTP_PROXY ||
  process.env.npm_config_https_proxy || process.env.npm_config_proxy

const CARGO_FEATURES = {
  darwin: "metal,window",
  linux: "vulkan,window,freetype",
  win32: "vulkan,window",
}[process.platform]

class Hasher extends stream.Transform {
  #digest
  constructor(options) {
    super(options)
    this.hash = crypto.createHash('sha256')
  }
  _transform(chunk, encoding, callback) {
    this.hash.update(chunk)
    this.push(chunk)
    callback()
  }
  get digest(){
    this.#digest = this.#digest || `sha256:${this.hash.digest('hex')}`
    return this.#digest
  }
}

async function config(){
  let package_json = JSON.parse(await readFile(PACKAGE_JSON)),
      {platform, arch} = process,
      libc = await family()

  let {version, prebuild} = package_json,
      triplet = [platform, arch, libc].filter(t=>t).join('-')

  return {version, triplet, prebuild}
}

async function snapshot(){
    let {version} = await config(),
        json = (await exec(`gh release view v${version} --json assets`)).stdout,
        {assets} = JSON.parse(json),
        hashes = Object.fromEntries(assets.map(({name, digest}) => [name, digest]))
    exec(`npm pkg set prebuild='${JSON.stringify(hashes)}' --json`)
}

async function upload(){
    let {version, triplet} = await config(),
        artifact = `${ROOT}/${triplet}.gz`

    try{
        await pipeline( createReadStream(BINARY_PATH), zlib.createGzip(), createWriteStream(artifact) )
        await exec(`gh release upload v${version} ${artifact}`)
    }catch(e){
        console.error(e.message)
        process.exit(1)
    }
}

async function download(...args){
    if (existsSync(BINARY_PATH)) return // nothing to be done if skia.node already exists

    let {version, triplet, prebuild} = await config(),
        url = `${BINARY_HOST}/v${version}/${triplet}.gz`,
        agent = PROXY_URL ? new HttpsProxyAgent(PROXY_URL) : undefined

    try{
        let body = await new Promise((res, rej) => {
          https.get(url, {agent}, resp => {
            let {statusCode:status} = resp
            if (status == 404) rej(Error(`Prebuilt library not found at "${url}" (HTTP error ${status})`))
            else if (status < 200 || status >= 300) rej(Error(`Failed to load prebuilt binary from "${url}" (HTTP error ${status})`))
            else res(resp)
          })
        })
        console.log(`Fetched prebuilt libary from "${url}"`)

        // write to /lib/skia.node while also hashing the .gz file
        let sha = new Hasher()
        let gunzip = zlib.createGunzip()
        await pipeline(body, sha, gunzip, createWriteStream(BINARY_PATH))

        // verify hash if `prebuild` obj exists in package.json (i.e., this is a published module, not a repo copy)
        let official = (prebuild || {})[`${triplet}.gz`],
            actual = sha.digest
        if (official && actual != official){
            await rm(BINARY_PATH, {force:true})
            throw Error(`Prebuilt library file '${triplet}.gz' failed integrity check\nDownloaded: ${url}\nExpected: ${official}\nReceived: ${actual}`)
        }
    }catch(e){
        console.warn(e.message)

        // optionally fall back to compiling locally
        if (!args.includes('--or-compile') || !existsSync(`${ROOT}/Cargo.toml`)) process.exit(1)
        else compile('--fallback')
    }
}

function compile(...args){
  let optimization = args.includes('custom') || args.includes('dev') ? '' : "--release",
      customFeatures = args.includes('custom') && (args[args.indexOf('custom')+1] || '').replace(/[^[a-z0-9\_\-\,]/g, ''),
      features = `--features "${args.includes('custom') ? customFeatures || '' : CARGO_FEATURES}"`,
      isFallback = args.includes('--fallback'),
      isSrcRepo = existsSync(`${ROOT}/Cargo.toml`)

  if (!isSrcRepo) throw Error(`Cannot compile from npm version of skia-canvas: clone source from ${REPO_URL}`)
  else if (isFallback) console.log("\nAttempting to rebuild locally...")
  else console.warn(`cargo build ${[optimization, features].filter(s=>s).join(' ')}`)

  let {status} = child_process.spawnSync(
    `cargo-cp-artifact -nc ${BINARY_PATH} -- cargo build --message-format=json-render-diagnostics ${optimization} ${features}`,
    {shell:true, stdio:'inherit'}
  )

  process.exit(status)
}

async function usage(){
    let {triplet} = await config()
    console.log("usage: prebuild.mjs <action>")
    console.log("\nactions:")
    console.log(`   compile - build /lib/skia.node from source using locally installed rustc`)
    console.log(`  download - fetch precompiled /lib/skia.node appropriate for this platform (${triplet})`)
    console.log(`    upload - post this platform's skia.node to the ${version} release on GitHub`)
    console.log(`  snapshot - add hashes of all uploaded assets to package.json (for publishing)`)
}

async function main(){
    let cmd = process.argv[2],
        args = process.argv.slice(3)

    await ({upload, download, snapshot, compile}[cmd] || usage)(...args)
}
main()
