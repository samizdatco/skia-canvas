// @ts-check

"use strict"

const fs = require('fs'),
      os = require('os'),
      path = require('path'),
      {assert} = require('../runner/assert'),
      {describe, test, beforeEach, afterEach} = require('node:test'),
      {Canvas, Image, FontLibrary, loadImage, loadCanvas} = require('../../lib');

const BLACK = [0,0,0,255],
      WHITE = [255,255,255,255],
      CLEAR = [0,0,0,0],
      MAGIC = {
        jpg: Buffer.from([0xFF, 0xD8, 0xFF]),
        png: Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]),
        webp: Buffer.from([0x52, 0x49, 0x46, 0x46]),
        pdf: Buffer.from([0x25, 0x50, 0x44, 0x46, 0x2d]),
        svg: Buffer.from(`<?xml version`, 'utf-8')
      },
      MIME = /** @type {const} */ ({
        png: "image/png",
        jpg: "image/jpeg",
        webp: "image/webp",
        pdf: "application/pdf",
        svg: "image/svg+xml"
      });

describe("Canvas", ()=>{
  /** @type {Canvas} */
  let canvas
  /** @type {import('../../lib').CanvasRenderingContext2D} */
  let ctx
  let WIDTH = 512, HEIGHT = 512,
      pixel = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data);

  /** @type {string} */
  let TMP
  let tmpFiles = () =>  fs.readdirSync(TMP)
        .map(fn =>  path.join(TMP, fn) )
        .filter(fn => fs.lstatSync(fn).isFile())


  beforeEach(()=>{
    canvas = new Canvas(WIDTH, HEIGHT)
    ctx = canvas.getContext("2d")
  })

  describe("can get & set", ()=>{
    test('width & height', () => {
      assert.equal(canvas.width, WIDTH)
      assert.equal(canvas.height, HEIGHT)

      ctx.fillStyle = 'white'
      ctx.fillRect(0,0, WIDTH,HEIGHT)
      assert.equal(ctx.fillStyle, '#ffffff')
      assert.deepEqual(pixel(0,0), WHITE)

      // resizing also clears content & resets state
      canvas.width = 123
      canvas.height = 456
      assert.equal(canvas.width, 123)
      assert.equal(canvas.height, 456)
      assert.equal(ctx.fillStyle, '#000000')
      assert.deepEqual(pixel(0,0), CLEAR)
    })

    test('ctx width & height (r/o)', () => {
      // each page keeps the size it was created at, so only the newest matches the canvas
      assert.deepEqual([ctx.width, ctx.height], [WIDTH, HEIGHT])

      let second = canvas.newPage(300, 200)
      assert.deepEqual([second.width, second.height], [300, 200])
      assert.deepEqual([canvas.width, canvas.height], [300, 200])
      assert.deepEqual([ctx.width, ctx.height], [WIDTH, HEIGHT]) // the first page is unchanged

      // and they track a resize of the page that's current
      canvas.width = 42
      assert.deepEqual([second.width, second.height], [42, 200])
      assert.deepEqual([ctx.width, ctx.height], [WIDTH, HEIGHT])
    })
  })

  describe("handles bad arguments for", ()=>{
    beforeEach(() => TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'skia-canvas-')) )
    afterEach(() => fs.rmSync(TMP, {recursive:true}) )

    test("initial dimensions", () => {
      let W = 300,
          H = 150,
          c

      c = new Canvas()
      assert.equal(c.width, W)
      assert.equal(c.height, H)

      c = new Canvas(0, 0)
      assert.equal(c.width, 0)
      assert.equal(c.height, 0)

      c = new Canvas(-99, 123)
      assert.equal(c.width, W)
      assert.equal(c.height, 123)

      c = new Canvas(456)
      assert.equal(c.width, 456)
      assert.equal(c.height, H)

      // @ts-expect-error
      c = new Canvas("0xff")
      assert.equal(c.width, 255)
      assert.equal(c.height, H)

      c = new Canvas(undefined, 789)
      assert.equal(c.width, W)
      assert.equal(c.height, 789)

      // @ts-expect-error
      c = new Canvas('garbage', NaN)
      assert.equal(c.width, W)
      assert.equal(c.height, H)

      // @ts-expect-error
      c = new Canvas(true, {})
      assert.equal(c.width, 1)
      assert.equal(c.height, H)
    })

    test("new page dimensions", () => {
      assert.equal(canvas.width, WIDTH)
      assert.equal(canvas.height, HEIGHT)
      assert.equal(canvas.pages.length, 1)
      canvas.getContext()
      assert.equal(canvas.pages.length, 1)
      canvas.newPage()
      assert.equal(canvas.pages.length, 2)

      let W = 300,
          H = 150,
          c, pg

      c = new Canvas(123, 456)
      assert.equal(c.width, 123)
      assert.equal(c.height, 456)

      assert.equal(c.pages.length, 0)
      pg = c.newPage().canvas
      assert.equal(c.pages.length, 1)
      c.getContext()
      assert.equal(c.pages.length, 1)

      assert.equal(pg.width, 123)
      assert.equal(pg.height, 456)

      pg = c.newPage(987).canvas
      assert.equal(pg.width, 123)
      assert.equal(pg.height, 456)

      pg = c.newPage(NaN, NaN).canvas
      assert.equal(pg.width, W)
      assert.equal(pg.height, H)
    })

    test("export file formats", async () => {
      assert.throws(() => canvas.toFile(`${TMP}/output.gif`) , /Unsupported file format/);
      assert.throws(() => canvas.toFile(`${TMP}/output.targa`) , /Unsupported file format/);
      assert.throws(() => canvas.toFile(`${TMP}/output`) , /Cannot determine image format/);
      assert.throws(() => canvas.toFile(`${TMP}/`) , /Cannot determine image format/);
      await canvas.toFile(`${TMP}/output`, {format:'png'});
    })

  })

  describe("can create | async", ()=>{
    beforeEach(() => {
      TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'skia-canvas-'))

      ctx.fillStyle = 'red'
      ctx.arc(100, 100, 25, 0, Math.PI/2)
      ctx.fill()
    })
    afterEach(() => fs.rmSync(TMP, {recursive:true}) )

    test("JPEGs", async ()=>{
      await Promise.all([
        canvas.toFile(`${TMP}/output1.jpg`),
        canvas.toFile(`${TMP}/output2.jpeg`),
        canvas.toFile(`${TMP}/output3.JPG`),
        canvas.toFile(`${TMP}/output4.JPEG`),
        canvas.toFile(`${TMP}/output5`, {format:'jpg'}),
        canvas.toFile(`${TMP}/output6`, {format:'jpeg'}),
        canvas.toFile(`${TMP}/output6.png`, {format:'jpeg'}),
      ])

      let magic = MAGIC.jpg
      for (let path of tmpFiles()){
        let header = fs.readFileSync(path).slice(0, magic.length)
        assert(header.equals(magic))
      }
    })

    test("PNGs", async ()=>{
      await Promise.all([
        canvas.toFile(`${TMP}/output1.png`),
        canvas.toFile(`${TMP}/output2.PNG`),
        canvas.toFile(`${TMP}/output3`, {format:'png'}),
        canvas.toFile(`${TMP}/output4.svg`, {format:'png'}),
      ])

      let magic = MAGIC.png
      for (let path of tmpFiles()){
        let header = fs.readFileSync(path).slice(0, magic.length)
        assert(header.equals(magic))
      }
    })

    test("WEBPs", async ()=>{
      await Promise.all([
        canvas.toFile(`${TMP}/output1.webp`),
        canvas.toFile(`${TMP}/output2.WEBP`),
        canvas.toFile(`${TMP}/output3`, {format:'webp'}),
        canvas.toFile(`${TMP}/output4.svg`, {format:'webp'}),
      ])

      let magic = MAGIC.webp
      for (let path of tmpFiles()){
        let header = fs.readFileSync(path).slice(0, magic.length)
        assert(header.equals(magic))
      }
    })

    test("SVGs", async ()=>{
      await Promise.all([
        canvas.toFile(`${TMP}/output1.svg`),
        canvas.toFile(`${TMP}/output2.SVG`),
        canvas.toFile(`${TMP}/output3`, {format:'svg'}),
        canvas.toFile(`${TMP}/output4.jpeg`, {format:'svg'}),
      ])

      for (let path of tmpFiles()){
        let svg = fs.readFileSync(path, 'utf-8')
        assert.match(svg, /^<\?xml version/)
      }
    })

    test("PDFs", async ()=>{
      await Promise.all([
        canvas.toFile(`${TMP}/output1.pdf`),
        canvas.toFile(`${TMP}/output2.PDF`),
        canvas.toFile(`${TMP}/output3`, {format:'pdf'}),
        canvas.toFile(`${TMP}/output4.jpg`, {format:'pdf'}),
      ])

      let magic = MAGIC.pdf
      for (let path of tmpFiles()){
        let header = fs.readFileSync(path).slice(0, magic.length)
        assert(header.equals(magic))
      }
    })

    test("raw pixel buffers", async () => {
      canvas.width = canvas.height = 4
      ctx.fillStyle='#f00'
      ctx.fillRect(0,0,1,1)
      ctx.fillStyle='#0f0'
      ctx.fillRect(1,0,1,1)
      ctx.fillStyle='#00f'
      ctx.fillRect(0,1,1,1)
      ctx.fillStyle='#fff'
      ctx.fillRect(1,1,1,1)

      let rgba = ctx.getImageData(0, 0, 2, 2)
      assert.deepEqual(rgba.data, new Uint8ClampedArray([
        255, 0,   0,   255,
        0,   255, 0,   255,
        0,   0,   255, 255,
        255, 255, 255, 255
      ]))

      let bgra = ctx.getImageData(0, 0, 2, 2, {colorType:"bgra"})
      assert.deepEqual(bgra.data, new Uint8ClampedArray([
        0,   0,   255, 255,
        0,   255, 0,   255,
        255, 0,   0,   255,
        255, 255, 255, 255
      ]))

    })

    test("image-sequences", async () => {
      let colors = ['orange', 'yellow', 'green', 'skyblue', 'purple']
      colors.forEach((color, i) => {
        let dim = 512 + 100*i
        ctx = i ? canvas.newPage(dim, dim) : canvas.newPage()
        ctx.fillStyle = color
        ctx.arc(100, 100, 25, 0, Math.PI + Math.PI/colors.length*(i+1))
        ctx.fill()
        assert.equal(ctx.canvas.height, dim)
        assert.equal(ctx.canvas.width, dim)
      })

      await canvas.toFile(`${TMP}/output-{2}.png`)

      let files = tmpFiles()
      assert.equal(files.length, colors.length+1)

      for (const [i, fn] of files.entries()){
        let img = new Image()
        img.src = fn
        await img.decode()
        assert.equal(img.complete, true)

        // second page inherits the first's size, then they increase
        let dim = i<2 ? 512 : 512 + 100 * (i-1)
        assert.equal(img.width, dim)
        assert.equal(img.height, dim)
      }

      // each file in a sequence is written in its own page's color space, so pages that
      // disagree don't have to be reconciled down to a single value for the batch
      let seq = async (...spaces) => {
        let dir = fs.mkdtempSync(path.join(os.tmpdir(), 'skia-canvas-')),
            canvas = new Canvas(4, 4)
        for (const colorSpace of spaces){
          let ctx = canvas.pages.length ? canvas.newPage({colorSpace}) : canvas.getContext('2d', {colorSpace})
          ctx.fillStyle = '#f00'
          ctx.fillRect(0, 0, 4, 4)
        }
        await canvas.toFile(`${dir}/seq-{2}.raw`)
        return fs.readdirSync(dir).sort()
          .map(fn => Array.from(fs.readFileSync(path.join(dir, fn)).slice(0, 4)))
      }

      let P3 = [234, 51, 35, 255], SRGB = [255, 0, 0, 255]

      // pages that agree on a space all export in it
      assert.deepEqual(await seq('display-p3', 'display-p3'), [P3, P3])
      assert.deepEqual(await seq('srgb', 'srgb'), [SRGB, SRGB])

      // …and a mixed batch writes each file in its own page's space, in page order
      assert.deepEqual(await seq('display-p3', 'srgb'), [P3, SRGB])
      assert.deepEqual(await seq('srgb', 'display-p3'), [SRGB, P3])
    })

    test("multi-page PDFs", async () => {
      let colors = ['orange', 'yellow', 'green', 'skyblue', 'purple']
      colors.forEach((color, i) => {
        ctx = canvas.newPage()
        ctx.fillStyle = color
        ctx.fillRect(0, 0, canvas.width, canvas.height)
        ctx.fillStyle = 'white'
        ctx.textAlign = 'center'
        ctx.fillText(`${i+1}`, canvas.width/2, canvas.height/2)
      })

      let path = `${TMP}/multipage.pdf`
      await canvas.toFile(path)

      let header = fs.readFileSync(path).slice(0, MAGIC.pdf.length)
      assert(header.equals(MAGIC.pdf))
    })

    test("image Buffers", async () => {
      for (let ext of /** @type {const} */ (["png", "jpg", "pdf", "svg"])){
        // use extension to specify type
        let path = `${TMP}/output.${ext}`
        let buf = await canvas.toBuffer(ext)
        assert(buf instanceof Buffer)

        fs.writeFileSync(path, buf)
        let header = fs.readFileSync(path).slice(0, MAGIC[ext].length)
        assert(header.equals(MAGIC[ext]))

        // use mime to specify type
        path = `${TMP}/bymime.${ext}`
        buf = await canvas.toBuffer(MIME[ext])
        assert(buf instanceof Buffer)

        fs.writeFileSync(path, buf)
        header = fs.readFileSync(path).slice(0, MAGIC[ext].length)
        assert(header.equals(MAGIC[ext]))
      }
    })

    test("data URLs", async () => {
      for (let key in MIME){
        let ext = /** @type {keyof typeof MIME} */ (key),
            magic = MAGIC[ext],
            mime = MIME[ext],
            [extURL, mimeURL] = await Promise.all([
              canvas.toDataURL(ext),
              canvas.toDataURL(mime),
            ]),
            header = `data:${mime};base64,`,
            data = Buffer.from(extURL.substr(header.length), 'base64')
        assert.equal(extURL, mimeURL)
        assert.equal(extURL.startsWith(header), true)
        assert(data.slice(0, magic.length).equals(magic))
      }
    })

    test("sensible error messages", async () => {
      ctx.fillStyle = 'lightskyblue'
      ctx.fillRect(0, 0, canvas.width, canvas.height)

      // invalid path
      await assert.rejects(canvas.toFile(`${TMP}/deep/path/that/doesn/not/exist.pdf`))

      // canvas has a zero dimension
      let width = 0, height = 128
      Object.assign(canvas, {width, height})
      assert.matchesSubset(canvas, {width, height})
      await assert.rejects(canvas.toFile(`${TMP}/zeroed.png`), /must be non-zero/)
    })
  })

  describe("can export with options", ()=>{
    test("density", async () => {
      let small = new Canvas(50, 50),
          ctx = small.getContext('2d')
      ctx.fillStyle = 'red'
      ctx.fillRect(10, 10, 30, 30)

      let img = await loadImage(await small.toBuffer('png', {density:2}))
      assert.equal(img.width, 100)
      assert.equal(img.height, 100)

      // content scales up with the pixel grid
      let dest = new Canvas(100, 100),
          dtx = dest.getContext('2d')
      dtx.drawImage(img, 0, 0)
      assert.deepEqual(Array.from(dtx.getImageData(50, 50, 1, 1).data), [255, 0, 0, 255])
      assert.deepEqual(Array.from(dtx.getImageData(10, 10, 1, 1).data), CLEAR)
    })

    test("matte", async () => {
      // a matte is a backdrop for the export, not content the canvas is allowed to erase. a
      // region-affecting composite makes the page depend on its own prior pixels, and replaying
      // it onto the matte let it treat the matte as those pixels and wipe it out — so both modes
      // have to leave the same square over the same backdrop
      for (const mode of ['source-over', 'destination-in']){
        ctx.reset()
        ctx.fillStyle = 'red'
        if (mode == 'destination-in'){
          ctx.fillRect(0, 0, WIDTH, HEIGHT) // fill everything, then erase back to just the square
          ctx.globalCompositeOperation = mode
        }
        ctx.fillRect(100, 100, 100, 100)
        assert.deepEqual(pixel(50, 50), CLEAR)
        assert.deepEqual(pixel(150, 150), [255, 0, 0, 255])

        let img = await loadImage(await canvas.toBuffer('png', {matte:'white'}))
        let flat = new Canvas(WIDTH, HEIGHT),
            ftx = flat.getContext('2d')
        ftx.drawImage(img, 0, 0)
        assert.deepEqual(Array.from(ftx.getImageData(50, 50, 1, 1).data), WHITE, `matte erased by ${mode}`)
        assert.deepEqual(Array.from(ftx.getImageData(150, 150, 1, 1).data), [255, 0, 0, 255], `content lost with ${mode}`)
      }
    })

    test("quality", async () => {
      // noisy content so compression quality has bytes to trade away
      for (let i = 0; i < 50; i++){
        ctx.fillStyle = `hsl(${i * 7}, 80%, 50%)`
        ctx.beginPath()
        ctx.arc(WIDTH/2, HEIGHT/2, 250 - i * 5, 0, 2 * Math.PI)
        ctx.fill()
      }
      let hi = await canvas.toBuffer('jpg', {quality:1.0}),
          lo = await canvas.toBuffer('jpg', {quality:0.2})
      assert(lo.length < hi.length / 2)
    })

    test("outline", async () => {
      FontLibrary.use(`tests/assets/fonts/Monoton-Regular.woff`)
      ctx.font = '40px Monoton'
      ctx.fillText('Hi', 20, 60)

      let glyphs = (await canvas.toBuffer('svg')).toString(),
          outlined = (await canvas.toBuffer('svg', {outline:true})).toString()
      assert.equal(glyphs.includes('<text'), true)
      assert.equal(outlined.includes('<text'), false)
      assert.equal(outlined.includes('<path'), true)
    })

    test("colorSpace", async () => {
      // a page renders in the space its context was created with — there's no export-time setting
      let wide = new Canvas(8, 8),
          wideCtx = wide.getContext('2d', {colorSpace:'display-p3'})
      canvas.width = canvas.height = 8
      for (const c of [ctx, wideCtx]){
        c.fillStyle = '#f00'
        c.fillRect(0, 0, 8, 8)
      }

      // raw exports convert pixel values into the page's space
      let srgb = await canvas.toBuffer('raw'),
          p3 = await wide.toBuffer('raw')
      assert.deepEqual(Array.from(srgb.slice(0, 4)), [255, 0, 0, 255])
      assert.deepEqual(Array.from(p3.slice(0, 4)), [234, 51, 35, 255])

      // png output embeds an ICC profile for display-p3 (vs a bare sRGB chunk by default)
      let sPng = await canvas.toBuffer('png'),
          pPng = await wide.toBuffer('png')
      assert(sPng.includes('sRGB') && !sPng.includes('iCCP'))
      assert(pPng.includes('iCCP') && !pPng.includes('sRGB'))

      // …while jpeg embeds it in an APP2 segment
      let pJpg = await wide.toBuffer('jpg')
      assert(pJpg.includes('ICC_PROFILE'))
    })

    test("colorSpace (RGBAF16)", async () => {
      // decode an IEEE-754 binary16 (half-float) channel from a raw buffer
      let f16 = (buf, i) => {
        let h = buf.readUInt16LE(i*2),
            sign = (h & 0x8000) ? -1 : 1,
            exp = (h >> 10) & 0x1f,
            frac = h & 0x3ff
        return exp==0    ? sign * 2**-14 * (frac/1024)
             : exp==0x1f ? sign * (frac ? NaN : Infinity)
             :             sign * 2**(exp-15) * (1 + frac/1024)
      }

      let wide = new Canvas(8, 8),
          wideCtx = wide.getContext('2d', {colorSpace:'display-p3'})
      canvas.width = canvas.height = 8
      for (const c of [ctx, wideCtx]){
        c.fillStyle = '#f00'
        c.fillRect(0, 0, 8, 8)
      }

      // an RGBAF16 raw export carries 8 bytes/pixel (4 channels × binary16), and
      // its display-p3 values match the 8-bit conversion ([234, 51, 35, 255]) — but
      // at float precision rather than quantized to 1/255 steps. Note that F16 buys
      // precision here, not out-of-gamut headroom: read_pixels still clamps to the
      // destination gamut during conversion regardless of bit depth.
      let p3 = await wide.toBuffer('raw', {colorType:'RGBAF16'})
      assert.equal(p3.length, 8 * 8 * 8)
      assert.nearEqual(f16(p3, 0), 234/255)
      assert.nearEqual(f16(p3, 1), 51/255)
      assert.nearEqual(f16(p3, 2), 35/255)
      assert.nearEqual(f16(p3, 3), 1.0)

      // the same colorType works for the default sRGB space (pure red is exact in F16)
      let srgb = await canvas.toBuffer('raw', {colorType:'RGBAF16'})
      assert.equal(srgb.length, 8 * 8 * 8)
      assert.deepEqual([0,1,2,3].map(c => f16(srgb, c)), [1, 0, 0, 1])
    })
  })

  describe("can create | sync", ()=>{
    beforeEach(() => {
      TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'skia-canvas-'))

      ctx.fillStyle = 'red'
      ctx.arc(100, 100, 25, 0, Math.PI/2)
      ctx.fill()
    })
    afterEach(() => fs.rmSync(TMP, {recursive:true}) )

    test("JPEGs", ()=>{
      canvas.toFileSync(`${TMP}/output1.jpg`)
      canvas.toFileSync(`${TMP}/output2.jpeg`)
      canvas.toFileSync(`${TMP}/output3.JPG`)
      canvas.toFileSync(`${TMP}/output4.JPEG`)
      canvas.toFileSync(`${TMP}/output5`, {format:'jpg'})
      canvas.toFileSync(`${TMP}/output6`, {format:'jpeg'})
      canvas.toFileSync(`${TMP}/output6.png`, {format:'jpeg'})

      let magic = MAGIC.jpg
      for (let path of tmpFiles()){
        let header = fs.readFileSync(path).slice(0, magic.length)
        assert(header.equals(magic))
      }
    })

    test("PNGs", ()=>{
      canvas.toFileSync(`${TMP}/output1.png`)
      canvas.toFileSync(`${TMP}/output2.PNG`)
      canvas.toFileSync(`${TMP}/output3`, {format:'png'})
      canvas.toFileSync(`${TMP}/output4.svg`, {format:'png'})

      let magic = MAGIC.png
      for (let path of tmpFiles()){
        let header = fs.readFileSync(path).slice(0, magic.length)
        assert(header.equals(magic))
      }
    })

    test("WEBPs", async ()=>{
      await Promise.all([
        canvas.toFileSync(`${TMP}/output1.webp`),
        canvas.toFileSync(`${TMP}/output2.WEBP`),
        canvas.toFileSync(`${TMP}/output3`, {format:'webp'}),
        canvas.toFileSync(`${TMP}/output4.svg`, {format:'webp'}),
      ])

      let magic = MAGIC.webp
      for (let path of tmpFiles()){
        let header = fs.readFileSync(path).slice(0, magic.length)
        assert(header.equals(magic))
      }
    })

    test("SVGs", ()=>{
      canvas.toFileSync(`${TMP}/output1.svg`)
      canvas.toFileSync(`${TMP}/output2.SVG`)
      canvas.toFileSync(`${TMP}/output3`, {format:'svg'})
      canvas.toFileSync(`${TMP}/output4.jpeg`, {format:'svg'})

      for (let path of tmpFiles()){
        let svg = fs.readFileSync(path, 'utf-8')
        assert.match(svg, /^<\?xml version/)
      }
    })

    test("PDFs", ()=>{
      canvas.toFileSync(`${TMP}/output1.pdf`)
      canvas.toFileSync(`${TMP}/output2.PDF`)
      canvas.toFileSync(`${TMP}/output3`, {format:'pdf'})
      canvas.toFileSync(`${TMP}/output4.jpg`, {format:'pdf'})

      let magic = MAGIC.pdf
      for (let path of tmpFiles()){
        let header = fs.readFileSync(path).slice(0, magic.length)
        assert(header.equals(magic))
      }
    })

    test("image-sequences", async ()=>{
      let colors = ['orange', 'yellow', 'green', 'skyblue', 'purple']
      colors.forEach((color, i) => {
        let dim = 512 + 100*i
        ctx = i ? canvas.newPage(dim, dim) : canvas.newPage()
        ctx.fillStyle = color
        ctx.arc(100, 100, 25, 0, Math.PI + Math.PI/colors.length*(i+1))
        ctx.fill()
        assert.equal(ctx.canvas.height, dim)
        assert.equal(ctx.canvas.width, dim)
      })

      canvas.toFileSync(`${TMP}/output-{2}.png`)

      let files = tmpFiles()
      assert.equal(files.length, colors.length+1)

      for (const [i, fn] of files.entries()){
        let img = new Image()
        img.src = fn
        await img.decode()
        assert.equal(img.complete, true)

        // second page inherits the first's size, then they increase
        let dim = i<2 ? 512 : 512 + 100 * (i-1)
        assert.equal(img.width, dim)
        assert.equal(img.height, dim)
      }
    })


    test("multi-page PDFs", () => {
      let colors = ['orange', 'yellow', 'green', 'skyblue', 'purple']
      colors.forEach((color, i) => {
        ctx = canvas.newPage()
        ctx.fillStyle = color
        ctx.fillRect(0, 0, canvas.width, canvas.height)
        ctx.fillStyle = 'white'
        ctx.textAlign = 'center'
        ctx.fillText(`${i+1}`, canvas.width/2, canvas.height/2)
      })

      let path = `${TMP}/multipage.pdf`
      assert.doesNotThrow(() => canvas.toFileSync(path) )

      let header = fs.readFileSync(path).slice(0, MAGIC.pdf.length)
      assert(header.equals(MAGIC.pdf))
    })

    test("image Buffers", () => {
      for (let ext of /** @type {const} */ (["png", "jpg", "pdf", "svg"])){
        // use extension to specify type
        let path = `${TMP}/output.${ext}`
        let buf = canvas.toBufferSync(ext)
        assert(buf instanceof Buffer)

        fs.writeFileSync(path, buf)
        let header = fs.readFileSync(path).slice(0, MAGIC[ext].length)
        assert(header.equals(MAGIC[ext]))

        // use mime to specify type
        path = `${TMP}/bymime.${ext}`
        buf = canvas.toBufferSync(MIME[ext])
        assert(buf instanceof Buffer)

        fs.writeFileSync(path, buf)
        header = fs.readFileSync(path).slice(0, MAGIC[ext].length)
        assert(header.equals(MAGIC[ext]))
      }
    })

    test("data URLs", async () => {
      for (let key in MIME){
        let ext = /** @type {keyof typeof MIME} */ (key),
            magic = MAGIC[ext],
            mime = MIME[ext],
            extURL = canvas.toURLSync(ext),
            mimeURL = canvas.toURLSync(mime),
            stdURL = canvas.toDataURL(mime, 0.92),
            asyncURL = await canvas.toURL(ext),
            header = `data:${mime};base64,`,
            data = Buffer.from(extURL.substr(header.length), 'base64')
        assert.equal(extURL, mimeURL)
        assert.equal(extURL, stdURL)
        assert.equal(extURL, asyncURL)
        assert(extURL.startsWith(header))
        assert(data.slice(0, magic.length).equals(magic))
      }
    })

    test("sensible error messages", () => {
      ctx.fillStyle = 'lightskyblue'
      ctx.fillRect(0, 0, canvas.width, canvas.height)

      // invalid path
      assert.throws(() => canvas.toFileSync(`${TMP}/deep/path/that/doesn/not/exist.pdf`))

      // canvas has a zero dimension
      let width = 0, height = 128
      Object.assign(canvas, {width, height})
      assert.matchesSubset(canvas, {width, height})
      assert.throws( () => canvas.toFileSync(`${TMP}/zeroed.png`), /must be non-zero/)
    })

    test("an image even without a ctx", () => {
      let canvas = new Canvas(200, 200)
      assert.doesNotThrow( () => canvas.toURLSync("png") )
    })
  })

  describe("loadCanvas()", () => {
    var PNG_PATH = 'tests/assets/pentagon.png',
        SVG_PATH = 'tests/assets/image/format.svg',
        firstPixel = ctx => Array.from(ctx.getImageData(0, 0, 1, 1).data)
  
    // a 3-page document: red, a deliberately blank middle page, then blue on a wider final page
    const makePdf = async () => {
      let canvas = new Canvas(100, 100),
          ctx = canvas.getContext('2d')
      ctx.fillStyle = '#f00'
      ctx.fillRect(0, 0, 300, 300)
      canvas.newPage(120, 90) // left empty
      ctx = canvas.newPage(200, 100)
      ctx.fillStyle = '#00f'
      ctx.fillRect(0, 0, 300, 300)
      return canvas.toBuffer('pdf')
    }
  
    test("reads multipage PDFs", async () => {
      let doc = await loadCanvas(await makePdf())
  
      // one canvas page per document page, each at its own size…
      assert.equal(doc.pages.length, 3)
      assert.deepEqual(doc.pages.map(p => [p.width, p.height]), [[100, 100], [120, 90], [200, 100]])
  
      // …with the canvas itself sized to the last page, as after any newPage()
      assert.deepEqual([doc.width, doc.height], [200, 100])
  
      assert.deepEqual(firstPixel(doc.pages[0]), [255, 0, 0, 255])
      assert.deepEqual(firstPixel(doc.pages[1]), [0, 0, 0, 0]) // the blank page survives the load
      assert.deepEqual(firstPixel(doc.pages[2]), [0, 0, 255, 255])
    })
  
    test("can re-export multipage docs", async () => {
      // the pages hold real geometry, so the document can make a round trip
      let doc = await loadCanvas(await makePdf()),
          again = await loadCanvas(await doc.toBuffer('pdf'))
  
      assert.equal(again.pages.length, 3)
      assert.deepEqual(again.pages.map(p => [p.width, p.height]), [[100, 100], [120, 90], [200, 100]])
      assert.deepEqual(firstPixel(again.pages[2]), [0, 0, 255, 255])
    })
  
    test("reads single-page sources", async () => {
      // a bitmap is sized to its pixel dimensions and pre-drawn
      let bitmap = await loadCanvas(PNG_PATH)
      assert.equal(bitmap.pages.length, 1)
      assert.deepEqual([bitmap.width, bitmap.height], [125, 125])
  
      // as is an SVG with an intrinsic size
      let vector = await loadCanvas(SVG_PATH)
      assert.equal(vector.pages.length, 1)
      assert.deepEqual([vector.width, vector.height], [60, 60])
  
      // and a one-page PDF is just a document that happens to be short
      let solo = new Canvas(50, 60)
      solo.getContext('2d').fillStyle = '#0f0'
      solo.getContext('2d').fillRect(0, 0, 99, 99)
      let short = await loadCanvas(await solo.toBuffer('pdf'))
      assert.equal(short.pages.length, 1)
      assert.deepEqual([short.width, short.height], [50, 60])
      assert.deepEqual(firstPixel(short.getContext('2d')), [0, 255, 0, 255])
    })
  
    test("uses the viewBox for sizeless SVGs", async () => {
      let svg = Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 250"></svg>')
      assert.deepEqual((c => [c.width, c.height])(await loadCanvas(svg)), [400, 250])
  
      // …while loadImage() keeps reporting Chrome's 150px-tall default for the same file
      assert.matchesSubset(await loadImage(svg), {width:240, height:150})
  
      // with no viewBox to go on, both fall back to that default (Chrome's 300×150 default object size)
      let bare = Buffer.from('<svg xmlns="http://www.w3.org/2000/svg"></svg>')
      assert.deepEqual((c => [c.width, c.height])(await loadCanvas(bare)), [300, 150])
    })
  
    test("parses canvas & context options", async () => {
      let doc = await loadCanvas(PNG_PATH, {colorSpace:'display-p3', gpu:false, textGamma:1.8})
      assert.equal(doc.getContext('2d').getContextAttributes().colorSpace, 'display-p3')
      assert.equal(doc.gpu, false)
      assert.equal(doc.engine.textGamma, 1.8)
  
      // every page of a document inherits the settings, not just the first
      let pdf = await loadCanvas(await makePdf(), {colorSpace:'display-p3'})
      for (let page of pdf.pages){
        assert.equal(page.getContextAttributes().colorSpace, 'display-p3')
      }
  
      // an unusable colorSpace is silently ignored rather than thrown (outside of strict mode)
      // @ts-expect-error — deliberately passing an unknown color space
      let fallback = await loadCanvas(PNG_PATH, {colorSpace:'nonsense'})
      assert.equal(fallback.getContext('2d').getContextAttributes().colorSpace, 'srgb')
    })
  
    test("rejects undecodable data", async () => {
      await assert.rejects(loadCanvas(Buffer.from('not an image')), /Could not decode/)
    })
  })
})
