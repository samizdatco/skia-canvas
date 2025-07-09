import {readFile, writeFile, mkdir as mkdirFs} from 'fs/promises'
import {resolve} from 'path'
import {promisify} from 'util'
import zlib from 'zlib'
import child_process from 'child_process'
import {family} from 'detect-libc'
import fetch from 'cross-fetch'

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
        let data = await readFile(BINARY_PATH),
            gzData = await zlib.gzipSync(data)
        await writeFile(artifact, gzData)
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
            if (resp.ok) return resp.arrayBuffer()

            if (resp.status==404) console.warn(`Prebuilt library not found at "${url}" (HTTP error ${resp.status})`)
            else console.warn(`Failed to load prebuilt binary from "${url}" (HTTP error ${resp.status})`)
            console.log("Attempting to rebuild locally...")
            process.exit(1)
        }).then(buf =>
            writeFile(BINARY_PATH, zlib.gunzipSync(buf))
        )
}

async function main(){
    let verb = process.argv[2]
    if (verb=='upload') await upload()
    else if (verb=='download') await download()
}
main()