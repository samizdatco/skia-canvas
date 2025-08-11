// @ts-check

const _ = require('lodash'),
      fs = require('fs'),
      path = require('path'),
      {Canvas, Image, ImageData, FontLibrary, loadImage, loadImageData} = require('../lib')

jest.mock("cross-fetch", () => {
  const fs = require('fs')
  return {
    fetch: function(src){
      let path = src.replace(/^https?:\//, process.cwd())

      try{
        let buf = new Uint8Array(fs.readFileSync(path)).buffer
        return Promise.resolve({
          ok: true,
          status: 200,
          arrayBuffer:() => Promise.resolve(buf)
        })
      }catch(e){
        return Promise.resolve({ok: false, status: 404})
      }
    },
  }
})

describe("Image", () => {
  var PATH = 'test/assets/pentagon.png',
      URI = `https://${PATH}`,
      BUFFER = fs.readFileSync(PATH),
      DATA_URI = `data:image/png;base64,${BUFFER.toString('base64')}`,
      FRESH = {complete:false, width:0, height:0},
      LOADED = {complete:true, width:125, height:125},
      FORMAT = 'test/assets/image/format',
      PARSED = {complete:true, width:60, height:60},
      SVG_PATH = `${FORMAT}.svg`,
      SVG_URI = `https://${SVG_PATH}`,
      SVG_BUFFER = fs.readFileSync(SVG_PATH),
      SVG_DATA_URI = `data:image/svg;base64,${SVG_BUFFER.toString('base64')}`,
      img

  beforeEach(() => img = new Image() )

  describe("can initialize bitmaps from", () => {
    test("buffer", async () => {
      expect(img).toMatchObject(FRESH)
      img.src = BUFFER
      await img.decode()
      expect(img).toMatchObject(LOADED)
    })

    test("data uri", () => {
      expect(img).toMatchObject(FRESH)
      img.src = DATA_URI
      expect(img).toMatchObject(LOADED)
    })

    test("local file", async () => {
      expect(img).toMatchObject(FRESH)
      img.src = PATH
      await img.decode()
      expect(img).toMatchObject(LOADED)
    })

    test("http url", done => {
      expect(img).toMatchObject(FRESH)
      img.onload = loaded => {
        expect(loaded).toBe(img)
        expect(img).toMatchObject(LOADED)
        done()
      }
      img.src = URI
    })

    test("loadImage call", async () => {
      expect(img).toMatchObject(FRESH)

      img = await loadImage(URI)
      expect(img).toMatchObject(LOADED)

      img = await loadImage(BUFFER)
      expect(img).toMatchObject(LOADED)

      img = await loadImage(DATA_URI)
      expect(img).toMatchObject(LOADED)

      img = await loadImage(PATH)
      expect(img).toMatchObject(LOADED)

      img = await loadImage(SVG_PATH)
      expect(img).toMatchObject(PARSED)

      img = await loadImage(new URL(URI))
      expect(img).toMatchObject(LOADED)

      img = await loadImage(new URL(DATA_URI))
      expect(img).toMatchObject(LOADED)

      img = await loadImage(new URL(`file:${__dirname}/../`+PATH))
      expect(img).toMatchObject(LOADED)

      img = await loadImage(new URL(`file:${__dirname}/../`+SVG_PATH))
      expect(img).toMatchObject(PARSED)

      expect(loadImage("http://nonesuch")).rejects.toThrow("HTTP error 404")
    })
  })

  describe("can initialize SVGs from", () => {
    test("buffer", async () => {
      expect(img).toMatchObject(FRESH)
      img.src = SVG_BUFFER
      await img.decode()
      expect(img).toMatchObject(PARSED)
    })

    test("data uri", async () => {
      expect(img).toMatchObject(FRESH)
      img.src = SVG_DATA_URI
      await img.decode()
      expect(img).toMatchObject(PARSED)
    })

    test("local file", async () => {
      expect(img).toMatchObject(FRESH)
      img.src = SVG_PATH
      await img.decode()
      expect(img).toMatchObject(PARSED)
    })

    test("http url", done => {
      expect(img).toMatchObject(FRESH)
      img.onload = loaded => {
        expect(loaded).toBe(img)
        expect(img).toMatchObject(PARSED)
        done()
      }
      img.src = SVG_URI
    })
  })

  describe("sends notifications through", () => {
    test(".complete flag", async () => {
      expect(img.complete).toEqual(false)

      img.src = PATH
      await img.decode()
      expect(img.complete).toEqual(true)
    })

    test(".onload callback", done => {
      // ensure that the fetch process can be overwritten while in flight
      img.onload = loaded => { throw Error("should not be called") }
      img.src = URI

      img.onload = function(){
        // confirm that `this` is set correctly
        expect(this).toBe(img)
        done()
      }
      img.src = 'http://test/assets/globe.jpg'
    })

    test(".onerror callback", done => {
      img.onerror = err => {
        expect(err.message).toMatch("HTTP error 404")
        done()
      }
      img.src = 'http://nonesuch'
    })

    test(".decode promise", async () => {
      expect(()=> img.decode() ).rejects.toEqual(new Error('Image source not set'))

      img.src = URI
      let decoded = await img.decode()
      expect(decoded).toBe(img)

      // can load new data into existing Image
      img.src = 'http://test/assets/image/format.png'
      decoded = await img.decode()
      expect(decoded).toBe(img)

      // autoresolves once loaded
      expect(img.decode()).resolves.toEqual(img)
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
      expect(img).toMatchObject(PARSED)

      img = new Image()
      img.src = asDataURI(path)
      await img.decode()
      expect(img).toMatchObject(PARSED)

      img = new Image()
      img.src = asBuffer(path)
      await img.decode()
      expect(img).toMatchObject(PARSED)
    }

    test("PNG", async () => await testFormat("png") )
    test("JPEG", async () => await testFormat("jpg") )
    test("GIF", async () => await testFormat("gif") )
    test("BMP", async () => await testFormat("bmp") )
    test("ICO", async () => await testFormat("ico") )
    test("WEBP", async () => await testFormat("webp") )
    test("SVG", async () => await testFormat("svg") )
  })
})

describe("ImageData", () => {
  var FORMAT = 'test/assets/image/format.raw',
      RGBA = {width:60, height:60, colorType:'rgba'},
      BGRA = {width:60, height:60, colorType:'bgra'}

  describe("can be initialized from", () => {
    test("buffer", () => {
      let buffer = fs.readFileSync(FORMAT)
      let imgData = new ImageData(buffer, 60, 60)
      expect(imgData).toMatchObject(RGBA)

      expect(() => new ImageData(buffer, 60, 59))
        .toThrow("ImageData dimensions must match buffer length")
    })

    test("loadImageData call", done => {
      loadImageData(FORMAT, 60, 60).then(imgData => {
        expect(imgData).toMatchObject(RGBA)
        done()
      })
    })

    test("canvas content", () => {
      let canvas = new Canvas(60, 60),
          ctx = canvas.getContext("2d")
      let rgbaData = ctx.getImageData(0, 0, 60, 60)
      expect(rgbaData).toMatchObject(RGBA)
      let bgraData = ctx.getImageData(0, 0, 60, 60, {colorType:'bgra'})
      expect(bgraData).toMatchObject(BGRA)
    })
  })
})

describe("FontLibrary", ()=>{
  let canvas, ctx,
      WIDTH = 512, HEIGHT = 512,
      ASSETS_DIR = path.join(__dirname, 'assets'),
      FONTS_DIR = path.join(ASSETS_DIR, 'fonts'),
      findFont = font => path.join(FONTS_DIR, font),
      pixel = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data);

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
        unique = _.uniq(sorted);

    expect(fams.indexOf("Arial")>=0 || fams.indexOf("DejaVu Sans")>=0).toBe(true)
    expect(fams).toEqual(sorted)
    expect(fams).toEqual(unique)
  })

  test("can check for a family", ()=>{
    expect(FontLibrary.has("Arial") || FontLibrary.has("DejaVu Sans")).toBe(true)
    expect(FontLibrary.has("_n_o_n_e_s_u_c_h_")).toBe(false)
  })

  test("can describe a family", ()=>{
    let fam = FontLibrary.has("Arial") ? "Arial"
            : FontLibrary.has("DejaVu Sans") ? "DejaVu Sans"
            : null;

    if (fam){
      let info = FontLibrary.family(fam)
      expect(info).toBeTruthy()
      expect(info).toHaveProperty('family')
      expect(info).toHaveProperty('weights')
      expect(info && typeof info.weights[0]).toBe('number');
      expect(info).toHaveProperty('widths')
      expect(info && typeof info.widths[0]).toBe('string');
      expect(info).toHaveProperty('styles')
      expect(info && typeof info.styles[0]).toBe('string');
    }
  })

  test("can register fonts", ()=>{
    let ttf = findFont("AmstelvarAlpha-VF.ttf"),
        name = "AmstelvarAlpha",
        alias = "PseudonymousBosch";

    // with real name
    expect(() => FontLibrary.use(ttf)).not.toThrow()
    expect(FontLibrary.has(name)).toBe(true)
    expect(_.get(FontLibrary.family(name), "weights")).toContain(400)

    // with alias
    expect(() => FontLibrary.use(alias, ttf)).not.toThrow()
    expect(FontLibrary.has(alias)).toBe(true)
    expect(_.get(FontLibrary.family(alias), "weights")).toContain(400)

    // fonts disappear after reset
    FontLibrary.reset()
    expect(FontLibrary.has(name)).toBe(false)
    expect(FontLibrary.has(alias)).toBe(false)
  })

  test("can render woff2 fonts", ()=>{
    for (const ext of ['woff', 'woff2']){
      let woff = findFont("Monoton-Regular." + ext),
          name = "Monoton"
      expect(() => FontLibrary.use(woff)).not.toThrow()
      expect(FontLibrary.has(name)).toBe(true)

      ctx.font = '256px Monoton'
      ctx.fillText('G', 128, 256)

      // look for one of the gaps between the inline strokes of the G
      let bmp = ctx.getImageData(300, 172, 1, 1)
      expect(Array.from(bmp.data)).toEqual([0,0,0,0])
    }
  })

  test("can handle glob patterns", () => {
    expect( FontLibrary.use([`${FONTS_DIR}/montserrat*/montserrat-v30-latin-italic.woff2`]) ).toHaveLength(1)
    expect( FontLibrary.use([`${FONTS_DIR}/montserrat-latin/*700*.woff2`]) ).toHaveLength(2)
    expect( FontLibrary.use([`${ASSETS_DIR}/**/montserrat-v30-latin-italic.woff2`]) ).toHaveLength(1)
    expect( FontLibrary.use([`${ASSETS_DIR}/**/montserrat*italic.*`]) ).toHaveLength(3)

    // `**` must be standalone (i.e., can't be attached to a file extension)
    expect( FontLibrary.use([`${ASSETS_DIR}/**.woff2`]) ).toHaveLength(0)
    expect( FontLibrary.use([`${ASSETS_DIR}/**/*.woff2`]) ).toHaveLength(7)

    // single alias
    expect( FontLibrary.use("Montmartre", [`${ASSETS_DIR}/**/montserrat*italic.*`]) ).toHaveLength(3)

    // multiple aliases (array of patterns)
    let { Monaton, Montserrat } = FontLibrary.use({
      Monaton: [`${FONTS_DIR}/*.woff2`],
      Montserrat: [`${FONTS_DIR}/montserrat-latin/*italic.woff2`, `${FONTS_DIR}/**/montserrat-latin/*700.woff2`]
    })
    expect(Monaton).toHaveLength(1)
    expect(Montserrat).toHaveLength(4)

    // multiple aliases (bare-string pattern)
    let { MonatonNowrap, MontserratNowrap  } = FontLibrary.use({
      MonatonNowrap: `${FONTS_DIR}/*.woff2`,
      MontserratNowrap: `${FONTS_DIR}/montserrat-latin/*.woff2`
    })
    expect(MonatonNowrap).toHaveLength(1)
    expect(MontserratNowrap).toHaveLength(6)
  })

})
