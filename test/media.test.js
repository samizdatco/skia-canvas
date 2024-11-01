// @ts-check

const _ = require('lodash'),
      fs = require('fs'),
      path = require('path'),
      {Canvas, Image, FontLibrary, loadImage} = require('../lib'),
      simple = require('simple-get')

jest.mock('simple-get', () => {
  const fs = require('fs')
  return {
    concat:function(src, callback){
      let path = src.replace(/^https?:\//, process.cwd())
      try{
        var [statusCode, data] = [200, fs.readFileSync(path)]
      }catch(e){
        var [statusCode, err] = [404, 'HTTP_ERROR_404']
      }

      setTimeout(() => callback(err, {statusCode}, data) )
    }
  }
})

describe("Image", () => {
  var PATH = 'test/assets/pentagon.png',
      URL = `https://${PATH}`,
      BUFFER = fs.readFileSync(PATH),
      DATA_URI = `data:image/png;base64,${BUFFER.toString('base64')}`,
      FRESH = {complete:false, width:undefined, height:undefined},
      LOADED = {complete:true, width:125, height:125},
      FORMAT = 'test/assets/image/format',
      PARSED = {complete:true, width:60, height:60},
      img

  beforeEach(() => img = new Image() )

  describe("can be initialized from", () => {
    test("buffer", () => {
      expect(img).toMatchObject(FRESH)
      img.src = BUFFER
      expect(img).toMatchObject(LOADED)
    })

    test("data uri", () => {
      expect(img).toMatchObject(FRESH)
      img.src = DATA_URI
      expect(img).toMatchObject(LOADED)
    })

    test("local file", () => {
      expect(img).toMatchObject(FRESH)
      img.src = PATH
      expect(img).toMatchObject(LOADED)
    })

    test("http url", done => {
      expect(img).toMatchObject(FRESH)
      img.onload = loaded => {
        expect(loaded).toBe(img)
        expect(img).toMatchObject(LOADED)
        done()
      }
      img.src = URL
    })

    test("loadImage call", async () => {
      expect(img).toMatchObject(FRESH)

      img = await loadImage(URL)
      expect(img).toMatchObject(LOADED)

      img = await loadImage(BUFFER)
      expect(img).toMatchObject(LOADED)

      img = await loadImage(DATA_URI)
      expect(img).toMatchObject(LOADED)

      img = await loadImage(PATH)
      expect(img).toMatchObject(LOADED)

      expect(async () => { await loadImage('http://nonesuch') }).rejects.toEqual("HTTP_ERROR_404")
    })
  })

  describe("sends notifications through", () => {
    test(".complete flag", () => {
      expect(img.complete).toEqual(false)

      img.src = PATH
      expect(img.complete).toEqual(true)
    })

    test(".onload callback", done => {
      // ensure that the fetch process can be overwritten while in flight
      img.onload = loaded => { throw Error("should not be called") }
      img.src = URL

      img.onload = loaded => done()
      img.src = 'http://test/assets/globe.jpg'
    })

    test(".onerror callback", done => {
      img.onerror = err => {
        expect(err).toEqual("HTTP_ERROR_404")
        done()
      }
      img.src = 'http://nonesuch'
    })

    test(".decode promise", async () => {
      expect(()=> img.decode() ).rejects.toEqual(new Error('Missing Source URL'))

      img.src = URL
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
    test("PNG", () => {
      img.src = FORMAT + '.png'
      expect(img).toMatchObject(PARSED)
    })

    test("JPEG", () => {
      img.src = FORMAT + '.jpg'
      expect(img).toMatchObject(PARSED)
    })

    test("GIF", () => {
      img.src = FORMAT + '.gif'
      expect(img).toMatchObject(PARSED)
    })

    test("BMP", () => {
      img.src = FORMAT + '.bmp'
      expect(img).toMatchObject(PARSED)
    })

    test("ICO", () => {
      img.src = FORMAT + '.ico'
      expect(img).toMatchObject(PARSED)
    })

    test("WEBP", () => {
      img.src = FORMAT + '.webp'
      expect(img).toMatchObject(PARSED)
    })
  })
})


describe("FontLibrary", ()=>{
  let canvas, ctx,
      WIDTH = 512, HEIGHT = 512,
      findFont = font => path.join(__dirname, 'assets', font),
      pixel = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data);

  beforeEach(() => {
    canvas = new Canvas(WIDTH, HEIGHT)
    ctx = canvas.getContext("2d")
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


    FontLibrary.reset()
  })

})

