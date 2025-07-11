import zlib from 'zlib'
import stream from 'stream'
import child_process from 'child_process'
import {createReadStream, createWriteStream} from 'fs'
import {readFile} from 'fs/promises'
import {resolve} from 'path'
import {promisify} from 'util'
import fetch from 'cross-fetch'
import {family} from 'detect-libc'

const pipeline = promisify(stream.pipeline)
const exec = promisify(child_process.exec);
const ROOT = resolve(`${import.meta.dirname}/..`)

const BINARY_HOST = "https://github.com/samizdatco/skia-canvas/releases/download"
const BINARY_PATH = `${ROOT}/lib/skia.node`
const PACKAGE_JSON = `${ROOT}/package.json`

async function getOpts(){
    let package_json = JSON.parse(await readFile(PACKAGE_JSON)),
        {version} = package_json,
        {platform, arch} = process,
        libc = await family(),
        triplet = [platform, arch, libc].filter(t=>t).join('-')

    return { version, platform, arch, libc, triplet }
}

async function upload(){
    let {version, triplet} = await getOpts(),
        artifact = `${ROOT}/${triplet}.gz`

    try{
        await pipeline( createReadStream(BINARY_PATH), zlib.createGzip(), createWriteStream(artifact) )
        await exec(`gh release upload v${version} ${artifact}`)
    }catch(e){
        console.warn(e.message)
        process.exit(1)
    }
}

async function download(){
    let {version, triplet} = await getOpts()
    let url = `${BINARY_HOST}/v${version}/${triplet}.gz`

    return fetch(url)
        .then(resp => {
            if (resp.ok) return pipeline(
                resp.body, zlib.createGunzip(), createWriteStream(BINARY_PATH)
            )

            if (resp.status==404) console.warn(`Prebuilt library not found at "${url}" (HTTP error ${resp.status})`)
            else console.warn(`Failed to load prebuilt binary from "${url}" (HTTP error ${resp.status})`)
            console.log("Attempting to rebuild locally...")
            process.exit(1)
        })
}

async function main(){
    let verb = process.argv[2]
    if (verb=='upload') await upload()
    else if (verb=='download') await download()
}
main()