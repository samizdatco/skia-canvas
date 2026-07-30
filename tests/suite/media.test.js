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
    assert.equal(new ImageData(8, 8, {colorSpace:'rec2020'}).colorSpace, 'srgb')
  })
})

describe("FontLibrary", ()=>{
  let canvas, ctx,
      WIDTH = 512, HEIGHT = 512,
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
    ctx.fontHinting = 0
    assert.equal(ctx.fontHinting, false)
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
