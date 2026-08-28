import zlib from 'zlib'
import stream from 'stream'
import crypto from 'crypto'
import child_process from 'child_process'
import {createReadStream, createWriteStream, existsSync} from 'fs'
import {readFile, writeFile, rm, mkdir, copyFile, unlink} from 'fs/promises'
import {resolve, dirname, extname} from 'path'
import {promisify} from 'util'
import {fileURLToPath} from 'url'
import {family} from 'detect-libc'
import https from 'follow-redirects/https.js'
import {HttpsProxyAgent} from 'https-proxy-agent'

const pipeline = promisify(stream.pipeline)
const exec = promisify(child_process.exec);

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const REPO_URL = "https://github.com/samizdatco/skia-canvas"
const BINARY_HOST = `${REPO_URL}/releases/download`
const BINARY_PATH = `${ROOT}/lib/skia.node`
const PACKAGE_JSON = `${ROOT}/package.json`
const PROXY_URL =
  process.env.https_proxy || process.env.HTTPS_PROXY ||
  process.env.http_proxy || process.env.HTTP_PROXY ||
  process.env.npm_config_https_proxy || process.env.npm_config_proxy

const NPM_SCOPE = '@skia-canvas'
const PLATFORMS = [
  {os:'darwin', cpu:'arm64'},
  {os:'darwin', cpu:'x64'},
  {os:'linux',  cpu:'arm64', libc:'glibc'},
  {os:'linux',  cpu:'arm64', libc:'musl'},
  {os:'linux',  cpu:'x64',   libc:'glibc'},
  {os:'linux',  cpu:'x64',   libc:'musl'},
  {os:'win32',  cpu:'arm64'},
  {os:'win32',  cpu:'x64'},
].map(({os, cpu, libc}) => (
  {os, cpu, libc, triplet:[os, cpu, libc].filter(t=>t).join('-')}
))

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

async function collect(...args){
    let {version} = await config(),
        destDir = resolve(args.find(a => !a.startsWith('--')) || `${ROOT}/assets`)

    // pull every prebuilt <triplet>.gz off the GitHub release for `packages` to assemble from
    await exec(`gh release download v${version} --pattern '*.gz' --dir "${destDir}"`)

    // record each asset's digest in package.json so `download` can integrity-check fetched binaries
    let {assets} = JSON.parse((await exec(`gh release view v${version} --json assets`)).stdout),
        hashes = Object.fromEntries(assets.map(({name, digest}) => [name, digest]))
    await exec(`npm pkg set prebuild='${JSON.stringify(hashes)}' --json`)
    console.log(`Downloaded prebuilt binaries to ${destDir} and recorded ${assets.length} asset hashes`)
}

async function packages(...args){
    let {version, triplet} = await config(),
        local = args.includes('--local'),
        srcDir = resolve(args.find(a => !a.startsWith('--')) || `${ROOT}/assets`),
        npmDir = `${srcDir}/npm`,
        built = []

    await rm(npmDir, {recursive:true, force:true})

    for (const plat of PLATFORMS){
        let pkgDir = `${npmDir}/${plat.triplet}`,
            asset = `${srcDir}/${plat.triplet}.gz`

        if (local){ // package this platform's lib/skia.node only (for testing the pipeline in CI)
            if (plat.triplet != triplet) continue
            if (!existsSync(BINARY_PATH)) throw Error(`No local binary at ${BINARY_PATH} (run \`npm run build\` first)`)
            await mkdir(pkgDir, {recursive:true})
            await copyFile(BINARY_PATH, `${pkgDir}/skia.node`)
        }else{
            if (!existsSync(asset)) continue
            await mkdir(pkgDir, {recursive:true})
            await pipeline( createReadStream(asset), zlib.createGunzip(), createWriteStream(`${pkgDir}/skia.node`) )
        }

        let manifest = {
            name: `${NPM_SCOPE}/${plat.triplet}`,
            version,
            description: `Prebuilt skia-canvas native binary (${plat.triplet})`,
            os: [plat.os],
            cpu: [plat.cpu],
            ...(plat.libc ? {libc: [plat.libc]} : {}),
            main: "skia.node",
            files: ["skia.node"],
            license: "MIT",
            homepage: "https://skia-canvas.org",
            repository: {type: "git", url: "git+https://github.com/samizdatco/skia-canvas.git"},
            bugs: {url: `${REPO_URL}/issues`},
            publishConfig: {access: "public"},
        }
        await writeFile(`${pkgDir}/package.json`, JSON.stringify(manifest, null, 2) + '\n')
        built.push(plat)
        console.log(`Assembled ${manifest.name}@${version} in assets/npm/${plat.triplet}`)
    }

    // for non-testing runs, ensure no platforms are missing then add optionalDependencies to package.json
    if (!local){
        let missing = PLATFORMS.filter(p => !built.includes(p)).map(p => `${p.triplet}.gz`)
        if (missing.length){
            console.error(`Missing release assets in ${srcDir}: ${missing.join(', ')}`)
            process.exit(1)
        }
        let deps = Object.fromEntries(built.map(p => [`${NPM_SCOPE}/${p.triplet}`, version]))
        await exec(`npm pkg set optionalDependencies='${JSON.stringify(deps)}' --json`)
        console.log(`Added ${built.length} ${NPM_SCOPE}/* packages as optionalDependencies`)
    }
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
        let official = prebuild?.[`${triplet}.gz`],
            actual = sha.digest
        if (official && actual != official){
            await rm(BINARY_PATH, {force:true})
            throw Error(`Prebuilt library file '${triplet}.gz' failed integrity check\nDownloaded: ${url}\nExpected: ${official}\nReceived: ${actual}`)
        }
    }catch(e){
        console.warn(e.message)

        // optionally fall back to compiling locally
        if (!args.includes('--or-compile') || !existsSync(`${ROOT}/Cargo.toml`)) process.exit(1)
        else return compile('--fallback')
    }
}

async function compile(...args){
  let optimization = args.includes('custom') || args.includes('dev') ? '' : "--release",
      customFeatures = args.includes('custom') && (args[args.indexOf('custom')+1] || '').replace(/[^[a-z0-9\_\-\,]/g, ''),
      featureList = args.includes('custom') ? (customFeatures || '') : CARGO_FEATURES,
      features = `--features "${featureList}"`,
      isFallback = args.includes('--fallback'),
      isSrcRepo = existsSync(`${ROOT}/Cargo.toml`)

  if (!isSrcRepo) throw Error(`Cannot compile from npm version of skia-canvas: clone source from ${REPO_URL}`)
  else if (isFallback) console.log("\nAttempting to rebuild locally...")
  else console.warn(`cargo build ${[optimization, features].filter(s=>s).join(' ')}`)

  let cargoArgs = [
    ...(optimization ? [optimization] : []),
    ...(featureList ? ["--features", featureList] : []),
  ]

  process.exit(await runCargo(BINARY_PATH, cargoArgs))
}

// Run `cargo build`, move the compiled lib to `outputFile`, and resolve with outcome:
// 0 = success, 1 = build error or skipped/stale post-compile copy operation
function runCargo(outputFile, cargoArgs){
  const CRATE = "skia_canvas"   // cargo normalizes `-`→`_` in target.name
  const KIND  = "cdylib"

  return new Promise(resolve => {
    // machine JSON on stdout (captured); pretty diagnostics on stderr (inherited)
    let cargo = child_process.spawn(
      "cargo",
      ["build", "--message-format=json-render-diagnostics", ...cargoArgs],
      {stdio:["inherit", "pipe", "inherit"]}
    )

    cargo.on("error", err => {
      console.error(err.code === "ENOENT"
        ? "Error: could not find the `cargo` executable.\nInstall Rust: https://www.rust-lang.org/tools/install"
        : err)
      resolve(1)
    })

    let json = ""
    cargo.stdout.setEncoding("utf8")
    cargo.stdout.on("data", chunk => { json += chunk })

    // `close` fires after stdout has drained, so `json` is complete.
    cargo.on("close", async (code, signal) => {
      if (signal){ console.error(`cargo was terminated by signal ${signal}`); return resolve(1) }
      if (code) return resolve(code)

      let src = null
      for (let line of json.split("\n")){
        if (!line) continue
        let msg; try { msg = JSON.parse(line) } catch { continue }
        if (msg.reason !== "compiler-artifact") continue
        if (msg.target?.name?.replaceAll('-', '_') !== CRATE) continue
        let i = (msg.target.kind ?? []).indexOf(KIND)
        if (i >= 0 && msg.filenames?.[i]) src = msg.filenames[i]
      }
      if (!src){
        console.error(`cargo did not emit a ${KIND} artifact for crate "${CRATE}"`)
        return resolve(1)
      }

      // copy to lib/skia.node but delete the old copy first so macOS re-signs it (neon#911)
      try {
        let dir = dirname(outputFile)
        if (dir && dir !== ".") await mkdir(dir, {recursive:true})
        if (extname(outputFile) === ".node"){ try { await unlink(outputFile) } catch {} }
        await copyFile(src, outputFile)
        resolve(0)
      } catch (err) {
        console.error(err)
        resolve(1)
      }
    })
  })
}

async function usage(){
    let {version, triplet} = await config()
    console.log("usage: prebuild.mjs <action>")
    console.log("\nactions:")
    console.log(`    compile - build /lib/skia.node from source using locally installed rustc`)
    console.log(`   download - fetch precompiled /lib/skia.node appropriate for this platform (${triplet})`)
    console.log(`     upload - post this platform's skia.node to the v${version} release on GitHub`)
    console.log(`    collect - download the release's <triplet>.gz assets into ./assets and record`)
    console.log(`              their hashes in package.json (for publishing)`)
    console.log(`   packages - assemble the ${NPM_SCOPE}/* platform packages in assets/npm/ from release assets`)
    console.log(`              and pin them as optionalDependencies in package.json (for publishing);`)
    console.log(`              pass --local to make a test package from /lib/skia.node (for CI testing)`)
}

async function main(){
    let cmd = process.argv[2],
        args = process.argv.slice(3)

    await ({upload, download, collect, compile, packages}[cmd] || usage)(...args)
}
main()
