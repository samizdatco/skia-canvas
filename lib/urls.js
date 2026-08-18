const url = require('url'),
      {http, https} = require('follow-redirects'),
      {HttpsProxyAgent} = require('https-proxy-agent')

const UA = {"User-Agent": "Skia Canvas"}
const PROXY_URL =
  process.env.https_proxy || process.env.HTTPS_PROXY ||
  process.env.http_proxy || process.env.HTTP_PROXY

const fetchURL = (url, opts, ok, fail) => {
  let proto = url.slice(0,5).split(':')[0],
      client = {http, https}[proto.toLowerCase()]

  if (!client){
    fail(new Error(`Unsupported protocol: expected 'http' or 'https' (got: ${proto})`))
  }else{
    opts ??= {}
    opts.headers = {...UA, ...opts.headers}
    opts.agent = opts.agent===undefined && PROXY_URL ? new HttpsProxyAgent(PROXY_URL) : opts.agent

    let req = client.request(url, opts, resp => {
      if (resp.statusCode < 200 || resp.statusCode >= 300){
        fail(new Error(`Failed to load image from "${url}" (HTTP error ${resp.statusCode})`))
      }else{
        const chunks = []
        resp.on("data", chunk => chunks.push(chunk))
        resp.on("end", () => ok(Buffer.concat(chunks)))
        resp.on('error', e => fail(e))
      }
    })

    // distinguish lapsed `timeout` deadlines from the user calling .abort() on a controller of their own
    const timedOut = (err, signal) => err.name == 'AbortError' && signal?.reason?.name == 'TimeoutError'
    req.on('timeout', () => req.destroy(new Error(`Timed out loading image from "${url}"`)))
    req.on('error', e => fail(
      timedOut(e, opts.signal) ? new Error(`Timed out loading image from "${url}"`, {cause:e}) : e
    ))
    if (opts.body) req.write(opts.body)
    req.end()
  }
}

const decodeDataURL = (dataURL, ok, fail) => {
  if (typeof dataURL!='string') return fail(TypeError(`Expected a data URL string (got ${typeof dataURL})`))
  let [header, mime, enc] = dataURL.slice(0, 40).match(/^\s*data:(?<mime>[^;]*);(?:charset=)?(?<enc>[^,]*),/) || []
  if (!mime || !enc) return fail(TypeError(`Expected a valid data URL string (got: "${dataURL}")`))

  // SVGs in particular may not be base64 encoded
  let content = dataURL.slice(header.length)
  if (enc.toLowerCase() != 'base64') content = decodeURIComponent(content)

  try{ ok(Buffer.from(content, enc)) }
  catch(e){ fail(e) }
}

const expandURL = (src) => {
  // convert URLs to strings, otherwise pass arg through unmodified
  if (src instanceof URL){
    if (src.protocol=='file:') src = url.fileURLToPath(src)
    else if (src.protocol.match(/^(https?|data):/)) src = src.href
    else throw Error(`Unsupported protocol: ${src.protocol.replace(':', '')}`)
  }
  return src
}

module.exports = {fetchURL, decodeDataURL, expandURL}
