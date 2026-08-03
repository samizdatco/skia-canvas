// @ts-check

"use strict"

const path = require('path'),
      os = require('os'),
      fs = require('fs'),
      nock = require('nock'),
      {assert, describe, test, beforeEach, afterEach} = require('../runner'),
      {pathToFileURL, fileURLToPath} = require('url'),
      {Canvas, Image, ImageData, FontLibrary, loadImage, loadImageData} = require('../../lib')

const scope = nock('http://_h_o_s_t_')
  .persist()
  .get(/.*/)
  .reply((uri, requestBody) => {
    try{
      return [200, fs.readFileSync(process.cwd() + uri)]
    }catch(e){
      return [404, `Failed to load image from "${uri}" (HTTP error 404)`]
    }

  })

describe("Image", () => {
  var PATH = 'tests/assets/pentagon.png',
      URI = `http://_h_o_s_t_/${PATH}`,
      BUFFER = fs.readFileSync(PATH),
      DATA_URI = `data:image/png;base64,${BUFFER.toString('base64')}`,
      FILE_URL = pathToFileURL(PATH),
      FRESH = {complete:false, width:0, height:0},
      LOADED = {complete:true, width:125, height:125},
      FORMAT = 'tests/assets/image/format',
      PARSED = {complete:true, width:60, height:60},
      SVG_PATH = `${FORMAT}.svg`,
      SVG_URI = `http://_h_o_s_t_/${SVG_PATH}`,
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

      await assert.rejects(loadImage("http://_h_o_s_t_/nonesuch"), /HTTP error 404/)
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

    test("supported selector constructions all apply", async () => {
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

    test("structural selectors match the right sibling", async () => {
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

    test("cascade & specificity resolve like a browser", async () => {
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

    test("child-order & of-type structural pseudo-classes select correctly", async () => {
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

    test("general sibling (~) targets every following sibling", async () => {
      let body = `<style>circle ~ rect{fill:${GREEN}}</style><g>` +
        `<rect x="0" width="10" height="40"/>` +                 // before the circle — not matched
        `<circle cx="15" cy="20" r="3"/>` +
        `<rect x="20" width="10" height="40"/>` +                // after — matched
        `<rect x="30" width="10" height="40"/></g>`              // after — matched
      assert(!isGreen(await render(body, 5, 20)), 'a sibling before the circle should not match')
      assert(isGreen(await render(body, 25, 20)) && isGreen(await render(body, 35, 20)), 'all rects after the circle should match')
    })

    test(":not() handles simple, complex, and (deferred) list arguments", async () => {
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

    test("unknown/interactive pseudo-classes are skipped without dropping grouped selectors", async () => {
      // :target never matches statically, but the grouped plain `rect` selector must still apply —
      // i.e. an unknown pseudo no longer drops the whole rule (and its siblings) at parse time
      let grouped = `<style>rect:target, rect{fill:${GREEN}}</style><rect width="40" height="40"/>`
      assert(isGreen(await render(grouped)), 'a grouped plain selector should apply alongside an unknown :target selector')

      let target = `<style>rect:target{fill:${GREEN}}</style><rect width="40" height="40"/>`
      assert(!isGreen(await render(target)), ':target should never match a static render')
    })

    test("!important resolves in our own cascade (Skia ignores it)", async () => {
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
      img.src = 'http://_h_o_s_t_/tests/assets/globe.jpg'
    })

    test(".onerror callback", (t, done) => {
      img.onerror = err => {
        assert.match(err.message, /HTTP error 404/)
        done()
      }
      img.src = 'http://_h_o_s_t_/nonesuch'
    })

    test(".decode promise", async () => {
      await assert.rejects(()=> img.decode(), /Image source not set/)

      img.src = URI
      let decoded = await img.decode()
      assert.equal(decoded, img)

      // can load new data into existing Image
      img.src = 'http://_h_o_s_t_/tests/assets/image/format.png'
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

  test("can list families", ()=>{
    let fams = FontLibrary.families,
        sorted = fams.slice().sort(),
        unique = [...new Set(sorted)];

    assert(fams.indexOf("Arial")>=0 || fams.indexOf("DejaVu Sans") >= 0)
    assert.deepEqual(fams, sorted)
    assert.deepEqual(fams, unique)
  })

  test("can check for a family", ()=>{
    assert(FontLibrary.has("Arial") || FontLibrary.has("DejaVu Sans"))
    assert(!FontLibrary.has("_n_o_n_e_s_u_c_h_"))
  })

  test("can describe a family", ()=>{
    let fam = FontLibrary.has("Arial") ? "Arial"
            : FontLibrary.has("DejaVu Sans") ? "DejaVu Sans"
            : null;

    if (fam){
      let info = FontLibrary.family(fam)
      assert(info)
      assert(Object.hasOwn(info, 'family'))
      assert(Object.hasOwn(info, 'weights'))
      assert.equal(info && typeof info.weights[0], 'number');
      assert(Object.hasOwn(info, 'widths'))
      assert.equal(info && typeof info.widths[0], 'string');
      assert(Object.hasOwn(info, 'styles'))
      assert.equal(info && typeof info.styles[0], 'string');
    }
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

  test("synthesizes faux bold/oblique unless fontSynthesis is off", () => {
    // Monoton has a single (regular) face, so bold/italic have no *real* face to match
    FontLibrary.use(findFont("Monoton-Regular.woff"))
    let render = (spec, synth) => {
      ctx.fontSynthesis = synth
      ctx.clearRect(0, 0, WIDTH, HEIGHT)
      ctx.font = `${spec} 80px Monoton`
      ctx.fillText("RR", 40, 100)
      return ctx.getImageData(0, 0, WIDTH, HEIGHT).data
    }

    // count opaque pixels for weight comparisons
    let opaque = data => data.filter((v, i) => i % 4 == 3 && v > 10).length
    // diff alpha channels of pixmaps for oblique/normal comparisons
    let differs = (a, b) => { for (let i = 3; i < a.length; i += 4) if ((a[i] > 10) !== (b[i] > 10)) return true; return false }

    assert.equal(ctx.fontSynthesis, true) // browser-parity default: synthesis on
    let regular = render("400", true)
    assert(opaque(regular) > 0)

    // default is synthesis-on
    assert(opaque(render("700", true)) > opaque(regular)) // faux bold lays down more ink
    assert(differs(render("italic", true), regular))      // faux oblique skews the glyphs

    // ensure that disabling it actually prevents fake weight/slant
    assert.equal(opaque(render("700", false)), opaque(regular))
    assert(!differs(render("italic", false), regular))

    // check that the setting isn't cached across calls (i.e., FontLibrary clears its cache)
    assert(opaque(render("700", true)) > opaque(regular))
  })

  test("applies fontHinting to glyph rasterization", () => {
    // a face whose hinting instructions survive in the tracked woff2 subset
    FontLibrary.use("Montserrat", [findFont("montserrat-latin/montserrat-v30-latin-regular.woff2")])

    let render = (hinting) => {
      ctx.fontHinting = hinting
      ctx.clearRect(0, 0, WIDTH, HEIGHT)
      ctx.font = '11px Montserrat'
      ctx.fillText("Illegible waveforms 10x", 10, 24)
      return ctx.getImageData(0, 0, WIDTH, HEIGHT).data
    }
    let differs = (a, b) => { for (let i = 3; i < a.length; i += 4) if (a[i] !== b[i]) return true; return false }

    // boolean property; hinting disabled by default to match browser rendering
    assert.equal(ctx.fontHinting, false)
    ctx.fontHinting = true
    assert.equal(ctx.fontHinting, true)

    // truthy coercion like fontSynthesis (invalid values never throw)
    // @ts-expect-error — deliberately mistyped: exercises truthy coercion
    ctx.fontHinting = 0
    assert.equal(ctx.fontHinting, false)
    // @ts-expect-error — deliberately mistyped: exercises truthy coercion
    ctx.fontHinting = 'yes'
    assert.equal(ctx.fontHinting, true)

    // hinted vs unhinted rasterization must differ at text sizes (DirectWrite
    // quantization is unverified, hence the win32 escape hatch)
    let unhinted = render(false)
    let hinted = render(true)
    if (os.platform() != 'win32'){
      assert(differs(hinted, unhinted))
    }

    // toggling back must return to the identical rendering (i.e., FontLibrary clears its cache)
    assert(!differs(render(false), unhinted))
  })

  test("applies fontSmoothing to glyph antialiasing & positioning", () => {
    FontLibrary.use("Montserrat", [findFont("montserrat-latin/montserrat-v30-latin-regular.woff2")])

    let census = (smoothing) => {
      ctx.fontSmoothing = smoothing
      ctx.clearRect(0, 0, WIDTH, HEIGHT)
      ctx.font = '40px Montserrat'
      ctx.fillText("Osprey wings", 10, 55)
      let data = ctx.getImageData(0, 0, WIDTH, HEIGHT).data,
          tally = {opaque:0, partial:0}
      for (let i = 3; i < data.length; i += 4){
        if (data[i] == 255) tally.opaque++
        else if (data[i] > 0) tally.partial++
      }
      return tally
    }

    // boolean property; smoothing on by default (truthy coercion, like fontSynthesis)
    assert.equal(ctx.fontSmoothing, true)
    ctx.fontSmoothing = false
    assert.equal(ctx.fontSmoothing, false)
    // @ts-expect-error — deliberately mistyped: exercises truthy coercion
    ctx.fontSmoothing = 'yes'
    assert.equal(ctx.fontSmoothing, true)

    // smoothed edges include partial-alpha pixels; aliased coverage is strictly 0/255
    let aliased = census(false)
    assert(aliased.opaque > 0)
    assert.equal(aliased.partial, 0)
    let antialiased = census(true)
    assert(antialiased.partial > 0)

    // toggling back must re-alias (i.e., FontLibrary clears its cache)
    assert.equal(census(false).partial, 0)

    // smoothing also drives subpixel positioning: fractional placement renders
    // differently than the aliased/grid-snapped case, and round-trips after toggling
    let render = (smoothing) => {
      ctx.fontSmoothing = smoothing
      ctx.clearRect(0, 0, WIDTH, HEIGHT)
      ctx.font = '11px Montserrat'
      ctx.fillText("Illegible waveforms 10x", 10.5, 24)
      return ctx.getImageData(0, 0, WIDTH, HEIGHT).data
    }
    let differs = (a, b) => { for (let i = 3; i < a.length; i += 4) if (a[i] !== b[i]) return true; return false }
    let smoothed = render(true)
    assert(differs(render(false), smoothed))
    assert(!differs(render(true), smoothed))
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

describe("Typography", () => {
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

  describe("fontVariant", () => {
    test("defaults to normal", () => {
      assert.equal(ctx.fontVariant, "normal")
    })

    test("accepts single keywords", () => {
      for (let kw of ["small-caps", "all-small-caps", "tabular-nums", "oldstyle-nums",
                      "lining-nums", "discretionary-ligatures", "slashed-zero", "ordinal",
                      "super", "sub"]){
        ctx.fontVariant = kw
        assert.equal(ctx.fontVariant, kw)
      }
    })

    test("accepts normal (resetting a prior value)", () => {
      ctx.fontVariant = "small-caps"
      ctx.fontVariant = "normal"
      assert.equal(ctx.fontVariant, "normal")
    })

    test("accepts space-separated combos", () => {
      ctx.fontVariant = "small-caps tabular-nums"
      assert.equal(ctx.fontVariant, "small-caps tabular-nums")
      ctx.fontVariant = "oldstyle-nums lining-nums"
      assert.equal(ctx.fontVariant, "oldstyle-nums lining-nums")
    })

    test("accepts parameterized alternates", () => {
      for (let v of ["stylistic(2)", "styleset(3)", "swash(1)",
                     "character-variant(4)", "ornaments(1)", "annotation(1)"]){
        ctx.fontVariant = v
        assert.equal(ctx.fontVariant, v)
      }
    })

    test("applies features to text shaping", () => {
      FontLibrary.use("TestFace", [findFont("montserrat-latin/montserrat-v30-latin-regular.woff2")])
      ctx.font = "40px TestFace"
      let text = "0123456789",
          base = ctx.measureText(text).width
      ctx.fontVariant = "tabular-nums"
      let tnum = ctx.measureText(text).width
      assert(tnum > base, `expected tabular-nums width (${tnum}) > proportional width (${base})`)
    })

    test("silently ignores invalid values, keeping the prior value", () => {
      ctx.fontVariant = "small-caps"
      assert.doesNotThrow(() => { ctx.fontVariant = "bogus" })
      assert.equal(ctx.fontVariant, "small-caps")
    })

    test("rejects a combo whole if any token is invalid (all-or-nothing)", () => {
      ctx.fontVariant = "tabular-nums"
      ctx.fontVariant = "small-caps bogus"
      assert.equal(ctx.fontVariant, "tabular-nums")
    })
  })

  describe("fontStretch", () => {
    test("defaults to normal", () => {
      assert.equal(ctx.fontStretch, "normal")
    })

    test("accepts stretch keywords", () => {
      /** @type {Array<import('../../lib').CanvasRenderingContext2D['fontStretch']>} */
      let keywords = ["ultra-condensed", "extra-condensed", "condensed", "semi-condensed",
                      "semi-expanded", "expanded", "extra-expanded", "ultra-expanded", "normal"]
      for (let kw of keywords){
        ctx.fontStretch = kw
        assert.equal(ctx.fontStretch, kw)
      }
    })

    test("silently ignores invalid values, keeping the prior value", () => {
      ctx.fontStretch = "condensed"
      assert.doesNotThrow(() => {
        // @ts-expect-error - "bogus" is not a valid CanvasFontStretch
        ctx.fontStretch = "bogus"
      })
      assert.equal(ctx.fontStretch, "condensed")
    })
  })

  describe("letterSpacing / wordSpacing", () => {
    test("default to 0px", () => {
      assert.equal(ctx.letterSpacing, "0px")
      assert.equal(ctx.wordSpacing, "0px")
    })

    test("accept absolute length units", () => {
      for (let len of ["5px", "2pt", "1pc", "0.5cm", "10q"]){
        ctx.letterSpacing = len
        assert.equal(ctx.letterSpacing, len)
        ctx.wordSpacing = len
        assert.equal(ctx.wordSpacing, len)
      }
    })

    test("accept font-relative em / rem units", () => {
      ctx.letterSpacing = "0.1em"
      assert.equal(ctx.letterSpacing, "0.1em")
      ctx.wordSpacing = "1rem"
      assert.equal(ctx.wordSpacing, "1rem")
    })

    test("resolve em against the current font size", () => {
      // 1em of letter-spacing should widen text twice as much at 40px as at 20px
      ctx.letterSpacing = "1em"
      ctx.font = "40px serif"
      let wide = ctx.measureText("abc").width
      ctx.font = "20px serif"
      let narrow = ctx.measureText("abc").width
      assert(wide > narrow, `expected 40px spacing (${wide}) > 20px spacing (${narrow})`)
    })

    test("resolve rem against a fixed 16px root, not the current font size", () => {
      // rem is root-relative, so 1rem == 16px regardless of the font size (unlike em above)
      for (let px of ["40px", "10px"]){
        ctx.font = `${px} serif`
        ctx.letterSpacing = "1rem"
        let rem = ctx.measureText("abc").width
        ctx.letterSpacing = "16px"
        let px16 = ctx.measureText("abc").width
        assert.nearEqual(rem, px16)
      }
    })

    test("ignore units that can't be resolved (%, ex, ch)", () => {
      ctx.letterSpacing = "4px"
      for (let bad of ["10%", "2ex", "3ch"]){
        ctx.letterSpacing = bad
        assert.equal(ctx.letterSpacing, "4px", `${bad} should have been ignored`)
      }
    })

    test("silently ignore invalid values, keeping the prior value", () => {
      ctx.wordSpacing = "6px"
      assert.doesNotThrow(() => { ctx.wordSpacing = "bogus" })
      assert.equal(ctx.wordSpacing, "6px")
    })
  })

  describe("textDecoration", () => {
    test("defaults to none", () => {
      assert.equal(ctx.textDecoration, "none")
    })

    test("accepts a bare line keyword (inheriting the current color)", () => {
      // a decoration with no explicit color uses `currentColor`; it must still apply rather than
      // being dropped as "invalid" (regression: underline-alone previously round-tripped to "none")
      for (let line of ["underline", "overline", "line-through"]){
        ctx.textDecoration = "none"
        ctx.textDecoration = line
        assert.equal(ctx.textDecoration, line)
      }
    })

    test("accepts a line + style + color combo", () => {
      ctx.textDecoration = "underline wavy red"
      assert.equal(ctx.textDecoration, "underline wavy red")
    })

    test("accepts a wide-gamut color token", () => {
      ctx.textDecoration = "line-through oklch(0.7 0.1 200)"
      assert.equal(ctx.textDecoration, "line-through oklch(0.7 0.1 200)")
    })

    test("silently ignores junk, keeping the prior value", () => {
      // junk must no-op (retain the prior decoration), not reset to "none": a single unrecognized
      // token, an unparseable color, and junk hidden behind a valid color
      ctx.textDecoration = "underline wavy red"
      for (let junk of ["blahblah", "underline blahblah", "notaword blue"]){
        ctx.textDecoration = junk
        assert.equal(ctx.textDecoration, "underline wavy red", `"${junk}" should be ignored`)
      }
    })

    test("an explicit none resets a prior decoration", () => {
      ctx.textDecoration = "underline"
      ctx.textDecoration = "none"
      assert.equal(ctx.textDecoration, "none")
    })
  })
})
