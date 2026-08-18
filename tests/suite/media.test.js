// @ts-check

"use strict"

// urls.js reads the proxy env vars once, at load time, and would route requests bound for
// the local test server through a proxy instead, so clear them before requiring the library
for (const v of ['https_proxy', 'HTTPS_PROXY', 'http_proxy', 'HTTP_PROXY']) delete process.env[v]

const path = require('path'),
      os = require('os'),
      fs = require('fs'),
      http = require('http'),
      {assert} = require('../runner/assert'), 
      {describe, test, before, after, beforeEach, afterEach} = require('node:test'),
      {pathToFileURL, fileURLToPath} = require('url'),
      {Canvas, Image, ImageData, FontLibrary, loadImage, loadImageData} = require('../../lib')

// serve the repo's files over http so the url-loading tests have a real server to talk to
const PORT = 41777,
      HOST = `http://127.0.0.1:${PORT}`

const server = http.createServer((req, res) => {
  if (req.url == '/stall') return              // accept the connection, then never respond
  if (req.url == '/stall-mid-body'){           // send headers and some bytes, then stop
    res.writeHead(200)
    return void res.write('partial')
  }

  try{
    const body = fs.readFileSync(process.cwd() + req.url) // read before writing the header,
    res.writeHead(200)                                    // so a miss can still send a 404
    res.end(body)
  }catch(e){
    res.writeHead(404)
    res.end(`Failed to load image from "${req.url}" (HTTP error 404)`)
  }
})

before(() => new Promise((resolve, reject) => {
  server.once('error', (/** @type {NodeJS.ErrnoException} */ e) => reject(e.code == 'EADDRINUSE'
    ? Error(`Port ${PORT} is in use; the media tests need it to serve image fixtures`)
    : e
  ))
  server.listen(PORT, '127.0.0.1', () => resolve(undefined))
}))

after(() => new Promise(resolve => {
  server.closeAllConnections() // keep-alive sockets would otherwise stall the close
  server.close(resolve)
}))

describe("Image", () => {
  var PATH = 'tests/assets/pentagon.png',
      URI = `${HOST}/${PATH}`,
      BUFFER = fs.readFileSync(PATH),
      DATA_URI = `data:image/png;base64,${BUFFER.toString('base64')}`,
      FILE_URL = pathToFileURL(PATH),
      FRESH = {complete:false, width:0, height:0},
      LOADED = {complete:true, width:125, height:125},
      FORMAT = 'tests/assets/image/format',
      PARSED = {complete:true, width:60, height:60},
      SVG_PATH = `${FORMAT}.svg`,
      SVG_URI = `${HOST}/${SVG_PATH}`,
      SVG_BUFFER = fs.readFileSync(SVG_PATH),
      SVG_DATA_URI = `data:image/svg;base64,${SVG_BUFFER.toString('base64')}`,
      SVG_FILE_URL = pathToFileURL(SVG_PATH),
      img

  beforeEach(() => img = new Image() )

  describe("can initialize bitmaps from", () => {
    test("buffer", async () => {
      img = new Image(BUFFER)
      assert.matchesSubset(img, LOADED)
      assert.equal(img.src, "::Buffer::")

      let fakeSrc = 'arbitrary*src*string'
      img = new Image(BUFFER, fakeSrc)
      assert.equal(img.src, fakeSrc)

      img = new Image()
      img.src = BUFFER
      assert.matchesSubset(img, LOADED)
    })

    test("data uri", () => {
      img.src = DATA_URI
      assert.matchesSubset(img, LOADED)

      img = new Image(DATA_URI)
      assert.matchesSubset(img, LOADED)
      assert.equal(img.src, DATA_URI)

      let fakeSrc = 'arbitrary*src*string'
      img = new Image(DATA_URI, fakeSrc)
      assert.equal(img.src, fakeSrc)
    })

    test("local file", async () => {
      assert.matchesSubset(img, FRESH)
      img.src = PATH
      await img.decode()
      assert.matchesSubset(img, LOADED)
      assert.equal(img.src, PATH)

      assert.throws(() => new Image(PATH), /Expected a valid data URL/)
    })

    test("file url", async () => {
      assert.matchesSubset(img, FRESH)
      img.src = FILE_URL
      await img.decode()
      assert.matchesSubset(img, LOADED)
      assert.equal(img.src, fileURLToPath(FILE_URL))

      assert.throws(() => new Image(FILE_URL), /Expected a valid data URL/)
    })

    test("http url", (t, done) => {
      assert.matchesSubset(img, FRESH)
      img.onload = loaded => {
        assert.equal(loaded, img)
        assert.matchesSubset(img, LOADED)
        done()
      }
      img.src = URI

      assert.throws(() => new Image(URI), /Expected a valid data URL/)
    })

    test("loadImage call", async () => {
      assert.matchesSubset(img, FRESH)

      img = await loadImage(URI)
      assert.matchesSubset(img, LOADED)

      img = await loadImage(BUFFER)
      assert.matchesSubset(img, LOADED)

      img = await loadImage(DATA_URI)
      assert.matchesSubset(img, LOADED)

      img = await loadImage(PATH)
      assert.matchesSubset(img, LOADED)

      img = await loadImage(SVG_PATH)
      assert.matchesSubset(img, PARSED)

      img = await loadImage(new URL(URI))
      assert.matchesSubset(img, LOADED)

      img = await loadImage(new URL(DATA_URI))
      assert.matchesSubset(img, LOADED)

      img = await loadImage(pathToFileURL(PATH))
      assert.matchesSubset(img, LOADED)

      img = await loadImage(pathToFileURL(SVG_PATH))
      assert.matchesSubset(img, PARSED)

      await assert.rejects(loadImage(`${HOST}/nonesuch`), /HTTP error 404/)
    })

    test("request timeouts", async () => {
      // socket-inactivity timeout, before and after the response starts
      await assert.rejects(loadImage(`${HOST}/stall`, {timeout:100}), /Timed out/)
      await assert.rejects(loadImage(`${HOST}/stall-mid-body`, {timeout:100}), /Timed out/)

      // a deadline covers the whole request, including a response already in flight
      await assert.rejects(loadImage(`${HOST}/stall`, {signal:AbortSignal.timeout(100)}), /Timed out/)
      await assert.rejects(loadImage(`${HOST}/stall-mid-body`, {signal:AbortSignal.timeout(100)}), /Timed out/)

      // an explicit abort is not a timeout and keeps its own error
      let ac = new AbortController(),
          req = loadImage(`${HOST}/stall`, {signal:ac.signal})
      ac.abort()
      await assert.rejects(req, /aborted/)

      // neither option disturbs a request that finishes in time
      assert.matchesSubset(await loadImage(URI, {timeout:5000}), LOADED)
    })
  })

  describe("can initialize SVGs from", () => {
    test("buffer", () => {
      assert.matchesSubset(img, FRESH)
      img = new Image(SVG_BUFFER)
      assert.matchesSubset(img, PARSED)

      img = new Image()
      img.src = SVG_BUFFER
      assert.matchesSubset(img, PARSED)
    })

    test("data uri", async () => {
      assert.matchesSubset(img, FRESH)
      img.src = SVG_DATA_URI
      assert.matchesSubset(img, PARSED)
    })

    test("local file", async () => {
      assert.matchesSubset(img, FRESH)
      img.src = SVG_PATH
      assert(!img.complete)
      await img.decode()
      assert.matchesSubset(img, PARSED)
    })

    test("file url", async () => {
      assert.matchesSubset(img, FRESH)
      img.src = SVG_FILE_URL
      assert(!img.complete)
      await img.decode()
      assert.matchesSubset(img, PARSED)
    })

    test("http url", (t, done) => {
      assert.matchesSubset(img, FRESH)
      img.onload = loaded => {
        assert.equal(loaded, img)
        assert.matchesSubset(img, PARSED)
        done()
      }
      img.src = SVG_URI
      assert(!img.complete)
    })
  })

  describe("resolves SVG <style> CSS", () => {
    // Skia's SVG DOM ignores <style>/CSS selectors (only presentation & inline-style attrs), so
    // skia-canvas resolves the cascade itself before rendering (see src/image.rs). These tests
    // render each selector/cascade construction and check the fill actually landed. Behavior was
    // cross-checked against Chrome. `rgb(0,170,0)` = the "styled" fill; unstyled rects fall back
    // to SVG's default black.
    const GREEN = 'rgb(0,170,0)', RED = 'rgb(200,0,0)'
    const render = async (body, x = 20, y = 20) => {
      let svg = Buffer.from(`<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">${body}</svg>`)
      let image = await loadImage(svg)
      let canvas = new Canvas(40, 40), ctx = canvas.getContext('2d')
      ctx.drawImage(image, 0, 0)
      return Array.from(ctx.getImageData(x, y, 1, 1).data)
    }
    const isGreen = px => px[3] > 250 && px[1] > 150 && px[0] < 60 && px[2] < 60

    test("type, class, id & attribute selectors", async () => {
      const cases = {
        'type':               `<style>rect{fill:${GREEN}}</style><rect width="40" height="40"/>`,
        'class':              `<style>.p{fill:${GREEN}}</style><rect class="p" width="40" height="40"/>`,
        'id':                 `<style>#r{fill:${GREEN}}</style><rect id="r" width="40" height="40"/>`,
        'universal':          `<style>*{fill:${GREEN}}</style><rect width="40" height="40"/>`,
        'descendant':         `<style>g rect{fill:${GREEN}}</style><g><rect width="40" height="40"/></g>`,
        'child (>)':          `<style>g>rect{fill:${GREEN}}</style><g><rect width="40" height="40"/></g>`,
        'attribute [a]':      `<style>rect[data-x]{fill:${GREEN}}</style><rect data-x="1" width="40" height="40"/>`,
        'attribute [a=v]':    `<style>rect[role=box]{fill:${GREEN}}</style><rect role="box" width="40" height="40"/>`,
        'attribute [a~=v]':   `<style>[class~=p]{fill:${GREEN}}</style><rect class="a p b" width="40" height="40"/>`,
        'grouped selector':   `<style>circle, rect{fill:${GREEN}}</style><rect width="40" height="40"/>`,
      }
      for (const [name, body] of Object.entries(cases)){
        assert(isGreen(await render(body)), `${name} selector did not apply`)
      }
    })

    test("adjacent-sibling combinator (+)", async () => {
      // adjacent-sibling `rect + rect` targets only the SECOND of two stacked rects
      let adj = `<style>rect+rect{fill:${GREEN}}</style>` +
                `<rect x="0" y="0" width="40" height="20"/><rect x="0" y="20" width="40" height="20"/>`
      assert(isGreen(await render(adj, 20, 30)), 'adjacent sibling did not apply to the 2nd rect')
      assert(!isGreen(await render(adj, 20, 10)), 'adjacent sibling wrongly applied to the 1st rect')

      // `:first-child` targets only the FIRST of two side-by-side rects in a <g>
      let fc = `<style>rect:first-child{fill:${GREEN}}</style>` +
               `<g><rect x="0" y="0" width="20" height="40"/><rect x="20" y="0" width="20" height="40"/></g>`
      assert(isGreen(await render(fc, 10, 20)), ':first-child did not apply to the 1st rect')
      assert(!isGreen(await render(fc, 30, 20)), ':first-child wrongly applied to the 2nd rect')
    })

    test("cascade & specificity", async () => {
      // inline style beats a stylesheet rule
      assert(isGreen(await render(
        `<style>.x{fill:${RED}}</style><rect class="x" style="fill:${GREEN}" width="40" height="40"/>`)),
        'inline style should beat the stylesheet')
      // a stylesheet rule beats a presentation attribute
      assert(isGreen(await render(
        `<style>.x{fill:${GREEN}}</style><rect class="x" fill="${RED}" width="40" height="40"/>`)),
        'stylesheet should beat the presentation attribute')
      // higher specificity wins (#id over .class)
      assert(isGreen(await render(
        `<style>.x{fill:${RED}} #r{fill:${GREEN}}</style><rect id="r" class="x" width="40" height="40"/>`)),
        'id selector should outrank class selector')
      // equal specificity → later rule wins
      assert(isGreen(await render(
        `<style>.x{fill:${RED}} .x{fill:${GREEN}}</style><rect class="x" width="40" height="40"/>`)),
        'the later of two equal-specificity rules should win')
      // stylesheet inside a CDATA section is still honored
      assert(isGreen(await render(
        `<style><![CDATA[ .x{fill:${GREEN}} ]]></style><rect class="x" width="40" height="40"/>`)),
        'CDATA-wrapped stylesheet should be honored')
    })

    test(":nth-child & :nth-of-type", async () => {
      // four side-by-side rects (each 10 wide); centers at x = 5, 15, 25, 35
      const rects = `<rect x="0" width="10" height="40"/><rect x="10" width="10" height="40"/>` +
                    `<rect x="20" width="10" height="40"/><rect x="30" width="10" height="40"/>`
      const strip = rule => `<style>${rule}</style><g>${rects}</g>`

      let odd = strip(`rect:nth-child(odd){fill:${GREEN}}`)
      assert(isGreen(await render(odd, 5, 20)) && isGreen(await render(odd, 25, 20)), ':nth-child(odd) should hit the 1st & 3rd')
      assert(!isGreen(await render(odd, 15, 20)) && !isGreen(await render(odd, 35, 20)), ':nth-child(odd) should skip the 2nd & 4th')

      let last = strip(`rect:last-child{fill:${GREEN}}`)
      assert(isGreen(await render(last, 35, 20)) && !isGreen(await render(last, 5, 20)), ':last-child should hit only the 4th')

      let anb = strip(`rect:nth-child(2n){fill:${GREEN}}`)
      assert(isGreen(await render(anb, 15, 20)) && isGreen(await render(anb, 35, 20)), ':nth-child(2n) should hit the 2nd & 4th')

      // a non-rect sibling shifts child indices but not of-type indices: children are rect, circle, rect
      let ofType = `<style>rect:nth-of-type(2){fill:${GREEN}}</style><g>` +
        `<rect x="0" width="10" height="40"/><circle cx="15" cy="20" r="3"/><rect x="20" width="10" height="40"/></g>`
      assert(isGreen(await render(ofType, 25, 20)), ':nth-of-type(2) should hit the 2nd rect (the 3rd child)')
      assert(!isGreen(await render(ofType, 5, 20)), ':nth-of-type(2) should skip the 1st rect')
    })

    test("general-sibling combinator (~)", async () => {
      let body = `<style>circle ~ rect{fill:${GREEN}}</style><g>` +
        `<rect x="0" width="10" height="40"/>` +                 // before the circle — not matched
        `<circle cx="15" cy="20" r="3"/>` +
        `<rect x="20" width="10" height="40"/>` +                // after — matched
        `<rect x="30" width="10" height="40"/></g>`              // after — matched
      assert(!isGreen(await render(body, 5, 20)), 'a sibling before the circle should not match')
      assert(isGreen(await render(body, 25, 20)) && isGreen(await render(body, 35, 20)), 'all rects after the circle should match')
    })

    test(":not()", async () => {
      let simple = `<style>rect:not(.skip){fill:${GREEN}}</style><g>` +
        `<rect x="0" width="20" height="40" class="skip"/><rect x="20" width="20" height="40"/></g>`
      assert(!isGreen(await render(simple, 10, 20)), ':not(.skip) should skip the .skip rect')
      assert(isGreen(await render(simple, 30, 20)), ':not(.skip) should match the other rect')

      // complex inner selector (adjacent combinator): the 2nd rect *is* `rect + rect`, so it's excluded
      let complex = `<style>rect:not(rect + rect){fill:${GREEN}}</style><g>` +
        `<rect x="0" width="20" height="40"/><rect x="20" width="20" height="40"/></g>`
      assert(isGreen(await render(complex, 10, 20)), ':not(rect+rect) should match the 1st rect')
      assert(!isGreen(await render(complex, 30, 20)), ':not(rect+rect) should exclude the 2nd rect')

      // a comma-separated list inside :not() is deferred → the pseudo never matches (rule doesn't apply)
      let list = `<style>rect:not(.a, .b){fill:${GREEN}}</style><rect width="40" height="40"/>`
      assert(!isGreen(await render(list)), ':not() with a comma-list should be skipped, not applied')
    })

    test("unknown & interactive pseudo-classes", async () => {
      // :target never matches statically, but the grouped plain `rect` selector must still apply —
      // i.e. an unknown pseudo no longer drops the whole rule (and its siblings) at parse time
      let grouped = `<style>rect:target, rect{fill:${GREEN}}</style><rect width="40" height="40"/>`
      assert(isGreen(await render(grouped)), 'a grouped plain selector should apply alongside an unknown :target selector')

      let target = `<style>rect:target{fill:${GREEN}}</style><rect width="40" height="40"/>`
      assert(!isGreen(await render(target)), ':target should never match a static render')
    })

    test("!important", async () => {
      // an important rule beats a higher-specificity normal one
      assert(isGreen(await render(
        `<style>#r{fill:${RED}} .x{fill:${GREEN} !important}</style><rect id="r" class="x" width="40" height="40"/>`)),
        'an important class rule should beat a normal id rule')
      // inline !important beats stylesheet !important
      assert(isGreen(await render(
        `<style>.x{fill:${RED} !important}</style><rect class="x" style="fill:${GREEN} !important" width="40" height="40"/>`)),
        'inline !important should beat stylesheet !important')
      // stylesheet !important beats a normal inline style (which would otherwise win)
      assert(isGreen(await render(
        `<style>.x{fill:${GREEN} !important}</style><rect class="x" style="fill:${RED}" width="40" height="40"/>`)),
        'stylesheet !important should beat a normal inline style')
    })
  })

  describe("sends notifications through", () => {
    test(".complete flag", async () => {
      assert(!img.complete)

      img.src = PATH
      await img.decode()
      assert(img.complete)
    })

    test(".onload callback", (t, done) => {
      // ensure that the fetch process can be overwritten while in flight
      img.onload = loaded => { throw Error("should not be called") }
      img.src = URI

      img.onload = function(){
        // confirm that `this` is set correctly
        assert.equal(this, img)
        done()
      }
      img.src = `${HOST}/tests/assets/globe.jpg`
    })

    test(".onerror callback", (t, done) => {
      img.onerror = err => {
        assert.match(err.message, /HTTP error 404/)
        done()
      }
      img.src = `${HOST}/nonesuch`
    })

    test(".decode promise", async () => {
      await assert.rejects(()=> img.decode(), /Image source not set/)

      img.src = URI
      let decoded = await img.decode()
      assert.equal(decoded, img)

      // can load new data into existing Image
      img.src = `${HOST}/tests/assets/image/format.png`
      decoded = await img.decode()
      assert.equal(decoded, img)

      // autoresolves once loaded
      assert.equal(await img.decode(), img)
    })
  })

  describe("can decode format", () => {
    const asBuffer = path => fs.readFileSync(path)

    const asDataURI = path => {
      let ext = path.split('.').at(-1),
          mime = `image/${ext.replace('jpg', 'jpeg')}`,
          content = fs.readFileSync(path).toString('base64')
      return `data:${mime};base64,${content}`
    }

    async function testFormat(ext){
      let path = `${FORMAT}.${ext}`

      let img = new Image()
      img.src = path
      await img.decode()
      assert.matchesSubset(img, PARSED)

      img = new Image()
      img.src = asDataURI(path)
      await img.decode()
      assert.matchesSubset(img, PARSED)

      img = new Image(asBuffer(path))
      assert.matchesSubset(img, PARSED)
    }

    test("PNG", async () => await testFormat("png") )
    test("JPEG", async () => await testFormat("jpg") )
    test("GIF", async () => await testFormat("gif") )
    test("BMP", async () => await testFormat("bmp") )
    test("ICO", async () => await testFormat("ico") )
    test("WEBP", async () => await testFormat("webp") )
    test("SVG", async () => await testFormat("svg") )
  })

  describe("preserves wide-gamut color", () => {
    for (const ext of ['png', 'jpg']){
      test(`in ICC-tagged ${ext.toUpperCase()}s`, async () => {
        // the fixture's pixels are 'display-p3 red': the full-intensity primary outside sRGB's gamut
        let img = await loadImage(`tests/assets/image/p3-red.${ext}`)

        // on an srgb canvas the color clips to the gamut's edge, discarding its extra intensity…
        let srgb = new Canvas(8, 8).getContext('2d')
        srgb.drawImage(img, 0, 0)
        assert.deepEqual(Array.from(srgb.getImageData(0, 0, 1, 1).data), [255, 0, 0, 255])
        assert.deepEqual(Array.from(srgb.getImageData(0, 0, 1, 1, {colorSpace:'display-p3'}).data), [234, 51, 35, 255])

        // …and on an untouched canvas, where no earlier read has left a correctly-spaced surface
        // behind to rasterize into (see the matching case in context2d's `wide-gamut color`)
        let cold = new Canvas(8, 8).getContext('2d')
        cold.drawImage(img, 0, 0)
        assert.deepEqual(Array.from(cold.getImageData(0, 0, 1, 1, {colorSpace:'display-p3'}).data), [234, 51, 35, 255])

        // …but survives intact on a display-p3 canvas (modulo jpeg lossiness)
        let p3 = new Canvas(8, 8).getContext('2d', {colorSpace:'display-p3'})
        p3.drawImage(img, 0, 0)
        let [r, g, b, a] = p3.getImageData(0, 0, 1, 1).data
        assert.ok(r >= 254 && g == 0 && b == 0 && a == 255)
      })
    }
  })
})

describe("ImageData", () => {
  var FORMAT = 'tests/assets/image/format.raw',
      RGBA = {width:60, height:60, colorType:'rgba'},
      BGRA = {width:60, height:60, colorType:'bgra'}

  describe("can be initialized from", () => {
    test("buffer", () => {
      let buffer = fs.readFileSync(FORMAT)
      let imgData = new ImageData(buffer, 60, 60)
      assert.matchesSubset(imgData, RGBA)

      assert.throws(() => new ImageData(buffer, 60, 59), /ImageData dimensions must match buffer length/)
    })

    test("loadImageData call", async () => {
      await loadImageData(FORMAT, 60, 60).then(imgData => {
        assert.matchesSubset(imgData, RGBA)
      })
    })

    test("canvas content", () => {
      let canvas = new Canvas(60, 60),
          ctx = canvas.getContext("2d")
      let rgbaData = ctx.getImageData(0, 0, 60, 60)
      assert.matchesSubset(rgbaData, RGBA)
      let bgraData = ctx.getImageData(0, 0, 60, 60, {colorType:'bgra'})
      assert.matchesSubset(bgraData, BGRA)
    })
  })

  test("supports colorSpace setting", () => {
    assert.equal(new ImageData(8, 8).colorSpace, 'srgb')
    assert.equal(new ImageData(8, 8, {colorSpace:'display-p3'}).colorSpace, 'display-p3')

    // outside of SKIA_CANVAS_STRICT mode, unsupported spaces quietly fall back to srgb
    // @ts-expect-error — 'rec2020' is deliberately not a valid ColorSpace
    assert.equal(new ImageData(8, 8, {colorSpace:'rec2020'}).colorSpace, 'srgb')
  })
})

describe("FontLibrary", ()=>{
  /** @type {Canvas} */
  let canvas
  /** @type {import('../../lib').CanvasRenderingContext2D} */
  let ctx
  let WIDTH = 512, HEIGHT = 512,
      FONTS_DIR = 'tests/assets/fonts',
      findFont = font => path.join(FONTS_DIR, font);

  beforeEach(() => {
    canvas = new Canvas(WIDTH, HEIGHT)
    ctx = canvas.getContext("2d")
  })

  afterEach(() => {
    FontLibrary.reset()
  })

  // probe a bundled family rather than a system one — a minimal container (the musl images under
  // containers/) ships no system fonts at all, so "Arial or DejaVu Sans" isn't a safe assumption.
  // The alias matters: registering under the font's real name would merge into a system copy of
  // it, so these would pass on the strength of the *host's* fonts rather than the registration.
  let useTestFace = () =>
    FontLibrary.use("TestFace", [findFont("montserrat-latin/montserrat-v30-latin-regular.woff2")])

  test("can list families", ()=>{
    useTestFace()

    let fams = FontLibrary.families,
        sorted = fams.slice().sort(),
        unique = [...new Set(sorted)];

    assert.contains(fams, "TestFace")
    assert.deepEqual(fams, sorted)
    assert.deepEqual(fams, unique)
  })

  test("can check for a family", ()=>{
    useTestFace()
    assert(FontLibrary.has("TestFace"))
    assert(!FontLibrary.has("_n_o_n_e_s_u_c_h_"))
  })

  test("can describe a family", ()=>{
    useTestFace()

    let info = FontLibrary.family("TestFace")
    assert(info)
    assert(Object.hasOwn(info, 'family'))
    assert(Object.hasOwn(info, 'weights'))
    assert.equal(info && typeof info.weights[0], 'number');
    assert(Object.hasOwn(info, 'widths'))
    assert.equal(info && typeof info.widths[0], 'string');
    assert(Object.hasOwn(info, 'styles'))
    assert.equal(info && typeof info.styles[0], 'string');
  })

  test("can register fonts", ()=>{
    let ttf = findFont("AmstelvarAlpha-VF.ttf"),
        name = "AmstelvarAlpha",
        alias = "PseudonymousBosch";

    // with real name
    assert.doesNotThrow(() => FontLibrary.use(ttf))
    assert(FontLibrary.has(name))
    assert.contains((FontLibrary.family(name) || {}).weights, 400)

    // with alias
    assert.doesNotThrow(() => FontLibrary.use(alias, ttf))
    assert(FontLibrary.has(alias))
    assert.contains((FontLibrary.family(alias) || {}).weights, 400)

    // fonts disappear after reset
    FontLibrary.reset()
    assert(!FontLibrary.has(name))
    assert(!FontLibrary.has(alias))
  })

  test("can render woff2 fonts", ()=>{
    for (const ext of ['woff', 'woff2']){
      let woff = findFont("Monoton-Regular." + ext),
          name = "Monoton"
      assert.doesNotThrow(() => FontLibrary.use(woff))
      assert(FontLibrary.has(name))

      ctx.font = '256px Monoton'
      ctx.fillText('G', 128, 256)

      // look for one of the gaps between the inline strokes of the G
      let bmp = ctx.getImageData(300, 172, 1, 1)
      assert.deepEqual(Array.from(bmp.data), [0,0,0,0])
    }
  })

  test("instances variable fonts at the requested weight", () => {
    // Amstelvar exposes a `wght` axis, so heavier weights must lay down more ink
    // (verifies Skia's native variable-font instancing in Typesetter::layout)
    FontLibrary.use(findFont("AmstelvarAlpha-VF.ttf"))
    let ink = weight => {
      ctx.clearRect(0, 0, WIDTH, HEIGHT)
      ctx.font = `${weight} 80px AmstelvarAlpha`
      ctx.fillText("Hamburg", 20, 100)
      return ctx.getImageData(0, 0, WIDTH, HEIGHT).data.filter((v, i) => i % 4 == 3 && v > 10).length
    }
    assert(ink(400) > 0)
    assert(ink(700) > ink(400))
    assert(ink(900) > ink(700))
  })

  test("can handle different use() signatures", () => {
    const normalizePath = p => os.platform() == 'win32'
        ? p.replace(/^\\\\(?<path>[.?])/, '//$1') // The device path (\\.\ or \\?\)
           .replaceAll(/\\(?![!()+@[\]{}])/g, '/') // All backslashes except escapes
        : p

    FONTS_DIR = normalizePath(FONTS_DIR)

    const amstel = `${FONTS_DIR}/AmstelvarAlpha-VF.ttf`
    const monoton = [
      `${FONTS_DIR}/Monoton-Regular.woff`,
      `${FONTS_DIR}/Monoton-Regular.woff2`,
    ]
    const montserrat = [
      `${FONTS_DIR}/montserrat-latin/montserrat-v30-latin-200.woff2`,
      `${FONTS_DIR}/montserrat-latin/montserrat-v30-latin-700italic.woff2`,
      `${FONTS_DIR}/montserrat-latin/montserrat-v30-latin-200italic.woff2`,
      `${FONTS_DIR}/montserrat-latin/montserrat-v30-latin-italic.woff2`,
      `${FONTS_DIR}/montserrat-latin/montserrat-v30-latin-700.woff2`,
      `${FONTS_DIR}/montserrat-latin/montserrat-v30-latin-regular.woff2`,
    ]

    // list with multiple families
    assert.equal(FontLibrary.use([amstel, ...monoton]).length, 3)

    // alias for single family
    assert.equal(FontLibrary.use("Montmartre", montserrat).length, 6)

    // multiple family aliases (single-face per family)
    let single = FontLibrary.use({
      Monaton: monoton[0],
      Montserrat: montserrat[0]
    })
    assert.equal((single.Monaton || []).length, 1)
    assert.equal((single.Montserrat || []).length, 1)

    // multiple aliases (lists of faces)
    let multiple = FontLibrary.use({
      Monaton: [monoton[1]],
      Montserrat: montserrat.slice(1, -1)
    })
    assert.equal((multiple.Monaton || []).length, 1)
    assert.equal((multiple.Montserrat || []).length, 4)
  })

})
