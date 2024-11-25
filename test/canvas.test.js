// @ts-check

const _ = require('lodash'),
      fs = require('fs'),
      tmp = require('tmp'),
      glob = require('glob').sync,
      {Canvas, Image} = require('../lib');

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
      MIME = {
        png: "image/png",
        jpg: "image/jpeg",
        webp: "image/webp",
        pdf: "application/pdf",
        svg: "image/svg+xml"
      };

describe("Canvas", ()=>{
  let canvas, ctx,
      WIDTH = 512, HEIGHT = 512,
      pixel = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data);

  let TMP,
      findTmp = pattern => glob(pattern, {root:TMP}).sort()

  beforeEach(()=>{
    canvas = new Canvas(WIDTH, HEIGHT)
    ctx = canvas.getContext("2d")
  })

  describe("can get & set", ()=>{
    test('width & height', () => {
      expect(canvas.width).toBe(WIDTH)
      expect(canvas.height).toBe(HEIGHT)

      ctx.fillStyle = 'white'
      ctx.fillRect(0,0, WIDTH,HEIGHT)
      expect(ctx.fillStyle).toBe('#ffffff')
      expect(pixel(0,0)).toEqual(WHITE)

      // resizing also clears content & resets state
      canvas.width = 123
      canvas.height = 456
      expect(canvas.width).toBe(123)
      expect(canvas.height).toBe(456)
      expect(ctx.fillStyle).toBe('#000000')
      expect(pixel(0,0)).toEqual(CLEAR)
    })
  })

  describe("handles bad arguments for", ()=>{
    beforeEach(() => TMP = tmp.dirSync().name )
    afterEach(() => fs.rmSync(TMP, {recursive:true}) )

    test("initial dimensions", () => {
      let W = 300,
          H = 150,
          c

      c = new Canvas()
      expect(c.width).toBe(W)
      expect(c.height).toBe(H)

      c = new Canvas(0, 0)
      expect(c.width).toBe(0)
      expect(c.height).toBe(0)

      c = new Canvas(-99, 123)
      expect(c.width).toBe(W)
      expect(c.height).toBe(123)

      c = new Canvas(456)
      expect(c.width).toBe(456)
      expect(c.height).toBe(H)

      c = new Canvas(undefined, 789)
      expect(c.width).toBe(W)
      expect(c.height).toBe(789)

      // @ts-expect-error
      c = new Canvas('garbage', NaN)
      expect(c.width).toBe(W)
      expect(c.height).toBe(H)

      // @ts-expect-error
      c = new Canvas(false, {})
      expect(c.width).toBe(W)
      expect(c.height).toBe(H)
    })

    test("new page dimensions", () => {
      expect(canvas.width).toBe(WIDTH)
      expect(canvas.height).toBe(HEIGHT)
      expect(canvas.pages.length).toBe(1)
      canvas.getContext()
      expect(canvas.pages.length).toBe(1)
      canvas.newPage()
      expect(canvas.pages.length).toBe(2)

      let W = 300,
          H = 150,
          c, pg

      c = new Canvas(123, 456)
      expect(c.width).toBe(123)
      expect(c.height).toBe(456)

      expect(c.pages.length).toBe(0)
      pg = c.newPage().canvas
      expect(c.pages.length).toBe(1)
      c.getContext()
      expect(c.pages.length).toBe(1)

      expect(pg.width).toBe(123)
      expect(pg.height).toBe(456)

      pg = c.newPage(987).canvas
      expect(pg.width).toBe(123)
      expect(pg.height).toBe(456)

      pg = c.newPage(NaN, NaN).canvas
      expect(pg.width).toBe(W)
      expect(pg.height).toBe(H)
    })

    test("export file formats", async () => {
      expect(() => canvas.saveAs(`${TMP}/output.gif`) ).toThrowError('Unsupported file format');
      expect(() => canvas.saveAs(`${TMP}/output.targa`) ).toThrowError('Unsupported file format');
      expect(() => canvas.saveAs(`${TMP}/output`) ).toThrowError('Cannot determine image format');
      expect(() => canvas.saveAs(`${TMP}/`) ).toThrowError('Cannot determine image format');
      await expect(canvas.saveAs(`${TMP}/output`, {format:'png'}) ).resolves.not.toThrow();
    })

  })

  describe("can create | async", ()=>{
    beforeEach(() => {
      TMP = tmp.dirSync().name

      ctx.fillStyle = 'red'
      ctx.arc(100, 100, 25, 0, Math.PI/2)
      ctx.fill()
    })
    afterEach(() => fs.rmSync(TMP, {recursive:true}) )

    test("JPEGs", async ()=>{
      await Promise.all([
        canvas.saveAs(`${TMP}/output1.jpg`),
        canvas.saveAs(`${TMP}/output2.jpeg`),
        canvas.saveAs(`${TMP}/output3.JPG`),
        canvas.saveAs(`${TMP}/output4.JPEG`),
        canvas.saveAs(`${TMP}/output5`, {format:'jpg'}),
        canvas.saveAs(`${TMP}/output6`, {format:'jpeg'}),
        canvas.saveAs(`${TMP}/output6.png`, {format:'jpeg'}),
      ])

      let magic = MAGIC.jpg
      for (let path of findTmp(`/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
      }
    })

    test("PNGs", async ()=>{
      await Promise.all([
        canvas.saveAs(`${TMP}/output1.png`),
        canvas.saveAs(`${TMP}/output2.PNG`),
        canvas.saveAs(`${TMP}/output3`, {format:'png'}),
        canvas.saveAs(`${TMP}/output4.svg`, {format:'png'}),
      ])

      let magic = MAGIC.png
      for (let path of findTmp(`/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
      }
    })

    test("WEBPs", async ()=>{
      await Promise.all([
        canvas.saveAs(`${TMP}/output1.webp`),
        canvas.saveAs(`${TMP}/output2.WEBP`),
        canvas.saveAs(`${TMP}/output3`, {format:'webp'}),
        canvas.saveAs(`${TMP}/output4.svg`, {format:'webp'}),
      ])

      let magic = MAGIC.webp
      for (let path of findTmp(`/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
      }
    })

    test("SVGs", async ()=>{
      await Promise.all([
        canvas.saveAs(`${TMP}/output1.svg`),
        canvas.saveAs(`${TMP}/output2.SVG`),
        canvas.saveAs(`${TMP}/output3`, {format:'svg'}),
        canvas.saveAs(`${TMP}/output4.jpeg`, {format:'svg'}),
      ])

      for (let path of findTmp(`/*`)){
        let svg = fs.readFileSync(path, 'utf-8')
        expect(svg).toMatch(/^<\?xml version/)
      }
    })

    test("PDFs", async ()=>{
      await Promise.all([
        canvas.saveAs(`${TMP}/output1.pdf`),
        canvas.saveAs(`${TMP}/output2.PDF`),
        canvas.saveAs(`${TMP}/output3`, {format:'pdf'}),
        canvas.saveAs(`${TMP}/output4.jpg`, {format:'pdf'}),
      ])

      let magic = MAGIC.pdf
      for (let path of findTmp(`/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
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
      expect(rgba.data).toEqual(new Uint8ClampedArray([
        255, 0,   0,   255,
        0,   255, 0,   255,
        0,   0,   255, 255,
        255, 255, 255, 255
      ]))

      let bgra = ctx.getImageData(0, 0, 2, 2, {colorType:"bgra"})
      expect(bgra.data).toEqual(new Uint8ClampedArray([
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
        expect(ctx.canvas.height).toEqual(dim)
        expect(ctx.canvas.width).toEqual(dim)
      })

      await canvas.saveAs(`${TMP}/output-{2}.png`)

      let files = findTmp(`/output-0?.png`)
      expect(files.length).toEqual(colors.length+1)

      for (const [i, fn] of files.entries()){
        let img = new Image()
        img.src = fn
        await img.decode()
        expect(img.complete).toBe(true)

        // second page inherits the first's size, then they increase
        let dim = i<2 ? 512 : 512 + 100 * (i-1)
        expect(img.width).toEqual(dim)
        expect(img.height).toEqual(dim)
      }

    })

    test("multi-page PDFs", async () => {
      let colors = ['orange', 'yellow', 'green', 'skyblue', 'purple']
      colors.forEach((color, i) => {
        ctx = canvas.newPage()
        ctx.fillStyle = color
        ctx.fillRect(0, 0, canvas.width, canvas.height)
        ctx.fillStyle = 'white'
        ctx.textAlign = 'center'
        ctx.fillText(i+1, canvas.width/2, canvas.height/2)
      })

      let path = `${TMP}/multipage.pdf`
      await canvas.saveAs(path)

      let header = fs.readFileSync(path).slice(0, MAGIC.pdf.length)
      expect(header.equals(MAGIC.pdf)).toBe(true)
    })

    test("image Buffers", async () => {
      for (let ext of ["png", "jpg", "pdf", "svg"]){
        // use extension to specify type
        let path = `${TMP}/output.${ext}`
        let buf = await canvas.toBuffer(ext)
        expect(buf).toBeInstanceOf(Buffer)

        fs.writeFileSync(path, buf)
        let header = fs.readFileSync(path).slice(0, MAGIC[ext].length)
        expect(header.equals(MAGIC[ext])).toBe(true)

        // use mime to specify type
        path = `${TMP}/bymime.${ext}`
        buf = await canvas.toBuffer(MIME[ext])
        expect(buf).toBeInstanceOf(Buffer)

        fs.writeFileSync(path, buf)
        header = fs.readFileSync(path).slice(0, MAGIC[ext].length)
        expect(header.equals(MAGIC[ext])).toBe(true)
      }
    })

    test("data URLs", async () => {
      for (let ext in MIME){
        let magic = MAGIC[ext],
            mime = MIME[ext],
            [extURL, mimeURL] = await Promise.all([
              canvas.toDataURL(ext),
              canvas.toDataURL(mime),
            ]),
            header = `data:${mime};base64,`,
            data = Buffer.from(extURL.substr(header.length), 'base64')
        expect(extURL).toEqual(mimeURL)
        expect(extURL.startsWith(header)).toBe(true)
        expect(data.slice(0, magic.length)).toEqual(magic)
      }
    })

    test("sensible error messages", async () => {
      ctx.fillStyle = 'lightskyblue'
      ctx.fillRect(0, 0, canvas.width, canvas.height)

      // invalid path
      await expect(canvas.saveAs(`${TMP}/deep/path/that/doesn/not/exist.pdf`))
                  .rejects.toThrow()

      // canvas has a zero dimension
      let width = 0, height = 128
      Object.assign(canvas, {width, height})
      expect(canvas).toMatchObject({width, height})
      await expect(canvas.saveAs(`${TMP}/zeroed.png`)).rejects.toThrowError("must be non-zero")
    })
  })

  describe("can create | sync", ()=>{
    beforeEach(() => {
      TMP = tmp.dirSync().name

      ctx.fillStyle = 'red'
      ctx.arc(100, 100, 25, 0, Math.PI/2)
      ctx.fill()
    })
    afterEach(() => fs.rmSync(TMP, {recursive:true}) )

    test("JPEGs", ()=>{
      canvas.saveAsSync(`${TMP}/output1.jpg`)
      canvas.saveAsSync(`${TMP}/output2.jpeg`)
      canvas.saveAsSync(`${TMP}/output3.JPG`)
      canvas.saveAsSync(`${TMP}/output4.JPEG`)
      canvas.saveAsSync(`${TMP}/output5`, {format:'jpg'})
      canvas.saveAsSync(`${TMP}/output6`, {format:'jpeg'})
      canvas.saveAsSync(`${TMP}/output6.png`, {format:'jpeg'})

      let magic = MAGIC.jpg
      for (let path of findTmp(`/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
      }
    })

    test("PNGs", ()=>{
      canvas.saveAsSync(`${TMP}/output1.png`)
      canvas.saveAsSync(`${TMP}/output2.PNG`)
      canvas.saveAsSync(`${TMP}/output3`, {format:'png'})
      canvas.saveAsSync(`${TMP}/output4.svg`, {format:'png'})

      let magic = MAGIC.png
      for (let path of findTmp(`/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
      }
    })

    test("WEBPs", async ()=>{
      await Promise.all([
        canvas.saveAsSync(`${TMP}/output1.webp`),
        canvas.saveAsSync(`${TMP}/output2.WEBP`),
        canvas.saveAsSync(`${TMP}/output3`, {format:'webp'}),
        canvas.saveAsSync(`${TMP}/output4.svg`, {format:'webp'}),
      ])

      let magic = MAGIC.webp
      for (let path of findTmp(`/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
      }
    })

    test("SVGs", ()=>{
      canvas.saveAsSync(`${TMP}/output1.svg`)
      canvas.saveAsSync(`${TMP}/output2.SVG`)
      canvas.saveAsSync(`${TMP}/output3`, {format:'svg'})
      canvas.saveAsSync(`${TMP}/output4.jpeg`, {format:'svg'})

      for (let path of findTmp(`/*`)){
        let svg = fs.readFileSync(path, 'utf-8')
        expect(svg).toMatch(/^<\?xml version/)
      }
    })

    test("PDFs", ()=>{
      canvas.saveAsSync(`${TMP}/output1.pdf`)
      canvas.saveAsSync(`${TMP}/output2.PDF`)
      canvas.saveAsSync(`${TMP}/output3`, {format:'pdf'})
      canvas.saveAsSync(`${TMP}/output4.jpg`, {format:'pdf'})

      let magic = MAGIC.pdf
      for (let path of findTmp(`/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
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
        expect(ctx.canvas.height).toEqual(dim)
        expect(ctx.canvas.width).toEqual(dim)
      })

      canvas.saveAsSync(`${TMP}/output-{2}.png`)

      let files = findTmp(`/output-0?.png`)
      console.log({files})
      expect(files.length).toEqual(colors.length+1)

      for (const [i, fn] of files.entries()){
        let img = new Image()
        img.src = fn
        await img.decode()
        expect(img.complete).toBe(true)

        console.log({img})

        // second page inherits the first's size, then they increase
        let dim = i<2 ? 512 : 512 + 100 * (i-1)
        expect(img.width).toEqual(dim)
        expect(img.height).toEqual(dim)
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
        ctx.fillText(i+1, canvas.width/2, canvas.height/2)
      })

      let path = `${TMP}/multipage.pdf`
      expect(() => canvas.saveAsSync(path) ).not.toThrow()

      let header = fs.readFileSync(path).slice(0, MAGIC.pdf.length)
      expect(header.equals(MAGIC.pdf)).toBe(true)
    })

    test("image Buffers", () => {
      for (let ext of ["png", "jpg", "pdf", "svg"]){
        // use extension to specify type
        let path = `${TMP}/output.${ext}`
        let buf = canvas.toBufferSync(ext)
        expect(buf).toBeInstanceOf(Buffer)

        fs.writeFileSync(path, buf)
        let header = fs.readFileSync(path).slice(0, MAGIC[ext].length)
        expect(header.equals(MAGIC[ext])).toBe(true)

        // use mime to specify type
        path = `${TMP}/bymime.${ext}`
        buf = canvas.toBufferSync(MIME[ext])
        expect(buf).toBeInstanceOf(Buffer)

        fs.writeFileSync(path, buf)
        header = fs.readFileSync(path).slice(0, MAGIC[ext].length)
        expect(header.equals(MAGIC[ext])).toBe(true)
      }
    })

    test("data URLs", () => {
      for (let ext in MIME){
        let magic = MAGIC[ext],
            mime = MIME[ext],
            extURL = canvas.toDataURLSync(ext),
            mimeURL = canvas.toDataURLSync(mime),
            header = `data:${mime};base64,`,
            data = Buffer.from(extURL.substr(header.length), 'base64')
        expect(extURL).toEqual(mimeURL)
        expect(extURL.startsWith(header)).toBe(true)
        expect(data.slice(0, magic.length)).toEqual(magic)
      }
    })

    test("sensible error messages", () => {
      ctx.fillStyle = 'lightskyblue'
      ctx.fillRect(0, 0, canvas.width, canvas.height)

      // invalid path
      expect(() =>
        canvas.saveAsSync(`${TMP}/deep/path/that/doesn/not/exist.pdf`)
      ).toThrow()

      // canvas has a zero dimension
      let width = 0, height = 128
      Object.assign(canvas, {width, height})
      expect(canvas).toMatchObject({width, height})
      expect( () => canvas.saveAsSync(`${TMP}/zeroed.png`)).toThrowError("must be non-zero")
    })
  })

})
