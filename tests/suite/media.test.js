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
      {Canvas, Image, ImageData, FontLibrary, loadImage, loadImageData, loadCanvas} = require('../../lib')

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
      PDF_PATH = `${FORMAT}.pdf`,
      PDF_URI = `${HOST}/${PDF_PATH}`,
      PDF_BUFFER = fs.readFileSync(PDF_PATH),
      PDF_DATA_URI = `data:application/pdf;base64,${PDF_BUFFER.toString('base64')}`,
      PDF_FILE_URL = pathToFileURL(PDF_PATH),
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

      // like the raw-pixel descriptor, page is meaningless for other formats rather than an error
      assert.matchesSubset(await loadImage(PATH, {page:5}), LOADED)
      assert.matchesSubset(await loadImage(SVG_PATH, {page:5}), PARSED)

      await assert.rejects(loadImage(`${HOST}/nonesuch`), /HTTP error 404/)
    })

    test("request timeouts", async () => {
      // each of these has to sit through the full stall, so run them concurrently
      await Promise.all([
        // socket-inactivity timeout, before and after the response starts
        assert.rejects(loadImage(`${HOST}/stall`, {timeout:50}), /Timed out/),
        assert.rejects(loadImage(`${HOST}/stall-mid-body`, {timeout:50}), /Timed out/),

        // a deadline covers the whole request, including a response already in flight
        assert.rejects(loadImage(`${HOST}/stall`, {signal:AbortSignal.timeout(50)}), /Timed out/),
        assert.rejects(loadImage(`${HOST}/stall-mid-body`, {signal:AbortSignal.timeout(50)}), /Timed out/),
      ])

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

  describe("can resolve SVG <style> CSS", () => {
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

  describe("can draw autosized SVGs", () => {
    test("with crop coordinates", async () => {
      // a viewBox without width/height gives the SVG a ratio but no size, so it draws at its
      // 'concrete object size': the largest box with that ratio fitting the canvas. The crop rect
      // of the 9-argument form is relative to that same scaled-to-fit size — not to the canvas,
      // and not to the 300×150 default the Image reports. Browsers resolve both against the
      // canvas the same way (verified in Chrome).
      let quadrants = Buffer.from(
        `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 150">` +
        `<rect x="0" y="0" width="150" height="75" fill="#f00"/>` +
        `<rect x="150" y="0" width="150" height="75" fill="#0f0"/>` +
        `<rect x="0" y="75" width="150" height="75" fill="#00f"/>` +
        `<rect x="150" y="75" width="150" height="75" fill="#ff0"/></svg>`
      )
      let img = await loadImage(quadrants)
      assert.deepEqual([img.width, img.height], [300, 150])

      // sample the four quadrants of a 640×480 destination
      const quads = ctx => [[160, 120], [480, 120], [160, 360], [480, 360]]
        .map(([x, y]) => Array.from(ctx.getImageData(x, y, 1, 1).data).slice(0, 3).join())

      // the fitted size on a 640×480 canvas is 640×320, so that's the crop covering the whole image
      let all = new Canvas(640, 480).getContext('2d')
      all.drawImage(img, 0, 0, 640, 320, 0, 0, 640, 480)
      assert.deepEqual(quads(all), ['255,0,0', '0,255,0', '0,0,255', '255,255,0'])

      // and a quadrant is addressed in that same space (the top-left quarter of 640×320)
      let corner = new Canvas(640, 480).getContext('2d')
      corner.drawImage(img, 0, 0, 320, 160, 0, 0, 640, 480)
      assert.deepEqual(quads(corner), Array(4).fill('255,0,0'))

      // the 2-argument form draws at that fitted size, so the crop space and the drawn size agree
      let plain = new Canvas(640, 480).getContext('2d')
      plain.drawImage(img, 0, 0)
      assert.equal(plain.getImageData(639, 319, 1, 1).data[3], 255) // bottom-right of the fit box
      assert.equal(plain.getImageData(0, 321, 1, 1).data[3], 0)     // just below it
    })

    test("without an intrinsic ratio", async () => {
      // with no viewBox there's no ratio to preserve, so CSS's default sizing algorithm falls all the
      // way back to the 300×150 default object size. That box becomes the picture's bounds — clipping
      // any artwork outside it — and from there autosizing proceeds exactly as it does with a
      // viewBox: the box scales to fit the canvas and the crop coordinates follow it.
      //
      // Browsers diverge here. With no ratio their concrete object size is the canvas itself, and
      // with no viewBox that resize crops the artwork instead of scaling it, so Chrome paints these
      // at their literal user units. What follows pins our behavior, not the browser's.
      const svg = attrs => Buffer.from(
        `<svg xmlns="http://www.w3.org/2000/svg"${attrs}>` +
        `<rect x="0" y="0" width="150" height="75" fill="#f00"/>` +
        `<rect x="150" y="0" width="150" height="75" fill="#0f0"/>` +
        `<rect x="0" y="75" width="150" height="75" fill="#00f"/>` +
        `<rect x="150" y="75" width="150" height="75" fill="#ff0"/></svg>`
      )

      // no dimensions at all → the full 300×150 default, a 2:1 box that fits 640×480 at 640×320
      let sizeless = await loadImage(svg(''))
      assert.deepEqual([sizeless.width, sizeless.height], [300, 150])

      let fitted = new Canvas(640, 480).getContext('2d')
      fitted.drawImage(sizeless, 0, 0)
      assert.equal(fitted.getImageData(639, 319, 1, 1).data[3], 255) // bottom-right of the fit box
      assert.equal(fitted.getImageData(0, 321, 1, 1).data[3], 0)     // just below it

      // and the crop space is that same fitted box, just as when a viewBox supplies the ratio
      let cropped = new Canvas(640, 480).getContext('2d')
      cropped.drawImage(sizeless, 0, 0, 640, 320, 0, 0, 640, 480)
      assert.deepEqual([[160, 120], [480, 120], [160, 360], [480, 360]]
        .map(([x, y]) => Array.from(cropped.getImageData(x, y, 1, 1).data).slice(0, 3).join()),
        ['255,0,0', '0,255,0', '0,0,255', '255,255,0'])

      // one axis on its own takes the default for the other and the box clips the rest: 200×150 is
      // 4:3, so it fills a 640×480 canvas edge to edge instead of leaving the 2:1 letterbox above
      let wide = await loadImage(svg(' width="200"'))
      assert.deepEqual([wide.width, wide.height], [200, 150])

      let filled = new Canvas(640, 480).getContext('2d')
      filled.drawImage(wide, 0, 0)
      assert.equal(filled.getImageData(639, 479, 1, 1).data[3], 255) // bottom-right corner
      assert.equal(filled.getImageData(0, 479, 1, 1).data[3], 255)   // bottom-left corner
    })
  })

  describe("can initialize PDFs from", () => {
    test("buffer", () => {
      assert.matchesSubset(img, FRESH)
      img = new Image(PDF_BUFFER)
      assert.matchesSubset(img, PARSED)

      img = new Image()
      img.src = PDF_BUFFER
      assert.matchesSubset(img, PARSED)
    })

    test("data uri", async () => {
      assert.matchesSubset(img, FRESH)
      img.src = PDF_DATA_URI
      assert.matchesSubset(img, PARSED)
    })

    test("local file", async () => {
      assert.matchesSubset(img, FRESH)
      img.src = PDF_PATH
      assert(!img.complete)
      await img.decode()
      assert.matchesSubset(img, PARSED)
    })

    test("file url", async () => {
      assert.matchesSubset(img, FRESH)
      img.src = PDF_FILE_URL
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
      img.src = PDF_URI
      assert(!img.complete)
    })

    test("selected pages", async () => {
      // a 3-page document: red, green, then blue on a wider final page
      let doc = new Canvas(100, 100),
          colors = ['#f00', '#0f0', '#00f'],
          page = doc.getContext('2d')
      for (let i = 0; i < colors.length; i++){
        if (i) page = doc.newPage(i == 2 ? 200 : 100, 100)
        page.fillStyle = colors[i]
        page.fillRect(0, 0, 300, 100)
      }
      let pdf = await doc.toBuffer('pdf')

      const firstPixel = loaded => {
        let ctx = new Canvas(loaded.width, loaded.height).getContext('2d')
        ctx.drawImage(loaded, 0, 0)
        return Array.from(ctx.getImageData(0, 0, 1, 1).data)
      }

      // 1-based, defaulting to the first page
      assert.deepEqual(firstPixel(await loadImage(pdf)), [255, 0, 0, 255])
      assert.deepEqual(firstPixel(await loadImage(pdf, {page:1})), [255, 0, 0, 255])
      assert.deepEqual(firstPixel(await loadImage(pdf, {page:2})), [0, 255, 0, 255])
      assert.deepEqual(firstPixel(await loadImage(pdf, {page:3})), [0, 0, 255, 255])

      // each page reports its own intrinsic size
      assert.matchesSubset(await loadImage(pdf, {page:2}), {width:100, height:100})
      assert.matchesSubset(await loadImage(pdf, {page:3}), {width:200, height:100})

      // out-of-range pages reject rather than clamping…
      await assert.rejects(loadImage(pdf, {page:4}), /Could not decode/)
      await assert.rejects(loadImage(pdf, {page:0}), /Could not decode/)

      // …as do fractional and non-numeric values
      await assert.rejects(loadImage(pdf, {page:1.5}), /Could not decode/)
      await assert.rejects(loadImage(pdf, {page:NaN}), /Could not decode/)
      // @ts-expect-error — deliberately passing a non-numeric page
      await assert.rejects(loadImage(pdf, {page:'nope'}), /Could not decode/)
    })

    test("a reused vector Image", async () => {
      // an Image is reusable, and an SVG with no intrinsic size marks it "draw me at the canvas
      // size". That flag governs any vector content, so loading a pdf into the same Image has to
      // clear it — otherwise the page silently scales up to fill whatever it's drawn onto.
      let sizeless = Buffer.from(
        `<svg xmlns="http://www.w3.org/2000/svg"><rect width="30" height="30" fill="#f00"/></svg>`
      )
      let reused = new Image(sizeless)
      assert.equal(reused.width, 300) // no width/height/viewBox → 300×150, per Chrome/CSS default sizing
      reused.src = PDF_BUFFER
      assert.matchesSubset(reused, PARSED)

      const render = image => {
        let ctx = new Canvas(200, 200).getContext('2d')
        ctx.drawImage(image, 0, 0) // no dims: the only case autosizing applies to
        return ctx
      }
      // the page is 60×60, so anything painted out at 150,150 means it was scaled to the canvas
      assert.equal(render(reused).getImageData(150, 150, 1, 1).data[3], 0)

      // and it lands pixel-for-pixel where a freshly-loaded copy does
      let fresh = render(new Image(PDF_BUFFER))
      for (let [x, y] of [[10, 10], [30, 30], [50, 50]]){
        let px = Array.from(render(reused).getImageData(x, y, 1, 1).data)
        assert(px[3] == 255, `page is unpainted at ${x},${y}`)
        assert.deepEqual(px, Array.from(fresh.getImageData(x, y, 1, 1).data))
      }
    })

    test("magic number detection", async () => {
      // the magic can appear anywhere in the first 1KB, but the body still has to parse
      let junk = Buffer.from('%PDF-1.7 not actually a pdf')
      await assert.rejects(loadImage(junk), /Could not decode/)
      assert.throws(() => new Image(junk), /Could not decode/)

      // and a broken pdf reports through the error event, like any undecodable image
      let broken = new Image(),
          failure = new Promise((res, rej) => { broken.onload = res, broken.onerror = rej })
      broken.src = junk
      await assert.rejects(failure, /Could not decode/)

      // the magic also has to begin a line, so an SVG that names the PDF it was converted from
      // stays on the SVG path instead of being routed into the (undecodable) PDF branch
      let svg = Buffer.from(
        `<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">` +
        `<metadata>converted from %PDF-1.4 source</metadata>` +
        `<rect width="40" height="40" fill="rgb(0,170,0)"/></svg>`
      )
      let mention = await loadImage(svg),
          ctx = new Canvas(40, 40).getContext('2d')
      assert.equal(mention.width, 40)
      ctx.drawImage(mention, 0, 0)
      assert.deepEqual(Array.from(ctx.getImageData(20, 20, 1, 1).data), [0, 170, 0, 255])
    })
  })

  describe("can render PDFs with", () => {
    // a 200×100 test page rendered by skia-canvas itself: red left half, blue right half
    const makePdf = async () => {
      let canvas = new Canvas(200, 100),
          ctx = canvas.getContext('2d')
      ctx.fillStyle = '#f00'
      ctx.fillRect(0, 0, 100, 100)
      ctx.fillStyle = '#00f'
      ctx.fillRect(100, 0, 100, 100)
      return canvas.toBuffer('pdf')
    }

    test("vectors preserved", async () => {
      let img = await loadImage(await makePdf())

      // drawn at 4×, the halves keep their exact colors…
      let ctx = new Canvas(800, 400).getContext('2d')
      ctx.drawImage(img, 0, 0, 800, 400)
      assert.deepEqual(Array.from(ctx.getImageData(200, 200, 1, 1).data), [255, 0, 0, 255])
      assert.deepEqual(Array.from(ctx.getImageData(600, 200, 1, 1).data), [0, 0, 255, 255])

      // …and the boundary between them stays sharp: a bitmap scaled up 4× would smear the
      // transition across many pixels, but a replayed vector antialiases within ~1px
      assert.deepEqual(Array.from(ctx.getImageData(397, 200, 1, 1).data), [255, 0, 0, 255])
      assert.deepEqual(Array.from(ctx.getImageData(402, 200, 1, 1).data), [0, 0, 255, 255])
    })

    test("transparency", async () => {
      // a half-opaque rect exports via an ExtGState alpha, exercising pdf transparency groups
      let canvas = new Canvas(50, 50),
          ctx = canvas.getContext('2d')
      ctx.globalAlpha = 0.5
      ctx.fillStyle = '#f00'
      ctx.fillRect(0, 0, 50, 50)

      let img = await loadImage(await canvas.toBuffer('pdf')),
          out = new Canvas(50, 50).getContext('2d')
      out.drawImage(img, 0, 0)
      let [r, g, b, a] = out.getImageData(25, 25, 1, 1).data
      assert.equal(r, 255)
      assert.nearEqual(a, 128, 2)
    })

    test("embedded fonts", async () => {
      FontLibrary.use("Montserrat", ['tests/assets/fonts/montserrat-latin/montserrat-v30-latin-regular.woff2'])
      let canvas = new Canvas(200, 50),
          ctx = canvas.getContext('2d')
      ctx.font = '32px Montserrat'
      ctx.fillStyle = 'black'
      ctx.fillText("Wavy", 10, 40)

      let outCanvas = new Canvas(200, 50),
          out = outCanvas.getContext('2d'),
          img = await loadImage(await canvas.toBuffer('pdf'))
      out.drawImage(img, 0, 0)

      // the glyphs should land in the same places as the canvas-rendered original
      // (compare ink coverage masks, since edge antialiasing weighs slightly differently)
      let direct = ctx.getImageData(0, 0, 200, 50).data,
          loaded = out.getImageData(0, 0, 200, 50).data,
          overlap = 0, combined = 0
      for (let i = 3; i < direct.length; i += 4){
        let a = direct[i] > 128, b = loaded[i] > 128
        if (a && b) overlap++
        if (a || b) combined++
      }
      assert(combined > 100, 'the reference text should have rendered')
      assert(overlap / combined > 0.85, `glyphs should overlay the original (got ${(overlap / combined).toFixed(3)})`)

      // and since the pdf's embedded font came through as real glyph runs (not traced
      // outlines), re-exporting the drawn image keeps text as text
      assert.contains((await outCanvas.toBuffer('svg')).toString(), '<text')
    })

    // Assemble a single-page pdf from a list of object bodies (1-indexed), splicing the embedded
    // font in as the object numbered `fontObj`. Hand-built because the constructs these tests
    // need — uncolored patterns, conventionally-placed text — aren't things skia ever writes.
    const buildPdf = (bodies, fontObj) => {
      const ttf = fs.readFileSync('tests/assets/fonts/Oswald-Medium.ttf')
      let parts = [Buffer.from('%PDF-1.4\n')], len = parts[0].length, offsets = []
      const push = buf => { parts.push(buf); len += buf.length }
      for (let n = 1; n <= bodies.length; n++){
        offsets.push(len)
        if (n == fontObj){
          push(Buffer.from(`${n} 0 obj\n<< /Length ${ttf.length} /Length1 ${ttf.length} >>\nstream\n`))
          push(ttf)
          push(Buffer.from(`\nendstream\nendobj\n`))
        }else push(Buffer.from(`${n} 0 obj\n${bodies[n - 1]}\nendobj\n`))
      }
      const startxref = len
      let tail = `xref\n0 ${bodies.length + 1}\n0000000000 65535 f \n`
      offsets.forEach(at => tail += `${String(at).padStart(10, '0')} 00000 n \n`)
      push(Buffer.from(tail + `trailer\n<< /Size ${bodies.length + 1} /Root 1 0 R >>\n` +
                       `startxref\n${startxref}\n%%EOF\n`))
      return Buffer.concat(parts)
    }

    // the font machinery every hand-built fixture below shares: objects 4 (font), 5 (file), 6 (descriptor)
    const FONT_OBJECTS = [
      `<< /Type /Font /Subtype /TrueType /BaseFont /Oswald /FirstChar 65 /LastChar 90 ` +
        `/Widths [${Array(26).fill(600).join(' ')}] /Encoding /WinAnsiEncoding /FontDescriptor 6 0 R >>`,
      null, // spliced in as the raw font file
      `<< /Type /FontDescriptor /FontName /Oswald /Flags 32 /FontBBox [-200 -300 1200 1000] ` +
        `/ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontFile2 5 0 R >>`,
    ]

    test("rotated text", async () => {
      // Rotation reaches us one of two ways: through the ctm (`cm`), which skia-canvas uses and
      // which leaves glyph_transform alone, or baked into the text matrix, which plenty of other
      // generators do for chart labels and watermarks. The latter is still just a uniform scale
      // and a rotation, so it belongs in a text run drawn under that rotation rather than traced.
      const render = async tm => {
        const content = `0 0 0 rg\nBT\n/F1 48 Tf\n${tm} Tm\n(ABC) Tj\nET\n`
        const pdf = buildPdf([
          `<< /Type /Catalog /Pages 2 0 R >>`,
          `<< /Type /Pages /Kids [3 0 R] /Count 1 >>`,
          `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] ` +
            `/Resources << /Font << /F1 4 0 R >> >> /Contents 7 0 R >>`,
          ...FONT_OBJECTS,
          `<< /Length ${content.length} >>\nstream\n${content}endstream`,
        ], 5)

        let canvas = new Canvas(200, 200), ctx = canvas.getContext('2d')
        ctx.drawImage(await loadImage(pdf), 0, 0)
        let px = ctx.getImageData(0, 0, 200, 200).data,
            box = {x0: 200, y0: 200, x1: -1, y1: -1, ink: 0}
        for (let y = 0; y < 200; y++) for (let x = 0; x < 200; x++){
          if (px[(y * 200 + x) * 4 + 3] > 128){
            box.ink++
            box.x0 = Math.min(box.x0, x); box.x1 = Math.max(box.x1, x)
            box.y0 = Math.min(box.y0, y); box.y1 = Math.max(box.y1, y)
          }
        }
        box.svg = (await canvas.toBuffer('svg')).toString()
        return box
      }

      let flat = await render('1 0 0 1 30 100'),      // upright
          turned = await render('0 1 -1 0 100 40')    // rotated a quarter turn

      // both take the glyph-run path, so re-exporting keeps them as text rather than outlines
      assert.contains(flat.svg, '<text')
      assert.contains(turned.svg, '<text')

      // and the rotation actually applied: a line of text that is wide and short upright comes
      // out narrow and tall, covering about as many pixels either way
      assert(flat.x1 - flat.x0 > flat.y1 - flat.y0, `upright text should be wider than tall`)
      assert(turned.y1 - turned.y0 > turned.x1 - turned.x0, `turned text should be taller than wide`)
      assert(Math.abs(flat.ink - turned.ink) < flat.ink * 0.15,
             `the same glyphs should ink comparably (${flat.ink} vs ${turned.ink})`)
    })

    test("conventional text", async () => {
      // skia writes its pages top-left-origin and re-flips per text block (`1 0 0 -1 … Tm`), but
      // everyone else places text in default pdf user space and leaves the y-flip in the ctm.
      // Both have to reach the glyph-run path — if only skia's convention did, ordinary pdfs
      // would silently fall back to tracing outlines and stop being text.
      const content = `0 0 0 rg\nBT\n/F1 48 Tf\n20 30 Td\n(ABC) Tj\nET\n`
      const pdf = buildPdf([
        `<< /Type /Catalog /Pages 2 0 R >>`,
        `<< /Type /Pages /Kids [3 0 R] /Count 1 >>`,
        `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] ` +
          `/Resources << /Font << /F1 4 0 R >> >> /Contents 7 0 R >>`,
        ...FONT_OBJECTS,
        `<< /Length ${content.length} >>\nstream\n${content}endstream`,
      ], 5)

      let img = await loadImage(pdf),
          canvas = new Canvas(200, 100),
          ctx = canvas.getContext('2d')
      ctx.drawImage(img, 0, 0)

      let px = ctx.getImageData(0, 0, 200, 100).data, inked = 0
      for (let i = 3; i < px.length; i += 4) if (px[i] > 128) inked++
      assert(inked > 200, `the text should have rendered (got ${inked}px)`)

      // real glyph runs survive a round-trip back out as text; traced outlines would be paths
      assert.contains((await canvas.toBuffer('svg')).toString(), '<text')
    })

    test("patterned text", async () => {
      // An uncolored (PaintType 2) tiling pattern takes its color from the `scn` that selects it,
      // but hayro's cache_key for a tiling pattern is just its content stream's — so two glyphs
      // stamped in different colors look identical by that key. Batching them into one run would
      // paint both in whichever color came first, since a TextBlob carries a single paint.
      const tile = `0 0 4 4 re f\n`
      const content = `/Pattern cs\nBT\n/F1 72 Tf\n10 20 Td\n` +
                      `1 0 0 /P1 scn\n(A) Tj\n0 0 1 /P1 scn\n(B) Tj\nET\n`
      const pdf = buildPdf([
        `<< /Type /Catalog /Pages 2 0 R >>`,
        `<< /Type /Pages /Kids [3 0 R] /Count 1 >>`,
        `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Resources ` +
          `<< /Font << /F1 4 0 R >> /Pattern << /P1 8 0 R >> >> /Contents 7 0 R >>`,
        ...FONT_OBJECTS,
        `<< /Length ${content.length} >>\nstream\n${content}endstream`,
        `<< /Type /Pattern /PatternType 1 /PaintType 2 /TilingType 1 /BBox [0 0 4 4] ` +
          `/XStep 4 /YStep 4 /Resources << >> /Length ${tile.length} >>\nstream\n${tile}endstream`,
      ], 5)

      let img = await loadImage(pdf),
          ctx = new Canvas(200, 100).getContext('2d')
      ctx.drawImage(img, 0, 0)

      // which hues show up in each glyph's column band
      const hues = (x0, x1) => {
        let {data} = ctx.getImageData(x0, 10, x1 - x0, 85), seen = new Set()
        for (let i = 0; i < data.length; i += 4){
          let [r, b, a] = [data[i], data[i + 2], data[i + 3]]
          if (a > 128) seen.add(r > 128 && b < 128 ? 'red'
                              : b > 128 && r < 128 ? 'blue' : 'other')
        }
        return seen
      }
      assert(hues(10, 55).has('red'), 'the first glyph should be stamped red')
      assert(hues(62, 110).has('blue'), 'the second glyph should be stamped blue, not the first color')
    })

    test("clipped, shaded text", async () => {
      // Text filled with a shading pattern has to honor the shading's /BBox just as a filled path
      // does — otherwise the same glyphs clip one way when they batch into a text run and another
      // way when they fall back to tracing, so the render would depend on whether the font parsed.
      // Hand-built with an embedded font: skia writes neither a /BBox nor a shading-filled string.
      const W = 300, H = 100, EDGE = 150
      const ttf = fs.readFileSync('tests/assets/fonts/Oswald-Medium.ttf')
      const content = `/Pattern cs /P1 scn\nBT\n/F1 72 Tf\n20 20 Td\n(HHHHHH) Tj\nET\n`
      const bodies = {
        1: `<< /Type /Catalog /Pages 2 0 R >>`,
        2: `<< /Type /Pages /Kids [3 0 R] /Count 1 >>`,
        3: `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${W} ${H}] /Resources ` +
           `<< /Pattern << /P1 4 0 R >> /Font << /F1 5 0 R >> >> /Contents 6 0 R >>`,
        4: `<< /Type /Pattern /PatternType 2 /Shading << /ShadingType 2 /ColorSpace /DeviceRGB ` +
           `/Coords [0 0 ${W} 0] /BBox [0 0 ${EDGE} ${H}] /Extend [true true] /Function ` +
           `<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >> >>`,
        5: `<< /Type /Font /Subtype /TrueType /BaseFont /Oswald /FirstChar 72 /LastChar 72 ` +
           `/Widths [600] /Encoding /WinAnsiEncoding /FontDescriptor 8 0 R >>`,
        6: `<< /Length ${content.length} >>\nstream\n${content}endstream`,
        8: `<< /Type /FontDescriptor /FontName /Oswald /Flags 32 /FontBBox [-200 -300 1200 1000] ` +
           `/ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 /FontFile2 7 0 R >>`,
      }
      let parts = [Buffer.from('%PDF-1.4\n')], len = parts[0].length, offsets = {}
      const push = buf => { parts.push(buf); len += buf.length }
      for (const n of [1, 2, 3, 4, 5, 6, 7, 8]){
        offsets[n] = len
        if (n == 7){
          push(Buffer.from(`7 0 obj\n<< /Length ${ttf.length} /Length1 ${ttf.length} >>\nstream\n`))
          push(ttf)
          push(Buffer.from(`\nendstream\nendobj\n`))
        }else push(Buffer.from(`${n} 0 obj\n${bodies[n]}\nendobj\n`))
      }
      const startxref = len
      let tail = `xref\n0 9\n0000000000 65535 f \n`
      for (let n = 1; n <= 8; n++) tail += `${String(offsets[n]).padStart(10, '0')} 00000 n \n`
      push(Buffer.from(tail + `trailer\n<< /Size 9 /Root 1 0 R >>\nstartxref\n${startxref}\n%%EOF\n`))

      let img = await loadImage(Buffer.concat(parts)),
          ctx = new Canvas(W, H).getContext('2d')
      ctx.drawImage(img, 0, 0)

      // count ink either side of the bbox edge rather than probing for stems
      let px = ctx.getImageData(0, 0, W, H).data,
          inked = [0, 0]
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++){
        if (px[(y * W + x) * 4 + 3] > 128) inked[x < EDGE ? 0 : 1]++
      }
      assert(inked[0] > 200, `glyphs inside the bbox should be painted (got ${inked[0]}px)`)
      assert.equal(inked[1], 0, `glyphs past the bbox should be clipped (got ${inked[1]}px)`)
    })

    test("patterns", async () => {
      let img = await loadImage(await makePdf()),
          ctx = new Canvas(400, 200).getContext('2d')
      ctx.fillStyle = ctx.createPattern(img, 'repeat')
      ctx.fillRect(0, 0, 400, 200)
      assert.deepEqual(Array.from(ctx.getImageData(50, 50, 1, 1).data), [255, 0, 0, 255])
      assert.deepEqual(Array.from(ctx.getImageData(150, 50, 1, 1).data), [0, 0, 255, 255])
      assert.deepEqual(Array.from(ctx.getImageData(250, 150, 1, 1).data), [255, 0, 0, 255])
    })

    test("vector gradients", async () => {
      // a gradient with a hard stop exports as a pdf axial shading; if the shading were baked
      // to a bitmap at 1× on load (the hayro-svg shortcut's failure mode), drawing at 4× would
      // smear the boundary across ≥8px — as a live gradient shader it stays crisp
      let canvas = new Canvas(100, 100),
          ctx = canvas.getContext('2d'),
          grad = ctx.createLinearGradient(0, 0, 100, 0)
      grad.addColorStop(0, '#f00')
      grad.addColorStop(0.5, '#f00')
      grad.addColorStop(0.5, '#00f')
      grad.addColorStop(1, '#00f')
      ctx.fillStyle = grad
      ctx.fillRect(0, 0, 100, 100)

      let img = await loadImage(await canvas.toBuffer('pdf')),
          out = new Canvas(800, 800).getContext('2d')
      out.drawImage(img, 0, 0, 800, 800)
      assert.deepEqual(Array.from(out.getImageData(200, 400, 1, 1).data), [255, 0, 0, 255])
      assert.deepEqual(Array.from(out.getImageData(600, 400, 1, 1).data), [0, 0, 255, 255])
      // the pixels on either side of the boundary, so the transition can't blur even a step wider
      // than the stops that define it (sampling the shading uniformly would spread it over ~2px
      // here, and proportionally more the further it's scaled up)
      assert.deepEqual(Array.from(out.getImageData(399, 400, 1, 1).data), [255, 0, 0, 255])
      assert.deepEqual(Array.from(out.getImageData(400, 400, 1, 1).data), [0, 0, 255, 255])

      // radial gradients ride through as two-point conical shadings
      let radial = new Canvas(100, 100),
          rctx = radial.getContext('2d'),
          rgrad = rctx.createRadialGradient(50, 50, 0, 50, 50, 50)
      rgrad.addColorStop(0, '#ff0')
      rgrad.addColorStop(1, '#00f')
      rctx.fillStyle = rgrad
      rctx.fillRect(0, 0, 100, 100)

      let rimg = await loadImage(await radial.toBuffer('pdf')),
          rout = new Canvas(400, 400).getContext('2d')
      rout.drawImage(rimg, 0, 0, 400, 400)
      // the 4× render should reproduce the original gradient's colors at matching points
      // (±4 tolerance: f32 color management plus the sub-pixel sampling offset)
      for (let [x, y] of [[50, 50], [50, 5], [10, 30], [80, 80]]){
        let src = rctx.getImageData(x, y, 1, 1).data,
            dst = rout.getImageData(x * 4, y * 4, 1, 1).data
        assert(src.every((c, i) => Math.abs(c - dst[i]) <= 4), `mismatch at ${x},${y}: ${src} vs ${dst}`)
      }
    })

    test("reconstructed sweep gradients", async () => {
      // skia has no pdf analogue for a conic gradient, so it exports one as a type 1 shading
      // whose function is just the angle about the origin. pdf.rs recognizes that shape and
      // rebuilds an actual sweep shader rather than rasterizing the function per pixel, which
      // keeps it resolution-independent the way axial/radial shadings already are.
      const SIZE = 400, C = SIZE / 2
      const paint = (ctx, scale) => {
        let grad = ctx.createConicGradient(0, C * scale, C * scale)
        for (let [pos, color] of [[0, '#f00'], [0.25, '#0f0'], [0.5, '#00f'], [0.75, '#ff0'], [1, '#f00']]){
          grad.addColorStop(pos, color)
        }
        ctx.fillStyle = grad
        ctx.fillRect(0, 0, SIZE * scale, SIZE * scale)
      }
      // a ring of samples around the gradient's center, at whatever scale it was drawn
      const ring = (ctx, scale) => {
        let out = []
        for (let deg = 0; deg < 360; deg += 15){
          let rad = deg * Math.PI / 180
          out.push(Array.from(ctx.getImageData(
            Math.round((C + Math.cos(rad) * 120) * scale),
            Math.round((C + Math.sin(rad) * 120) * scale), 1, 1
          ).data))
        }
        return out
      }
      const compare = (a, b, tolerance, label) => a.forEach((px, i) =>
        assert(px.every((c, j) => Math.abs(c - b[i][j]) <= tolerance),
               `${label}: ${px} vs ${b[i]} at sample ${i}`)
      )

      let source = new Canvas(SIZE, SIZE), srcCtx = source.getContext('2d')
      paint(srcCtx, 1)
      let img = await loadImage(await source.toBuffer('pdf'))

      // the round-trip reproduces the original's colors at matching angles (and isn't mirrored,
      // which sampling the function in the wrong direction would produce)
      let flat = new Canvas(SIZE, SIZE).getContext('2d')
      flat.drawImage(img, 0, 0)
      compare(ring(srcCtx, 1), ring(flat, 1), 4, 'conic round-trip')

      // and drawn at 4× it still matches a natively-rendered 4× conic, rather than smearing the
      // way an upscaled raster would
      let big = new Canvas(SIZE * 4, SIZE * 4), bigCtx = big.getContext('2d')
      bigCtx.drawImage(img, 0, 0, SIZE * 4, SIZE * 4)
      let native = new Canvas(SIZE * 4, SIZE * 4), nativeCtx = native.getContext('2d')
      paint(nativeCtx, 4)
      compare(ring(nativeCtx, 4), ring(bigCtx, 4), 4, 'conic at 4x')
    })

    test("rasterized shadings", async () => {
      // A hand-built type 1 (function-based) shading whose function ramps with x alone. Being
      // non-angular it can't be rebuilt as a sweep, so it lands in the rasterizing fallback —
      // the point of the test. The raster is capped, so a page wider than the cap has to be
      // sampled coarsely and scaled up to cover the geometry; cropping it instead would leave
      // everything past the cap unpainted.
      // (the page is kept short since only its width has to clear the cap, and every row costs
      // another 4096 evaluations of the shading's function)
      const W = 4200, H = 6
      const fn = `{ pop ${W} div dup dup }\n` // stack: x y -> x/W three times, a grey ramp
      const content = `/Pattern cs /P1 scn\n0 0 ${W} ${H} re f\n`
      const objects = [
        `<< /Type /Catalog /Pages 2 0 R >>`,
        `<< /Type /Pages /Kids [3 0 R] /Count 1 >>`,
        `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${W} ${H}] ` +
          `/Resources << /Pattern << /P1 4 0 R >> >> /Contents 6 0 R >>`,
        `<< /Type /Pattern /PatternType 2 /Shading << /ShadingType 1 /ColorSpace /DeviceRGB ` +
          `/Domain [0 ${W} 0 ${H}] /Function 5 0 R >> >>`,
        `<< /FunctionType 4 /Domain [0 ${W} 0 ${H}] /Range [0 1 0 1 0 1] /Length ${fn.length} >>` +
          `\nstream\n${fn}endstream`,
        `<< /Length ${content.length} >>\nstream\n${content}endstream`,
      ]
      let pdf = '%PDF-1.4\n', offsets = []
      objects.forEach((body, i) => {
        offsets.push(pdf.length)
        pdf += `${i + 1} 0 obj\n${body}\nendobj\n`
      })
      const startxref = pdf.length
      pdf += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`
      offsets.forEach(at => pdf += `${String(at).padStart(10, '0')} 00000 n \n`)
      pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${startxref}\n%%EOF\n`

      // sample single pixels rather than rendering the whole page
      let img = await loadImage(Buffer.from(pdf, 'latin1')),
          at = x => {
            let px = new Canvas(1, 1).getContext('2d')
            px.drawImage(img, x, H / 2, 1, 1, 0, 0, 1, 1)
            return Array.from(px.getImageData(0, 0, 1, 1).data)
          }

      assert.equal(img.width, W)
      for (let x of [10, W / 2 | 0, 4000, 4150, W - 10]){
        assert.equal(at(x)[3], 255, `shading is unpainted at x=${x}`)
      }
      // and it's still the ramp, not a smear: brightness climbs from left to right, including
      // across the stretch past the cap
      assert(at(10)[0] < 20, `expected black at the left edge, got ${at(10)}`)
      assert(at(W - 10)[0] > 235, `expected white at the right edge, got ${at(W - 10)}`)
      assert(at(4150)[0] > at(4000)[0], `ramp should still be climbing past the cap`)
    })

    test("tiling patterns", async () => {
      // a canvas pattern fill exports as a pdf tiling pattern (PatternType 1)
      let tile = new Canvas(10, 10),
          tctx = tile.getContext('2d')
      tctx.fillStyle = '#0a0'
      tctx.fillRect(0, 0, 5, 5)
      tctx.fillRect(5, 5, 5, 5)

      let canvas = new Canvas(100, 100),
          ctx = canvas.getContext('2d')
      ctx.fillStyle = ctx.createPattern(tile, 'repeat')
      ctx.fillRect(0, 0, 100, 100)

      let img = await loadImage(await canvas.toBuffer('pdf')),
          out = new Canvas(100, 100).getContext('2d')
      out.fillStyle = 'white'
      out.fillRect(0, 0, 100, 100)
      out.drawImage(img, 0, 0)

      // sample the center of on- and off-cells a few periods in
      assert.deepEqual(Array.from(out.getImageData(52, 52, 1, 1).data), [0, 170, 0, 255])
      assert.deepEqual(Array.from(out.getImageData(57, 52, 1, 1).data), [255, 255, 255, 255])
      assert.deepEqual(Array.from(out.getImageData(57, 57, 1, 1).data), [0, 170, 0, 255])

      // A cell whose bbox runs well past its x/y-step overlaps its neighbors, so the tile has to
      // be stamped from all of them — here ceil(100/5)-1 = 19 in each direction. The cell's only
      // ink sits in the far corner of its bbox, so the tile depends entirely on those distant
      // stamps: stopping short doesn't thin the pattern out, it erases it.
      const BBOX = 100, STEP = 5, SIZE = 200
      const cell = `0 0 1 rg\n${BBOX - 10} ${BBOX - 10} 10 10 re f\n`
      const content = `/Pattern cs /P1 scn\n0 0 ${SIZE} ${SIZE} re f\n`
      const bodies = {
        1: `<< /Type /Catalog /Pages 2 0 R >>`,
        2: `<< /Type /Pages /Kids [3 0 R] /Count 1 >>`,
        3: `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${SIZE} ${SIZE} ] ` +
           `/Resources << /Pattern << /P1 4 0 R >> >> /Contents 5 0 R >>`,
        4: `<< /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 ` +
           `/BBox [0 0 ${BBOX} ${BBOX}] /XStep ${STEP} /YStep ${STEP} /Resources << >> ` +
           `/Length ${cell.length} >>\nstream\n${cell}endstream`,
        5: `<< /Length ${content.length} >>\nstream\n${content}endstream`,
      }
      let pdf = '%PDF-1.4\n', offsets = {}
      for (const n of [1, 2, 3, 4, 5]){ offsets[n] = pdf.length; pdf += `${n} 0 obj\n${bodies[n]}\nendobj\n` }
      const startxref = pdf.length
      pdf += `xref\n0 6\n0000000000 65535 f \n`
      for (const n of [1, 2, 3, 4, 5]) pdf += `${String(offsets[n]).padStart(10, '0')} 00000 n \n`
      pdf += `trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n${startxref}\n%%EOF\n`

      let overlap = await loadImage(Buffer.from(pdf, 'latin1')),
          octx = new Canvas(SIZE, SIZE).getContext('2d')
      octx.drawImage(overlap, 0, 0)

      // with every stamp present the cells tile edge-to-edge and cover the page
      let px = octx.getImageData(0, 0, SIZE, SIZE).data, inked = 0
      for (let i = 3; i < px.length; i += 4) if (px[i] > 128) inked++
      assert(inked > SIZE * SIZE * 0.9, `pattern should cover the page (got ${inked}px of ${SIZE * SIZE})`)
      assert.deepEqual(Array.from(octx.getImageData(100, 100, 1, 1).data), [0, 0, 255, 255])
    })

    test("bounds-clipped shadings", async () => {
      // A shading dict can carry a /BBox limiting where it paints. Here an ImageMask (stencil)
      // covering most of the page is filled with an axial shading whose bbox stops at x=50, so
      // the right-hand part of the stencil has to come out unpainted. Hand-built because skia
      // never writes a /BBox onto the shadings it exports.
      const mask = Buffer.alloc(8, 0x00) // 8×8 1-bit mask, every sample 0 => paint
      const content = `q /Pattern cs /P1 scn\n80 0 0 80 10 10 cm\n/Im1 Do\nQ\n`
      const bodies = {
        1: `<< /Type /Catalog /Pages 2 0 R >>`,
        2: `<< /Type /Pages /Kids [3 0 R] /Count 1 >>`,
        3: `<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] /Resources ` +
           `<< /Pattern << /P1 4 0 R >> /XObject << /Im1 5 0 R >> >> /Contents 6 0 R >>`,
        4: `<< /Type /Pattern /PatternType 2 /Shading << /ShadingType 2 /ColorSpace /DeviceRGB ` +
           `/Coords [0 0 100 0] /BBox [0 0 50 100] /Extend [true true] /Function ` +
           `<< /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >> >>`,
        5: `<< /Type /XObject /Subtype /Image /Width 8 /Height 8 /ImageMask true ` +
           `/BitsPerComponent 1 /Length ${mask.length} >>`,
        6: `<< /Length ${content.length} >>\nstream\n${content}endstream`,
      }
      let parts = [Buffer.from('%PDF-1.4\n')], len = parts[0].length, offsets = {}
      const push = buf => { parts.push(buf); len += buf.length }
      for (const n of [1, 2, 3, 4, 5, 6]){
        offsets[n] = len
        if (n == 5){
          push(Buffer.from(`5 0 obj\n${bodies[5]}\nstream\n`))
          push(mask)
          push(Buffer.from(`\nendstream\nendobj\n`))
        }else push(Buffer.from(`${n} 0 obj\n${bodies[n]}\nendobj\n`))
      }
      const startxref = len
      let tail = `xref\n0 7\n0000000000 65535 f \n`
      for (const n of [1, 2, 3, 4, 5, 6]) tail += `${String(offsets[n]).padStart(10, '0')} 00000 n \n`
      push(Buffer.from(tail + `trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n${startxref}\n%%EOF\n`))

      let img = await loadImage(Buffer.concat(parts)),
          ctx = new Canvas(100, 100).getContext('2d')
      ctx.drawImage(img, 0, 0)
      const at = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data)

      assert.equal(at(20, 50)[3], 255, 'stencil should be painted inside the bbox')
      assert(at(20, 50)[0] > 150, `expected the gradient's red end, got ${at(20, 50)}`)
      assert.deepEqual(at(70, 50), [0, 0, 0, 0], 'stencil should be clipped outside the bbox')
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
    test("PDF", async () => await testFormat("pdf") )
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

const HAS_FLOAT16 = typeof Float16Array !== 'undefined' // Float16Array is a Node 23+ global

// `sharp` is an optional integration, deliberately not a declared dependency, so the tests that
// actually push pixels through it only run where someone has installed it
const HAS_SHARP = (() => { try{ require('sharp'); return true }catch{ return false } })()

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

    test("Uint8Array", () => {
      // Buffer is a Uint8Array subclass, so rejecting the superclass while accepting the subclass
      // was arbitrary — and it surfaced as a RangeError about the dimensions, not the argument
      let bytes = new Uint8Array(fs.readFileSync(FORMAT))
      assert.matchesSubset(new ImageData(bytes, 60, 60), RGBA)
    })

    test("shares memory with the array it was given", () => {
      // the spec has the constructor *use* the array it was handed, so writes through the
      // original are visible in `.data` — for every accepted buffer type, not just one
      for (const source of [new Uint8ClampedArray(16), new Uint8Array(16), Buffer.alloc(16)]){
        source.fill(10)
        let imgData = new ImageData(source, 2, 2)
        source[0] = 99
        assert.equal(imgData.data[0], 99, `${source.constructor.name} should alias, not copy`)
      }
    })

    test("copies when the byte offset can't be aliased", () => {
      // a Buffer carved out of the shared pool can land on an odd byteOffset, which a 2-byte
      // element type can't view directly
      let odd = Buffer.alloc(33).subarray(1)
      let imgData = new ImageData(odd, 2, 2, {colorType:'RGBAF16'})
      assert.equal(imgData.data.length, 16)
      odd[0] = 99
      assert.notEqual(imgData.data[0], 99) // fell back to a copy rather than throwing
    })

    test("rejects buffer types it can't interpret", () => {
      // a wide typed array is ambiguous — Uint16Array could be raw bytes or 16-bit samples —
      // so name the problem instead of reading it as a width
      for (const bad of [new Uint16Array(8), new Float32Array(4), new ArrayBuffer(16)]){
        // @ts-expect-error — these are exactly the types the constructor should refuse
        assert.throws(() => new ImageData(bad, 2, 2), /Expected a Uint8ClampedArray/)
      }
    })

    test("Float16Array", {skip: HAS_FLOAT16 ? false : "Float16Array requires Node 23+"}, () => {
      let [width, height] = [8, 8]

      // a bare Float16Array defaults to the RGBAF16 colorType (symmetric with u8 → 'rgba')
      let f16 = new Float16Array(width * height * 4).fill(0.5)
      let bmp = new ImageData(f16, width, height)
      assert.equal(bmp.colorType, 'RGBAF16')
      assert.equal(bmp.bytesPerPixel, 8)
      assert.ok(bmp.data instanceof Float16Array)
      assert.equal(bmp.data, f16) // kept byte-for-byte, not clamp-coerced
      assert.equal(bmp.data.length, width * height * 4)

      // an explicit half-float colorType is honored; RGBAF16Norm shares the RGBAF16 layout
      let norm = new ImageData(f16, width, height, {colorType:'RGBAF16Norm'})
      assert.equal(norm.colorType, 'RGBAF16Norm')
      assert.ok(norm.data instanceof Float16Array)

      // length validation is byteLength-based, so wrong dims still throw for an f16 buffer
      assert.throws(() => new ImageData(f16, width+1, height))

      // a Float16Array declared with a non-float colorType is a contradiction → throw
      assert.throws(() => new ImageData(f16, width, height, {colorType:'rgba'}), /half-float colorType/)

      // allocation-only with a half-float colorType yields a Float16Array
      let alloc = new ImageData(width, height, {colorType:'RGBAF16'})
      assert.ok(alloc.data instanceof Float16Array)
      assert.equal(alloc.data.length, width * height * 4)

      // RGBAF16 now reaches sharp (Skia widens it to f32 first) rather than being refused.
      // `sharp` is an optional integration and not a declared dependency, so tolerate it being
      // absent — the point is that the old half-float rejection is gone.
      try{ bmp.toSharp() }
      catch(e){ assert.match(e.message, /Cannot find module 'sharp'/) }

      // every half-float type reaches sharp now, routed to its unsigned equivalent — the values
      // were already clamped to [0,1] by the 8-bit surface, so there is no range to preserve
      for (const colorType of /** @type {import('../../lib').ColorType[]} */(['A16Float', 'R16G16Float'])){
        try{ new ImageData(4, 4, {colorType}).toSharp() }
        catch(e){ assert.match(e.message, /Cannot find module 'sharp'/) }
      }
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

    test("GPU surfaces", () => {
      // GPU backends refuse some ColorTypes as readback destinations even though Skia can convert
      // into them — BGR101010x is the one that trips it — so getImageData stages through rgba
      // when the direct read fails. Without that, these throw on GPU while working fine on CPU.
      const COLOR_TYPES = ["Alpha8","Gray8","R8UNorm","A16Float","A16UNorm","ARGB4444","R8G8UNorm",
        "RGB565","rgb","RGB888x","rgba","RGBA8888","bgra","BGRA8888","BGR101010x","RGB101010x",
        "BGRA1010102","RGBA1010102","R16G16Float","R16G16UNorm","SRGBA8888","R16G16B16A16UNorm",
        "RGBAF16","RGBAF16Norm","RGBAF32"]

      const painted = gpu => {
        let ctx = new Canvas(8, 1, {gpu}).getContext('2d')
        ctx.fillStyle = 'rgb(200 100 50)'; ctx.fillRect(0, 0, 4, 1)
        ctx.fillStyle = 'rgb(10 220 130)'; ctx.fillRect(4, 0, 4, 1)
        return ctx
      }

      for (const colorType of /** @type {import('../../lib').ColorType[]} */(COLOR_TYPES)){
        let gpu = painted(true).getImageData(0, 0, 8, 1, {colorType}),
            cpu = painted(false).getImageData(0, 0, 8, 1, {colorType})

        assert.equal(gpu.colorType, colorType)
        assert.equal(gpu.data.length, cpu.data.length)

        // the staged readback must not diverge from what the CPU backend produces directly
        // (a small tolerance covers ordinary GPU/CPU rasterization differences)
        for (let i = 0; i < cpu.data.length; i++){
          assert.nearEqual(gpu.data[i], cpu.data[i], 12)
        }
      }
    })
  })

  test("supports colorSpace setting", () => {
    assert.equal(new ImageData(8, 8).colorSpace, 'srgb')
    assert.equal(new ImageData(8, 8, {colorSpace:'display-p3'}).colorSpace, 'display-p3')

    // outside of SKIA_CANVAS_STRICT mode, unsupported spaces quietly fall back to srgb
    // @ts-expect-error — 'rec2020' is deliberately not a valid ColorSpace
    assert.equal(new ImageData(8, 8, {colorSpace:'rec2020'}).colorSpace, 'srgb')
  })

  test("round-trips half-float pixels", {skip: HAS_FLOAT16 ? false : "Float16Array requires Node 23+"}, () => {
    let [width, height] = [4, 4],
        canvas = new Canvas(width, height),
        ctx = canvas.getContext("2d")
    let white = new Float16Array(width * height * 4).fill(1.0)
    ctx.putImageData(new ImageData(white, width, height), 0, 0)

    // getImageData with a half-float colorType returns a Float16Array reading back the float values
    let f16 = ctx.getImageData(0, 0, width, height, {colorType:'RGBAF16'})
    assert.ok(f16.data instanceof Float16Array)
    assert.equal(f16.colorType, 'RGBAF16')
    assert.deepEqual(Array.from(f16.data.slice(0, 4)), [1, 1, 1, 1])

    // the default 8-bit read-back still works and sees the same white pixels
    let u8 = ctx.getImageData(0, 0, width, height)
    assert.ok(u8.data instanceof Uint8ClampedArray)
    assert.deepEqual(Array.from(u8.data.slice(0, 4)), [255, 255, 255, 255])
  })

  describe("toSharp()", {skip: HAS_SHARP ? false : "sharp is not installed"}, () => {
    const sharp = HAS_SHARP ? require('sharp') : null
    const SWATCHES = ['rgb(200 100 50)', 'rgb(10 220 130)', 'rgb(255 0 0)', 'rgb(17 34 51)']
    const p3Context = () => {
      let ctx = new Canvas(SWATCHES.length, 1).getContext('2d', {colorSpace:'display-p3'})
      SWATCHES.forEach((color, i) => { ctx.fillStyle = color; ctx.fillRect(i, 0, 1, 1) })
      return ctx
    }

    // Every colorType below must survive the trip into sharp with its channels in RGBA order and
    // its precision intact. `tolerance` is the format's own quantization (4-bit and 5-bit channels
    // genuinely cannot round-trip 8-bit values), not slack for layout errors.
    /** @type {{colorType: import('../../lib').ColorType, tolerance: number, channels?: number, depth?: string}[]} */
    const CASES = [
      {colorType:'rgba',              tolerance:0},
      {colorType:'bgra',              tolerance:0},  // channel order must be corrected, not passed through
      {colorType:'BGRA8888',          tolerance:0},
      {colorType:'RGB888x',           tolerance:0, channels:3},
      {colorType:'RGBA1010102',       tolerance:0, depth:'ushort'},  // 10-bit widened, not truncated
      {colorType:'BGRA1010102',       tolerance:0, depth:'ushort'},
      {colorType:'R16G16B16A16UNorm', tolerance:0, depth:'ushort'},
      {colorType:'RGB565',            tolerance:4},  // 5/6/5 bits
      {colorType:'ARGB4444',          tolerance:8},  // 4 bits per channel
      // the float types route to unsigned bands: skia-canvas rasterizes into 8-bit surfaces, so they
      // carry precision rather than dynamic range, and libvips would read a float band as linear scRGB
      {colorType:'RGBAF32',           tolerance:0, depth:'ushort'},
    ]
    if (HAS_FLOAT16) CASES.push(
      {colorType:'RGBAF16',     tolerance:0, depth:'ushort'},
      {colorType:'RGBAF16Norm', tolerance:0, depth:'ushort'},
    )

    // the colorTypes that also get checked in a display-p3 context (`RGB888x` is the name CASES uses
    // for the `rgb` alias)
    const P3_TYPES = ['rgba', 'bgra', 'RGB888x', 'RGBA1010102', 'R16G16B16A16UNorm', 'RGBAF16']

    describe("can round-trip", () => {
      for (const {colorType, tolerance, channels=4, depth='uchar'} of CASES){
        test(colorType, async () => {
          let canvas = new Canvas(SWATCHES.length, 1),
              ctx = canvas.getContext('2d')
          SWATCHES.forEach((color, i) => { ctx.fillStyle = color; ctx.fillRect(i, 0, 1, 1) })

          let expected = ctx.getImageData(0, 0, SWATCHES.length, 1).data,
              img = ctx.getImageData(0, 0, SWATCHES.length, 1, {colorType}).toSharp()

          let meta = await img.metadata()
          assert.equal(meta.channels, channels, `${colorType} should reach sharp as ${channels} channels`)
          assert.equal(meta.depth, depth, `${colorType} should reach sharp as ${depth}`)

          // srgb takes the raw handoff — wrapping it in a profiled container would be pure overhead
          assert.ok(!meta.icc, `${colorType} shouldn't carry a profile on the srgb path`)

          // normalize back to 8-bit sRGB and compare against the canvas's own RGB
          let {data} = await img.toColorspace('srgb').removeAlpha().raw().toBuffer({resolveWithObject:true})
          for (let p = 0; p < SWATCHES.length; p++){
            for (let c = 0; c < 3; c++){
              // nearEqual's comparison is strictly `<`, so a lossless format needs exact equality
              if (tolerance == 0) assert.equal(data[p*3 + c], expected[p*4 + c])
              else assert.nearEqual(data[p*3 + c], expected[p*4 + c], tolerance + 1)
            }
          }

          // A display-p3 ImageData holds different byte values for the same color, and libvips reads
          // raw pixels as sRGB with no way to say otherwise — so those bytes have to reach sharp
          // inside a container carrying their ICC profile, not as a raw handoff.
          if (P3_TYPES.includes(colorType)){
            let ctx = p3Context(),
                // what those p3 pixels *mean*, expressed in srgb by skia itself
                expected = ctx.getImageData(0, 0, SWATCHES.length, 1, {colorSpace:'srgb'}).data,
                wide = ctx.getImageData(0, 0, SWATCHES.length, 1, {colorType}),
                narrow = ctx.getImageData(0, 0, SWATCHES.length, 1, {colorType, colorSpace:'srgb'}),
                img = wide.toSharp()

            // the same swatches really are stored differently in p3, or there'd be nothing to tag
            assert.notDeepEqual(Array.from(wide.data), Array.from(narrow.data),
                                `${colorType} should hold different bytes in display-p3 than in srgb`)

            // the profile has to survive into sharp, or the pixels are just mislabeled sRGB
            let meta = await img.metadata()
            assert.ok(meta.icc, `${colorType} should carry an ICC profile into sharp`)

            // ...and survive sharp's own re-encoding, not just the handoff
            let exported = await sharp(await wide.toSharp().png().toBuffer()).metadata()
            assert.ok(exported.icc, `${colorType} should not be flattened to srgb by a png export`)

            // the band count belongs to the colorType, not the color space — `rgb`'s pad byte is
            // dropped on the way into the TIFF too, rather than arriving as an extra opaque alpha band
            assert.equal(meta.channels, (await narrow.toSharp().metadata()).channels,
                         `${colorType} should reach sharp with the same band count in either color space`)

            // `withIccProfile` color-manages through the profile; `toColorspace` only relabels the
            // band format, which would hand back the p3 numbers unconverted
            let {data} = await img.withIccProfile('srgb').removeAlpha().raw().toBuffer({resolveWithObject:true})
            for (let p = 0; p < SWATCHES.length; p++){
              for (let c = 0; c < 3; c++) assert.nearEqual(data[p*3 + c], expected[p*4 + c], 3)
            }
          }
        })
      }
    })


    // Three opaque bands, reached from opposite directions: the 2-channel formats widen into a third
    // band (libvips reads band 2 of a 2-band image as *alpha*, so a direct handoff would turn green
    // into transparency), while the padded RGB formats narrow, dropping their unused 4th byte rather
    // than passing it off as alpha.
    describe("partial-channel formats omit alpha", () => {
      const partial = /** @type {{colorType: import('../../lib').ColorType, pixels: number[]}[]} */ ([
        {colorType:'rgb',         pixels:[200, 100, 50, 10, 220, 130]},
        {colorType:'RGB888x',     pixels:[200, 100, 50, 10, 220, 130]},
        {colorType:'R8G8UNorm',   pixels:[200, 100,  0, 10, 220,   0]},
        {colorType:'R16G16UNorm', pixels:[200, 100,  0, 10, 220,   0]},
        {colorType:'R16G16Float', pixels:[200, 100,  0, 10, 220,   0]},
      ])

      for (const {colorType, pixels} of partial){
        test(`${colorType}`, async () => {
          let ctx = new Canvas(2, 1).getContext('2d')
          ctx.fillStyle = 'rgb(200 100 50)'; ctx.fillRect(0, 0, 1, 1)
          ctx.fillStyle = 'rgb(10 220 130)'; ctx.fillRect(1, 0, 1, 1)

          let img = ctx.getImageData(0, 0, 2, 1, {colorType}).toSharp(),
              meta = await img.metadata()

          // 3 opaque bands, never 2 or 4 — the surplus sample is dropped, not read as transparency
          assert.equal(meta.channels, 3)
          assert.equal(meta.hasAlpha, false)

          let {data} = await img.raw().toBuffer({resolveWithObject:true})
          assert.deepEqual(Array.from(data), pixels)

          // and it has to hold up under an alpha-aware op: with the surplus sample mistaken for
          // alpha, this flattened 200,100 to 78
          let flat = await ctx.getImageData(0, 0, 2, 1, {colorType}).toSharp()
            .flatten({background:'#000'}).raw().toBuffer({resolveWithObject:true})
          assert.deepEqual(Array.from(flat.data), pixels)
        })
      }
    })

    // A lone band has nothing for `channels` to drop and no RGB round trip to check, so these can't
    // ride along with the `can round-trip` cases — only that the band's width survives the handoff
    // (and that the float type lands on its unsigned equivalent rather than a linear-scRGB band).
    describe("single-channel formats maintain bit-depth", () => {
      const singleChannel = /** @type {{colorType: import('../../lib').ColorType, depth: string}[]} */ ([
        {colorType:'Alpha8',   depth:'uchar'},
        {colorType:'Gray8',    depth:'uchar'},
        {colorType:'R8UNorm',  depth:'uchar'},
        {colorType:'A16UNorm', depth:'ushort'},
        {colorType:'A16Float', depth:'ushort'},
      ])

      for (const {colorType, depth} of singleChannel){
        test(`${colorType}`, async () => {
          let meta = await new ImageData(2, 1, {colorType}).toSharp().metadata()
          assert.equal(meta.channels, 1, `${colorType} should reach sharp as a single band`)
          assert.equal(meta.depth, depth, `${colorType} should reach sharp as ${depth}`)
        })
      }
    })
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

  test("can identify variable fonts", ()=>{
    let [statik] = useTestFace(),
        [variable] = FontLibrary.use("VariableFace", [findFont("AmstelvarAlpha-VF.ttf")]);

    // the flag is set for faces with an `fvar` table (and the axes it describes are reported too)
    assert.equal(statik.variable, false)
    assert.deepEqual(statik.variations, {})
    assert.equal(variable.variable, true)
    assert(Object.keys(variable.variations).length > 0)

    // family() summarizes the same thing for the family as a whole
    assert.equal(FontLibrary.family("TestFace").variable, false)
    assert.equal(FontLibrary.family("VariableFace").variable, true)
  })

  test("can describe variation axes", ()=>{
    let [face] = FontLibrary.use("VariableFace", [findFont("AmstelvarAlpha-VF.ttf")]),
        family = FontLibrary.family("VariableFace");

    for (let {variations} of [face, family]){
      // the registered axes, with the ranges & names from the font's `fvar` and `name` tables
      assert.matchesSubset(variations.wght, {min:100, max:900, default:400, label:"Weight"})
      assert.matchesSubset(variations.wdth, {min:35, max:100, default:100, label:"Width"})
      assert.matchesSubset(variations.opsz, {min:10, max:72, default:14, label:"Optical Size"})

      // …plus Amstelvar's non-standard, upper-case tagged ones
      assert.matchesSubset(variations.GRAD, {min:88, max:150, default:88, label:"Grade"})
    }

    // the family's weight list is derived from the endpoints of its `wght` axis
    assert.contains(family.weights, 100)
    assert.contains(family.weights, 900)

    // non-variable faces report no axes at all
    assert.deepEqual(useTestFace()[0].variations, {})
  })

  test("can describe OpenType features", ()=>{
    let [face] = useTestFace(),
        family = FontLibrary.family("TestFace");

    for (let {features} of [face, family]){
      // tags found in the font's GSUB/GPOS tables, labelled from the OpenType registry
      assert.matchesSubset(features.liga, {label:"Standard Ligatures", type:"on/off"})
      assert.matchesSubset(features.kern, {label:"Kerning", type:"on/off"})
      assert.matchesSubset(features.tnum, {label:"Tabular Figures", type:"on/off"})
      assert(!Object.hasOwn(features, 'smcp')) // a tag this font doesn't define

      // tags are listed alphabetically
      assert.deepEqual(Object.keys(features), Object.keys(features).slice().sort())
    }

    // features taking an alternate's index (rather than an on/off toggle) are flagged as such
    let [oswald] = FontLibrary.use("Alternates", [findFont("Oswald-Medium.ttf")])
    assert.matchesSubset(oswald.features.aalt, {label:"Access All Alternates", type:"indexed"})

    // a family's features are the union of all its faces'
    FontLibrary.use("Chimera", [findFont("Oswald-Medium.ttf"), findFont("montserrat-latin/montserrat-v30-latin-regular.woff2")])
    let tags = Object.keys(FontLibrary.family("Chimera").features)
    assert.contains(tags, "sups") // only in Oswald
    assert.contains(tags, "tnum") // only in Montserrat
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
