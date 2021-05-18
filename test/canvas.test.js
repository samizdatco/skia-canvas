const _ = require('lodash'),
      fs = require('fs'),
      tmp = require('tmp'),
      glob = require('glob').sync,
      {Canvas, Image} = require('../lib');

const BLACK = [0,0,0,255],
      WHITE = [255,255,255,255],
      CLEAR = [0,0,0,0]

describe("Canvas", ()=>{
  let canvas, ctx,
      WIDTH = 512, HEIGHT = 512,
      pixel = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data);

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
    let TMP
    beforeEach(() => TMP = tmp.dirSync().name )
    afterEach(() => fs.rmdirSync(TMP, {recursive:true}) )

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

      c = new Canvas('garbage', NaN)
      expect(c.width).toBe(W)
      expect(c.height).toBe(H)

      c = new Canvas(false, {})
      expect(c.width).toBe(W)
      expect(c.height).toBe(H)
    })

    test("new page dimensions", () => {
      let W = 300,
          H = 150,
          c, pg

      c = new Canvas(123, 456)
      expect(c.width).toBe(123)
      expect(c.height).toBe(456)
      pg = c.newPage().canvas
      expect(pg.width).toBe(123)
      expect(pg.height).toBe(456)

      pg = c.newPage(987).canvas
      expect(pg.width).toBe(123)
      expect(pg.height).toBe(456)

      pg = c.newPage(NaN, NaN).canvas
      expect(pg.width).toBe(W)
      expect(pg.height).toBe(H)
    })

    test("export file formats", () => {
      expect(() => canvas.saveAs(`${TMP}/output.gif`) ).toThrowError('Unsupported file format');
      expect(() => canvas.saveAs(`${TMP}/output.targa`) ).toThrowError('Unsupported file format');
      expect(() => canvas.saveAs(`${TMP}/output`) ).toThrowError('Cannot determine image format');
      expect(() => canvas.saveAs(`${TMP}/`) ).toThrowError('Cannot determine image format');
      expect(() => canvas.saveAs(`${TMP}/output`, {format:'png'}) ).not.toThrow()
    })

  })

  describe("can create", ()=>{
    let TMP
    beforeEach(() => {
      TMP = tmp.dirSync().name

      ctx.fillStyle = 'red'
      ctx.arc(100, 100, 25, 0, Math.PI/2)
      ctx.fill()
    })
    afterEach(() => fs.rmdirSync(TMP, {recursive:true}) )

    test("JPEGs", ()=>{
      canvas.saveAs(`${TMP}/output1.jpg`)
      canvas.saveAs(`${TMP}/output2.jpeg`)
      canvas.saveAs(`${TMP}/output3.JPG`)
      canvas.saveAs(`${TMP}/output4.JPEG`)
      canvas.saveAs(`${TMP}/output5`, {format:'jpg'})
      canvas.saveAs(`${TMP}/output6`, {format:'jpeg'})
      canvas.saveAs(`${TMP}/output6.png`, {format:'jpeg'})

      let magic = Buffer.from([0xFF, 0xD8, 0xFF])
      for (let path of glob(`${TMP}/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
      }
    })

    test("PNGs", ()=>{
      canvas.saveAs(`${TMP}/output1.png`)
      canvas.saveAs(`${TMP}/output2.PNG`)
      canvas.saveAs(`${TMP}/output3`, {format:'png'})
      canvas.saveAs(`${TMP}/output4.svg`, {format:'png'})

      let magic = Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A])
      for (let path of glob(`${TMP}/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
      }
    })

    test("SVGs", ()=>{
      canvas.saveAs(`${TMP}/output1.svg`)
      canvas.saveAs(`${TMP}/output2.SVG`)
      canvas.saveAs(`${TMP}/output3`, {format:'svg'})
      canvas.saveAs(`${TMP}/output4.jpeg`, {format:'svg'})

      for (let path of glob(`${TMP}/*`)){
        let svg = fs.readFileSync(path, 'utf-8')
        expect(svg).toMatch(/^<\?xml version/)
      }
    })

    test("PDFs", ()=>{
      canvas.saveAs(`${TMP}/output1.pdf`)
      canvas.saveAs(`${TMP}/output2.PDF`)
      canvas.saveAs(`${TMP}/output3`, {format:'pdf'})
      canvas.saveAs(`${TMP}/output4.jpg`, {format:'pdf'})

      let magic = Buffer.from([0x25, 0x50, 0x44, 0x46, 0x2d])
      for (let path of glob(`${TMP}/*`)){
        let header = fs.readFileSync(path).slice(0, magic.length)
        expect(header.equals(magic)).toBe(true)
      }
    })

    test("image-sequences", ()=>{
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

      canvas.saveAs(`${TMP}/output-{2}.png`)

      let files = glob(`${TMP}/output-0?.png`)
      expect(files.length).toEqual(colors.length+1)

      files.forEach((fn, i) => {
        let img = new Image()
        img.src = fn
        expect(img.complete).toBe(true)

        // second page inherits the first's size, then they increase
        let dim = i<2 ? 512 : 512 + 100 * (i-1)
        expect(img.width).toEqual(dim)
        expect(img.height).toEqual(dim)
      })
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
      expect(() => canvas.saveAs(`${TMP}/multipage.pdf`) ).not.toThrow()
    })

    test("sensible errors for misbegotten exports", () => {
      ctx.fillStyle = 'lightskyblue'
      ctx.fillRect(0, 0, canvas.width, canvas.height)

      // invalid path
      expect(() =>
        canvas.saveAs(`${TMP}/deep/path/that/doesn/not/exist.pdf`)
      ).toThrow()

      // canvas has a zero dimension
      let width = 0, height = 128
      Object.assign(canvas, {width, height})
      expect(canvas).toMatchObject({width, height})
      canvas.saveAs(`${TMP}/zeroed.pdf`)
      expect( () => canvas.saveAs(`${TMP}/zeroed.png`)).toThrowError("must be non-zero")
    })
  })
})
