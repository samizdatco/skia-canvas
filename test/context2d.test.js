// @ts-check

"use strict"

const _ = require('lodash'),
      {Canvas, DOMMatrix, DOMPoint, ImageData, Path2D, FontLibrary, loadImage} = require('../lib'),
      css = require('../lib/classes/css');

const BLACK = [0,0,0,255],
      WHITE = [255,255,255,255],
      GREEN = [0,128,0,255],
      CLEAR = [0,0,0,0]

describe("Context2D", ()=>{
  let canvas, ctx,
      WIDTH = 512, HEIGHT = 512,
      pixel = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data),
      loadAsset = url => loadImage(`${__dirname}/assets/${url}`),
      mockedWarn = () => {},
      realWarn = console.warn;

  beforeEach(() => {
    canvas = new Canvas(WIDTH, HEIGHT)
    ctx = canvas.getContext("2d")
    console.warn = mockedWarn
  })

  afterEach(() => {
    console.warn = realWarn
  })

  describe("can get & set", ()=>{

    test('currentTransform', () => {
      ctx.scale(0.1, 0.3)
      let matrix = ctx.currentTransform
      _.each({a:0.1, b:0, c:0, d:0.3, e:0, f:0}, (val, term) =>
        expect(matrix[term]).toBeCloseTo(val)
      )

      ctx.resetTransform()
      _.each({a:1, d:1}, (val, term) =>
        expect(ctx.currentTransform[term]).toBeCloseTo(val)
      )

      ctx.currentTransform = matrix
      _.each({a:0.1, d:0.3}, (val, term) =>
        expect(ctx.currentTransform[term]).toBeCloseTo(val)
      )
    })

    test('font', () => {
      expect(ctx.font).toBe('10px sans-serif')
      let font = '16px Baskerville, serif',
          canonical = css.font(font).canonical;
      ctx.font = font
      expect(ctx.font).toBe(canonical)
      ctx.font = 'invalid'
      expect(ctx.font).toBe(canonical)
    })

    test('globalAlpha', () => {
      expect(ctx.globalAlpha).toBe(1)
      ctx.globalAlpha = 0.25
      expect(ctx.globalAlpha).toBeCloseTo(0.25)
      ctx.globalAlpha = -1
      expect(ctx.globalAlpha).toBeCloseTo(0.25)
      ctx.globalAlpha = 3
      expect(ctx.globalAlpha).toBeCloseTo(0.25)
      ctx.globalAlpha = 0
      expect(ctx.globalAlpha).toBe(0)
    })

    test('globalCompositeOperation', () => {
      let ops = ["source-over", "destination-over", "copy", "destination", "clear",
                 "source-in", "destination-in", "source-out", "destination-out",
                 "source-atop", "destination-atop", "xor", "lighter", "multiply",
                 "screen", "overlay", "darken", "lighten", "color-dodge", "color-burn",
                 "hard-light", "soft-light", "difference", "exclusion", "hue",
                 "saturation", "color", "luminosity"]

      expect(ctx.globalCompositeOperation).toBe('source-over')
      ctx.globalCompositeOperation = 'invalid'
      expect(ctx.globalCompositeOperation).toBe('source-over')

      for (let op of ops){
        ctx.globalCompositeOperation = op
        expect(ctx.globalCompositeOperation).toBe(op)
      }
    })

    test('imageSmoothingEnabled', () => {
      expect(ctx.imageSmoothingEnabled).toBe(true)
      ctx.imageSmoothingEnabled = false
      expect(ctx.imageSmoothingEnabled).toBe(false)
    })


    test('imageSmoothingQuality', () => {
      let vals = ["low", "medium", "high"]

      expect(ctx.imageSmoothingQuality).toBe('low')
      ctx.imageSmoothingQuality = 'invalid'
      expect(ctx.imageSmoothingQuality).toBe('low')

      for (let val of vals){
        ctx.imageSmoothingQuality = val
        expect(ctx.imageSmoothingQuality).toBe(val)
      }
    })

    test('lineCap', () => {
      let vals = ["butt", "square", "round"]

      expect(ctx.lineCap).toBe('butt')
      ctx.lineCap = 'invalid'
      expect(ctx.lineCap).toBe('butt')

      for (let val of vals){
        ctx.lineCap = val
        expect(ctx.lineCap).toBe(val)
      }
    })

    test('lineDash', () => {
      expect(ctx.getLineDash()).toEqual([])
      ctx.setLineDash([1,2,3,4])
      expect(ctx.getLineDash()).toEqual([1,2,3,4])
      ctx.setLineDash([NaN])
      expect(ctx.getLineDash()).toEqual([1,2,3,4])
    })

    test('lineJoin', () => {
      let vals = ["miter", "round", "bevel"]

      expect(ctx.lineJoin).toBe('miter')
      ctx.lineJoin = 'invalid'
      expect(ctx.lineJoin).toBe('miter')

      for (let val of vals){
        ctx.lineJoin = val
        expect(ctx.lineJoin).toBe(val)
      }
    })

    test('lineWidth', () => {
      ctx.lineWidth = 10.0;
      expect(ctx.lineWidth).toBe(10)
      ctx.lineWidth = Infinity;
      expect(ctx.lineWidth).toBe(10)
      ctx.lineWidth = -Infinity;
      expect(ctx.lineWidth).toBe(10)
      ctx.lineWidth = -5;
      expect(ctx.lineWidth).toBe(10)
      ctx.lineWidth = 0;
      expect(ctx.lineWidth).toBe(10)
    })

    test('textAlign', () => {
      let vals = ["start", "end", "left", "center", "right", "justify"]

      expect(ctx.textAlign).toBe('start')
      ctx.textAlign = 'invalid'
      expect(ctx.textAlign).toBe('start')

      for (let val of vals){
        ctx.textAlign = val
        expect(ctx.textAlign).toBe(val)
      }
    })

  })

  describe("can create", ()=>{
    test('a context', () => {
      expect(canvas.getContext("invalid")).toBe(null)
      expect(canvas.getContext("2d")).toBe(ctx)
      expect(canvas.pages[0]).toBe(ctx)
      expect(ctx.canvas).toBe(canvas)
    })

    test('multiple pages', () => {
      let ctx2 = canvas.newPage(WIDTH*2, HEIGHT*2);
      expect(canvas.width).toBe(WIDTH*2)
      expect(canvas.height).toBe(HEIGHT*2)
      expect(canvas.pages[0]).toBe(ctx)
      expect(canvas.pages[1]).toBe(ctx2)
      expect(ctx.canvas).toBe(canvas)
      expect(ctx2.canvas).toBe(canvas)
    })

    test("ImageData", () => {
      let [width, height] = [123, 456],
          bmp = ctx.createImageData(width, height);
      expect(bmp.width).toBe(width)
      expect(bmp.height).toBe(height)
      expect(bmp.data.length).toBe(width * height * 4)
      expect(Array.from(bmp.data.slice(0,4))).toEqual(CLEAR)

      let blank = new ImageData(width, height)
      expect(blank.width).toBe(width)
      expect(blank.height).toBe(height)
      expect(blank.data.length).toBe(width * height * 4)
      expect(Array.from(blank.data.slice(0,4))).toEqual(CLEAR)

      new ImageData(blank.data, width, height)
      new ImageData(blank.data, height, width)
      new ImageData(blank.data, width)
      new ImageData(blank.data, height)
      expect(() => new ImageData(blank.data, width+1, height) ).toThrow()
      expect(() => new ImageData(blank.data, width+1) ).toThrow()

      // @ts-ignore
      new ImageData(blank)
      // @ts-ignore
      expect(() => new ImageData(blank.data) ).toThrow()
    })

    describe("CanvasPattern", () => {
      test("from Image", async () => {
        let image = await loadAsset('checkers.png'),
            pattern = ctx.createPattern(image, 'repeat'),
            [width, height] = [20, 20];

        ctx.imageSmoothingEnabled = false
        ctx.fillStyle = pattern;
        ctx.fillRect(0,0,width,height)

        expect.assertions(width * height) // check each pixel

        let bmp = ctx.getImageData(0,0,width,height)
        let blackPixel = true
        for (var i=0; i<bmp.data.length; i+=4){
          if (i % (bmp.width*4) != 0) blackPixel = !blackPixel
          expect(Array.from(bmp.data.slice(i, i+4))).toEqual(
            blackPixel ? BLACK : WHITE
          )
        }
      })

      test("from ImageData", () => {
        let blank = new Canvas()
        ctx.fillStyle = ctx.createPattern(blank, 'repeat');
        ctx.fillRect(0,0, 20,20);

        let checkers = new Canvas(2, 2),
            patCtx = checkers.getContext("2d");
        patCtx.fillStyle = 'white';
        patCtx.fillRect(0,0,2,2);
        patCtx.fillStyle = 'black';
        patCtx.fillRect(0,0,1,1);
        patCtx.fillRect(1,1,1,1);

        let checkersData = patCtx.getImageData(0,0,2,2)

        let pattern = ctx.createPattern(checkersData, 'repeat')
        ctx.fillStyle = pattern;
        ctx.fillRect(0,0, 20,20);

        let bmp = ctx.getImageData(0,0, 20,20)
        let blackPixel = true
        for (var i=0; i<bmp.data.length; i+=4){
          if (i % (bmp.width*4) != 0) blackPixel = !blackPixel
          expect(Array.from(bmp.data.slice(i, i+4))).toEqual(
            blackPixel ? BLACK : WHITE
          )
        }
      })

      test("from Canvas", () => {
        let blank = new Canvas()
        ctx.fillStyle = ctx.createPattern(blank, 'repeat');
        ctx.fillRect(0,0, 20,20);

        let checkers = new Canvas(2, 2),
            patCtx = checkers.getContext("2d");
        patCtx.fillStyle = 'white';
        patCtx.fillRect(0,0,2,2);
        patCtx.fillStyle = 'black';
        patCtx.fillRect(0,0,1,1);
        patCtx.fillRect(1,1,1,1);

        let pattern = ctx.createPattern(checkers, 'repeat')
        ctx.fillStyle = pattern;
        ctx.fillRect(0,0, 20,20);

        let bmp = ctx.getImageData(0,0, 20,20)
        let blackPixel = true
        for (var i=0; i<bmp.data.length; i+=4){
          if (i % (bmp.width*4) != 0) blackPixel = !blackPixel
          expect(Array.from(bmp.data.slice(i, i+4))).toEqual(
            blackPixel ? BLACK : WHITE
          )
        }
      })

      test("with local transform", () => {
        // call func with an ImageData-offset and pixel color value appropriate for a 4-quadrant pattern within
        // the width and height that's white in the upper-left & lower-right and black in the other corners
        function eachPixel(bmp, func){
          let {width, height} = bmp;
          for (let x=0; x<width; x++){
            for (let y=0; y<height; y++){
              let i = y*4*width + x*4,
                  clr = (x<width/2 && y<height/2 || x>=width/2 && y>=height/2) ? 255 : 0;
              func(i, clr);
            }
          }
        }

        // create a canvas with a single repeat of the pattern within its dims
        function makeCheckerboard(w, h){
          let check = new Canvas(w, h),
              ctx = check.getContext('2d'),
              bmp = ctx.createImageData(w, h);
          eachPixel(bmp, (i, clr) => bmp.data.set([clr,clr,clr, 255], i));
          ctx.putImageData(bmp, 0, 0);
          return check;
        }

        // verify that the region looks like a single 4-quadrant checkerboard cell
        function isCheckerboard(ctx, w, h){
          let bmp = ctx.getImageData(0, 0, w, h);
          eachPixel(bmp, (i, clr) => {
            let px = Array.from(bmp.data.slice(i, i+4))
            expect(px).toEqual([clr,clr,clr, 255])
          })
        }

        let w = 160, h = 160,
            pat = ctx.createPattern(makeCheckerboard(w, h), 'repeat'),
            mat = new DOMMatrix();

        ctx.fillStyle = pat;

        // draw a single repeat of the pattern at each scale and then confirm that
        // the transformation succeeded
        [1, .5, .25, .125, 0.0625].forEach(mag => {
          mat = new DOMMatrix().scale(mag);
          pat.setTransform(mat);
          // make sure the alternative matrix syntaxes also work
          expect(() => {pat.setTransform(mag, 0, 0, mag, 0, 0)}).not.toThrow()
          expect(() => {pat.setTransform([mag, 0, 0, mag, 0, 0])}).not.toThrow()
          expect(() => {pat.setTransform({a:mag, b:0, c:0, d:mag, e:0, f:0})}).not.toThrow()
          ctx.fillRect(0,0, w*mag, h*mag);
          isCheckerboard(ctx, w*mag, h*mag);
        })
      })
    })

    describe("CanvasGradient", () => {
      test("linear", () => {
        let gradient = ctx.createLinearGradient(1,1,19,1);
        ctx.fillStyle = gradient;
        gradient.addColorStop(0,'#fff');
        gradient.addColorStop(1,'#000');
        ctx.fillRect(0,0,21,1);

        expect(pixel(0,0)).toEqual(WHITE)
        expect(pixel(20,0)).toEqual(BLACK)
      })

      test("radial", () => {
        let [x, y, inside, outside] = [100, 100, 45, 55],
            inner = [x, y, 25],
            outer = [x, y, 50],
            gradient = ctx.createRadialGradient(...inner, ...outer);
        ctx.fillStyle = gradient
        gradient.addColorStop(0,'#fff');
        gradient.addColorStop(.5,'#000');
        gradient.addColorStop(1,'#000');
        gradient.addColorStop(1,'red');
        ctx.fillRect(0,0, 200,200)

        expect(pixel(x, y)).toEqual(WHITE)
        expect(pixel(x+inside, y)).toEqual(BLACK)
        expect(pixel(x, y+inside)).toEqual(BLACK)
        expect(pixel(x+outside, y)).toEqual([255,0,0,255])
        expect(pixel(x, y+outside)).toEqual([255,0,0,255])
      })

      test("conic", () => {
        // draw a sweep with white at top and black on bottom
        let gradient = ctx.createConicGradient(0, 256, 256);
        ctx.fillStyle = gradient;
        gradient.addColorStop(0,'#fff');
        gradient.addColorStop(.5,'#000');
        gradient.addColorStop(1,'#fff');
        ctx.fillRect(0,0,512,512);

        expect(pixel(5, 256)).toEqual(BLACK)
        expect(pixel(500, 256)).toEqual(WHITE)

        // rotate 90° so black is left and white is right
        gradient = ctx.createConicGradient(Math.PI/2, 256, 256);
        ctx.fillStyle = gradient;
        gradient.addColorStop(0,'#fff');
        gradient.addColorStop(.5,'#000');
        gradient.addColorStop(1,'#fff');
        ctx.fillRect(0,0,512,512);

        expect(pixel(256, 500)).toEqual(WHITE)
        expect(pixel(256, 5)).toEqual(BLACK)
      })
    })

    describe("CanvasTexture", () => {
      var waves, nylon, lines

      beforeEach(() => {
        let w = 40
        let wavePath = new Path2D()
        wavePath.moveTo(-w/2, w/2)
        wavePath.bezierCurveTo(-w*3/8, w*3/4, -w/8, w*3/4, 0, w/2)
        wavePath.bezierCurveTo(w/8, w/4, w*3/8, w/4, w/2, w/2)
        wavePath.bezierCurveTo(w*5/8, w*3/4, w*7/8, w*3/4, w, w/2)
        wavePath.bezierCurveTo(w*9/8, w/4, w*11/8, w/4, w*3/2, w/2)
        waves = ctx.createTexture([w, w/2], {path:wavePath, color:'black', line:3, angle:Math.PI/7})

        let n = 50
        let nylonPath = new Path2D()
        nylonPath.moveTo(0,     n/4)
        nylonPath.lineTo(n/4,   n/4)
        nylonPath.lineTo(n/4,   0)
        nylonPath.moveTo(n*3/4, n)
        nylonPath.lineTo(n*3/4, n*3/4)
        nylonPath.lineTo(n,     n*3/4)
        nylonPath.moveTo(n/4,   n/2)
        nylonPath.lineTo(n/4,   n*3/4)
        nylonPath.lineTo(n/2,   n*3/4)
        nylonPath.moveTo(n/2,   n/4)
        nylonPath.lineTo(n*3/4, n/4)
        nylonPath.lineTo(n*3/4, n/2)
        nylon = ctx.createTexture(n, {path:nylonPath, color:'black', line:5, cap:'square', angle:Math.PI/8})

        lines = ctx.createTexture(8, {line:4, color:'black'})
      })

      test("with filled Path2D", async () => {
        ctx.fillStyle = nylon
        ctx.fillRect(10, 10, 80, 80)

        expect(pixel(26, 24)).toEqual(CLEAR)
        expect(pixel(28, 26)).toEqual(BLACK)
        expect(pixel(48, 48)).toEqual(BLACK)
        expect(pixel(55, 40)).toEqual(CLEAR)
      })

      test("with stroked Path2D", async () => {
        ctx.strokeStyle = waves
        ctx.lineWidth = 10
        ctx.moveTo(0,0)
        ctx.lineTo(100, 100)
        ctx.stroke()

        expect(pixel(10, 10)).toEqual(CLEAR)
        expect(pixel(16, 16)).toEqual(BLACK)
        expect(pixel(73, 73)).toEqual(BLACK)
        expect(pixel(75, 75)).toEqual(CLEAR)
      })

      test("with lines", async () => {
        ctx.fillStyle = lines
        ctx.fillRect(10, 10, 80, 80)

        expect(pixel(22, 22)).toEqual(CLEAR)
        expect(pixel(25, 25)).toEqual(BLACK)
        expect(pixel(73, 73)).toEqual(CLEAR)
        expect(pixel(76, 76)).toEqual(BLACK)
      })
    })
  })

  describe("supports", () => {
    test("filter", () => {
      // results differ b/t cpu & gpu renderers so make sure test doesn't fail if gpu support isn't present
      let {gpu} = canvas
      canvas.gpu = false
      // make sure chains of filters compose correctly <https://codepen.io/sosuke/pen/Pjoqqp>
      ctx.filter = 'blur(5px) invert(56%) sepia(63%) saturate(4837%) hue-rotate(163deg) brightness(96%) contrast(101%)'
      ctx.fillRect(0,0,20,20)
      expect(pixel(10, 10)).toEqual([0, 162, 213, 245])
      canvas.gpu = gpu
    })

    test('shadow', async() => {
      const sin = Math.sin(1.15*Math.PI)
      const cos = Math.cos(1.15*Math.PI)
      ctx.translate(150, 150)
      ctx.transform(cos, sin, -sin, cos, 0, 0)

      ctx.shadowColor = '#000'
      ctx.shadowBlur = 5
      ctx.shadowOffsetX = 10
      ctx.shadowOffsetY = 10
      ctx.fillStyle = '#eee'
      ctx.fillRect(25, 25, 65, 10)

      // ensure that the shadow is actually fuzzy despite the transforms
      expect(pixel(143, 117)).not.toEqual(BLACK)
    })

    test("clip()", () => {
      ctx.fillStyle = 'white'
      ctx.fillRect(0, 0, 2, 2)

      // overlapping rectangles to use as a clipping mask
      ctx.rect(0, 0, 2, 1)
      ctx.rect(1, 0, 1, 2)

      // b | w
      // -----
      // w | b
      ctx.save()
      ctx.clip('evenodd')
      ctx.fillStyle = 'black'
      ctx.fillRect(0, 0, 2, 2)
      ctx.restore()

      expect(pixel(0, 0)).toEqual(BLACK)
      expect(pixel(1, 0)).toEqual(WHITE)
      expect(pixel(0, 1)).toEqual(WHITE)
      expect(pixel(1, 1)).toEqual(BLACK)

      // b | b
      // -----
      // w | b
      ctx.save()
      ctx.clip() // nonzero
      ctx.fillStyle = 'black'
      ctx.fillRect(0, 0, 2, 2)
      ctx.restore()

      expect(pixel(0, 0)).toEqual(BLACK)
      expect(pixel(1, 0)).toEqual(BLACK)
      expect(pixel(0, 1)).toEqual(WHITE)
      expect(pixel(1, 1)).toEqual(BLACK)

      // test intersection of sequential clips while incorporating transform
      ctx.fillStyle = 'black'
      ctx.fillRect(0,0,WIDTH,HEIGHT)

      ctx.save()
      ctx.beginPath()
      ctx.rect(20,20,60,60)
      ctx.clip()
      ctx.fillStyle = 'white'
      ctx.fillRect(0,0,WIDTH,HEIGHT)

      ctx.beginPath()
      ctx.translate(20, 20)
      ctx.rect(0,0,30,30)
      ctx.clip()
      ctx.fillStyle = 'green'
      ctx.fillRect(0,0,WIDTH,HEIGHT)
      ctx.restore()

      expect(pixel(10, 10)).toEqual(BLACK)
      expect(pixel(90, 90)).toEqual(BLACK)
      expect(pixel(22, 22)).toEqual(GREEN)
      expect(pixel(48, 48)).toEqual(GREEN)
      expect(pixel(52, 52)).toEqual(WHITE)

      // non-overlapping clips & empty clips should prevent drawing altogether
      ctx.beginPath()
      ctx.rect(20,20,30,30)
      ctx.clip()
      ctx.fillStyle = 'black'
      ctx.fillRect(0,0,WIDTH,HEIGHT)

      ctx.save()
      ctx.beginPath()
      ctx.rect(25,25,0,0)
      ctx.clip()
      ctx.fillStyle = 'green'
      ctx.fillRect(0,0,WIDTH,HEIGHT)
      ctx.restore()

      ctx.save()
      ctx.beginPath()
      ctx.rect(0,0,10,10)
      ctx.clip()
      ctx.fillStyle = 'green'
      ctx.fillRect(0,0,WIDTH,HEIGHT)
      ctx.restore()

      expect(pixel(30, 30)).toEqual(BLACK)
    })

    test("fill()", () => {
      ctx.fillStyle = 'white'
      ctx.fillRect(0, 0, 2, 2)

      // set the current path to a pair of overlapping rects
      ctx.fillStyle = 'black'
      ctx.rect(0, 0, 2, 1)
      ctx.rect(1, 0, 1, 2)

      // b | w
      // -----
      // w | b
      ctx.fill('evenodd')
      expect(pixel(0, 0)).toEqual(BLACK)
      expect(pixel(1, 0)).toEqual(WHITE)
      expect(pixel(0, 1)).toEqual(WHITE)
      expect(pixel(1, 1)).toEqual(BLACK)

      // b | b
      // -----
      // w | b
      ctx.fill() // nonzero
      expect(pixel(0, 0)).toEqual(BLACK)
      expect(pixel(1, 0)).toEqual(BLACK)
      expect(pixel(0, 1)).toEqual(WHITE)
      expect(pixel(1, 1)).toEqual(BLACK)
    })

    test("fillText()", () => {
      /** @type {[args: any[], shouldDraw: boolean][]} */
      let argsets = [
        [['A', 10, 10], true],
        [['A', 10, 10, undefined], true],
        [['A', 10, 10, NaN], false],
        [['A', 10, 10, Infinity], false],
        [[1234, 10, 10], true],
        [[false, 10, 10], true],
        [[{}, 10, 10], true],
      ]

      _.each(argsets, ([args, shouldDraw]) => {
        canvas.width = WIDTH
        ctx.textBaseline = 'middle'
        ctx.textAlign = 'center'
        ctx.fillText(...args)
        expect(ctx.getImageData(0, 0, 20, 20).data.some(a => a)).toBe(shouldDraw)
      })
    })

    test("roundRect()", () => {
      let dim = WIDTH/2
      let radii = [50, 25, {x:15, y:15}, new DOMPoint(20, 10)]
      ctx.beginPath()
      ctx.roundRect(dim, dim, dim, dim, radii)
      ctx.roundRect(dim, dim, -dim, -dim, radii)
      ctx.roundRect(dim, dim, -dim, dim, radii)
      ctx.roundRect(dim, dim, dim, -dim, radii)
      ctx.fill()

      let off = [ [3,3], [dim-14, dim-14], [dim-4, 3], [7, dim-6]]
      let on = [ [5,5], [dim-17, dim-17], [dim-9, 3], [9, dim-9] ]

      for (const [x, y] of on){
        expect(pixel(x, y)).toEqual(BLACK)
        expect(pixel(x, HEIGHT - y - 1)).toEqual(BLACK)
        expect(pixel(WIDTH - x - 1, y)).toEqual(BLACK)
        expect(pixel(WIDTH - x - 1, HEIGHT - y - 1)).toEqual(BLACK)
      }

      for (const [x, y] of off){
        expect(pixel(x, y)).toEqual(CLEAR)
        expect(pixel(x, HEIGHT - y - 1)).toEqual(CLEAR)
        expect(pixel(WIDTH - x - 1, y)).toEqual(CLEAR)
        expect(pixel(WIDTH - x - 1, HEIGHT - y - 1)).toEqual(CLEAR)
      }
    })

    test('getImageData()', () => {
      ctx.fillStyle = 'rgba(255,0,0, 0.25)'
      ctx.fillRect(0,0,1,6)

      ctx.fillStyle = 'rgba(0,255,0, 0.5)'
      ctx.fillRect(1,0,1,6)

      ctx.fillStyle = 'rgba(0,0,255, 0.75)'
      ctx.fillRect(2,0,1,6)

      let [width, height] = [3, 6],
          bmp1 = ctx.getImageData(0,0, width,height),
          bmp2 = ctx.getImageData(width,height,-width,-height) // negative dimensions shift origin
      for (const bmp of [bmp1, bmp2]){
        expect(bmp.width).toBe(width)
        expect(bmp.height).toBe(height)
        expect(bmp.data.length).toBe(width * height * 4)
        expect(Array.from(bmp.data.slice(0,4))).toEqual([255,0,0,64])
        expect(Array.from(bmp.data.slice(4,8))).toEqual([0,255,0,128])
        expect(Array.from(bmp.data.slice(8,12))).toEqual([0,0,255,191])
        for(var x=0; x<width; x++){
          for(var y=0; y<height; y++){
            let i = 4 * (y*width + x)
            let px = Array.from(bmp.data.slice(i,i+4))
            expect(pixel(x,y)).toEqual(px)
          }
        }
      }
    })

    test('putImageData()', () => {
      expect(() => ctx.putImageData({}, 0, 0)).toThrow()
      expect(() => ctx.putImageData(undefined, 0, 0)).toThrow()

      var srcImageData = ctx.createImageData(2,2)
      srcImageData.data.set([
        1,2,3,255, 5,6,7,255,
        0,1,2,255, 4,5,6,255
      ], 0)

      ctx.putImageData(srcImageData, -1, -1);
      var resImageData = ctx.getImageData(0, 0, 2, 2);
      expect(Array.from(resImageData.data)).toEqual([
        4,5,6,255, 0,0,0,0,
        0,0,0,0,   0,0,0,0
      ])

      // try mask rect
      ctx.reset()
      ctx.putImageData(srcImageData, 0, 0, 1, 1, 1, 1);
      resImageData = ctx.getImageData(0, 0, 2, 2);
      expect(Array.from(resImageData.data)).toEqual([
        0,0,0,0, 0,0,0,0,
        0,0,0,0, 4,5,6,255
      ])

      // try negative dimensions
      ctx.reset()
      ctx.putImageData(srcImageData, 0, 0, 1, 1, -1, -1);
      resImageData = ctx.getImageData(0, 0, 2, 2);
      expect(Array.from(resImageData.data)).toEqual([
        1,2,3,255, 0,0,0,0,
        0,0,0,0,   0,0,0,0
      ])
    })

    test("isPointInPath()", () => {
      let inStroke = [100, 94],
          inFill = [150, 150],
          inBoth = [100, 100];

      ctx.rect(100,100,100,100)
      ctx.lineWidth = 12

      expect(ctx.isPointInPath(...inStroke)).toBe(false)
      expect(ctx.isPointInStroke(...inStroke)).toBe(true)

      expect(ctx.isPointInPath(...inFill)).toBe(true)
      expect(ctx.isPointInStroke(...inFill)).toBe(false)

      expect(ctx.isPointInPath(...inBoth)).toBe(true)
      expect(ctx.isPointInStroke(...inBoth)).toBe(true)
    })

    test("isPointInPath(Path2D)", () => {
      let inStroke = [100, 94],
          inFill = [150, 150],
          inBoth = [100, 100];

      let path = new Path2D()
      path.rect(100,100,100,100)
      ctx.lineWidth = 12

      expect(ctx.isPointInPath(path, ...inStroke)).toBe(false)
      expect(ctx.isPointInStroke(path, ...inStroke)).toBe(true)

      expect(ctx.isPointInPath(path, ...inFill)).toBe(true)
      expect(ctx.isPointInStroke(path, ...inFill)).toBe(false)

      expect(ctx.isPointInPath(path, ...inBoth)).toBe(true)
      expect(ctx.isPointInStroke(path, ...inBoth)).toBe(true)
    })

    test("letterSpacing", () => {
        FontLibrary.use(`${__dirname}/assets/Monoton-Regular.woff`)

        let [x, y] = [40, 100]
        let size = 32
        let text = "RR"
        ctx.font = `${size}px Monoton`
        ctx.letterSpacing = '20px'
        ctx.fillStyle = 'black'
        ctx.fillText(text, x, y)

        // there should be no initial added space indenting the beginning of the line
        expect(ctx.getImageData(x, y-size, 10, size).data.some(a => a)).toBe(true)

        // there should be whitespace between the first and second characters
        expect(ctx.getImageData(x+28, y-size, 18, size).data.some(a => a)).toBe(false)

        // check whether upstream has fixed the indent bug and our compensation is now outdenting
        expect(ctx.getImageData(x-20, y-size, 18, size).data.some(a => a)).toBe(false)

        // make sure the extra space skia adds to the beginning/end have been subtracted
        expect(ctx.measureText(text).width).toBeCloseTo(74)
        ctx.textWrap = true
        expect(ctx.measureText(text).width).toBeCloseTo(74)
    })

    test("measureText()", () => {
      ctx.font = "20px Arial, DejaVu Sans"

      let ø = ctx.measureText('').width,
          _ = ctx.measureText(' ').width,
          __ = ctx.measureText('  ').width,
          foo = ctx.measureText('foo').width,
          foobar = ctx.measureText('foobar').width,
          __foo = ctx.measureText('  foo').width,
          __foo__ = ctx.measureText('  foo  ').width
      expect(ø).toBeLessThan(_)
      expect(_).toBeLessThan(__)
      expect(foo).toBeLessThan(foobar)
      expect(__foo).toBeGreaterThan(foo)
      expect(__foo__).toBeGreaterThan(__foo)

      // start from the default, alphabetic baseline
      let msg = "Lordran gypsum",
          metrics = ctx.measureText(msg)

      // + means up, - means down when it comes to baselines
      expect(metrics.alphabeticBaseline).toBe(0)
      expect(metrics.hangingBaseline).toBeGreaterThan(0)
      expect(metrics.ideographicBaseline).toBeLessThan(0)

      // for ascenders + means up, for descenders + means down
      expect(metrics.actualBoundingBoxAscent).toBeGreaterThan(0)
      expect(metrics.actualBoundingBoxDescent).toBeGreaterThan(0)
      expect(metrics.actualBoundingBoxAscent).toBeGreaterThan(metrics.actualBoundingBoxDescent)

      // make sure the polarity has flipped for 'top' baseline
      ctx.textBaseline = "top"
      metrics = ctx.measureText("Lordran gypsum")
      expect(metrics.alphabeticBaseline).toBeLessThan(0)
      expect(metrics.hangingBaseline).toBeLessThan(0)
      expect(metrics.actualBoundingBoxAscent).toBeLessThan(0)
      expect(metrics.actualBoundingBoxDescent).toBeGreaterThan(0)

      // width calculations should be the same (modulo rounding) for any alignment
      let [lft, cnt, rgt] = ['left', 'center', 'right'].map(align => {
        ctx.textAlign = align
        return ctx.measureText(msg).width
      })
      expect(lft).toBeCloseTo(cnt)
      expect(cnt).toBeCloseTo(rgt)

      // make sure string indices account for trailing whitespace and non-8-bit characters
      let text = ' 石 ',
          {startIndex, endIndex} = ctx.measureText(text).lines[0]
      expect(text.substring(startIndex, endIndex)).toBe(text)
    })


    test("createProjection()", () => {
      let quad = [
        WIDTH*.33, HEIGHT/2,
        WIDTH*.66, HEIGHT/2,
        WIDTH, HEIGHT*.9,
        0, HEIGHT*.9,
      ]

      let matrix = ctx.createProjection(quad)
      ctx.setTransform(matrix)

      ctx.fillStyle = 'black'
      ctx.fillRect(0,0, WIDTH/4, HEIGHT)
      ctx.fillStyle = 'white'
      ctx.fillRect(WIDTH/4, 0, WIDTH/4, HEIGHT)
      ctx.fillStyle = 'green'
      ctx.fillRect(WIDTH/2, 0, WIDTH/4, HEIGHT)
      ctx.resetTransform()

      let x = WIDTH/2, y = HEIGHT/2 + 2
      expect(pixel(x, y - 5)).toEqual(CLEAR)
      expect(pixel(x + 25, y)).toEqual(GREEN)
      expect(pixel(x + 75, y)).toEqual(CLEAR)
      expect(pixel(x - 25, y)).toEqual(WHITE)
      expect(pixel(x - 75, y)).toEqual(BLACK)
      expect(pixel(x - 100, y)).toEqual(CLEAR)

      y = HEIGHT*.9 - 2
      expect(pixel(x + 100, y)).toEqual(GREEN)
      expect(pixel(x + 130, y)).toEqual(CLEAR)
      expect(pixel(x - 75, y)).toEqual(WHITE)
      expect(pixel(x - 200, y)).toEqual(BLACK)
      expect(pixel(0, y)).toEqual(CLEAR)
    })

    test('drawImage()', async () => {
      let image = await loadAsset('checkers.png')
      ctx.imageSmoothingEnabled = false

      ctx.drawImage(image, 0,0)
      expect(pixel(0, 0)).toEqual(BLACK)
      expect(pixel(1, 0)).toEqual(WHITE)
      expect(pixel(0, 1)).toEqual(WHITE)
      expect(pixel(1, 1)).toEqual(BLACK)

      ctx.drawImage(image,-256,-256,512,512)
      expect(pixel(0, 0)).toEqual(BLACK)
      expect(pixel(149, 149)).toEqual(BLACK)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.save()
      ctx.translate(WIDTH/2, HEIGHT/2)
      ctx.rotate(.25*Math.PI)
      ctx.drawImage(image,-256,-256,512,512)
      ctx.restore()
      expect(pixel(0, 0)).toEqual(CLEAR)
      expect(pixel(WIDTH/2, HEIGHT*.25)).toEqual(BLACK)
      expect(pixel(WIDTH/2, HEIGHT*.75)).toEqual(BLACK)
      expect(pixel(WIDTH*.25, HEIGHT/2)).toEqual(WHITE)
      expect(pixel(WIDTH*.75, HEIGHT/2)).toEqual(WHITE)
      expect(pixel(WIDTH-1, HEIGHT-1)).toEqual(CLEAR)

      let srcCanvas = new Canvas(3, 3),
          srcCtx = srcCanvas.getContext("2d");
      srcCtx.fillStyle = 'green'
      srcCtx.fillRect(0,0,3,3)
      srcCtx.clearRect(1,1,1,1)

      ctx.drawImage(srcCanvas, 0,0)
      expect(pixel(0, 0)).toEqual(GREEN)
      expect(pixel(1, 1)).toEqual(CLEAR)
      expect(pixel(2, 2)).toEqual(GREEN)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.drawImage(srcCanvas,-2,-2,6,6)
      expect(pixel(0, 0)).toEqual(CLEAR)
      expect(pixel(2, 0)).toEqual(GREEN)
      expect(pixel(2, 2)).toEqual(GREEN)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.save()
      ctx.translate(WIDTH/2, HEIGHT/2)
      ctx.rotate(.25*Math.PI)
      ctx.drawImage(srcCanvas,-256,-256,512,512)
      ctx.restore()
      expect(pixel(WIDTH/2, HEIGHT*.25)).toEqual(GREEN)
      expect(pixel(WIDTH/2, HEIGHT*.75)).toEqual(GREEN)
      expect(pixel(WIDTH*.25, HEIGHT/2)).toEqual(GREEN)
      expect(pixel(WIDTH*.75, HEIGHT/2)).toEqual(GREEN)
      expect(pixel(WIDTH/2, HEIGHT/2)).toEqual(CLEAR)
    })

    test('drawCanvas()', async () => {
      let srcCanvas = new Canvas(3, 3),
          srcCtx = srcCanvas.getContext("2d");
      srcCtx.fillStyle = 'green'
      srcCtx.fillRect(0,0,3,3)
      srcCtx.clearRect(1,1,1,1)

      ctx.drawCanvas(srcCanvas, 0,0)
      expect(pixel(0, 0)).toEqual(GREEN)
      expect(pixel(1, 1)).toEqual(CLEAR)
      expect(pixel(2, 2)).toEqual(GREEN)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.drawCanvas(srcCanvas,-2,-2,6,6)
      expect(pixel(0, 0)).toEqual(CLEAR)
      expect(pixel(2, 0)).toEqual(GREEN)
      expect(pixel(2, 2)).toEqual(GREEN)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.save()
      ctx.translate(WIDTH/2, HEIGHT/2)
      ctx.rotate(.25*Math.PI)
      ctx.drawCanvas(srcCanvas,-256,-256,512,512)
      ctx.restore()
      expect(pixel(WIDTH/2, HEIGHT*.25)).toEqual(GREEN)
      expect(pixel(WIDTH/2, HEIGHT*.75)).toEqual(GREEN)
      expect(pixel(WIDTH*.25, HEIGHT/2)).toEqual(GREEN)
      expect(pixel(WIDTH*.75, HEIGHT/2)).toEqual(GREEN)
      expect(pixel(WIDTH/2, HEIGHT/2)).toEqual(CLEAR)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.drawCanvas(srcCanvas, 1,1,2,2, 0,0,2,2)
      expect(pixel(0, 0)).toEqual(CLEAR)
      expect(pixel(0, 1)).toEqual(GREEN)
      expect(pixel(1, 0)).toEqual(GREEN)
      expect(pixel(1, 1)).toEqual(GREEN)

      let image = await loadAsset('checkers.png')
      expect( () => ctx.drawCanvas(image, 0, 0) ).not.toThrow()
    })

    test('reset()', async () => {
      ctx.fillStyle = 'green'
      ctx.scale(2, 2)
      ctx.translate(0, -HEIGHT/4)

      ctx.fillRect(WIDTH/4, HEIGHT/4, WIDTH/8, HEIGHT/8)
      expect(pixel(WIDTH * .5 + 1, 0)).toEqual(GREEN)
      expect(pixel(WIDTH * .75 - 1, 0)).toEqual(GREEN)

      ctx.beginPath()
      ctx.rect(WIDTH/4, HEIGHT/2, 100, 100)
      ctx.reset()
      ctx.fill()
      expect(pixel(WIDTH/2 + 1, HEIGHT/2 + 1)).toEqual(CLEAR)
      expect(pixel(WIDTH * .5 + 1, 0)).toEqual(CLEAR)
      expect(pixel(WIDTH * .75 - 1, 0)).toEqual(CLEAR)

      ctx.globalAlpha = 0.4
      ctx.reset()
      ctx.fillRect(WIDTH/2, HEIGHT/2, 3, 3)
      expect(pixel(WIDTH/2 + 1, HEIGHT/2 + 1)).toEqual(BLACK)
    })

    describe("transform()", ()=>{
      const a=0.1, b=0, c=0, d=0.3, e=0, f=0

      test('with args list', () => {
        ctx.transform(a, b, c, d, e, f)
        let matrix = ctx.currentTransform
        _.each({a, b, c, d, e, f}, (val, term) =>
          expect(matrix[term]).toBeCloseTo(val)
        )
      })

      test('with DOMMatrix', () => {
        ctx.transform(new DOMMatrix().scale(0.1, 0.3));
        let matrix = ctx.currentTransform
        _.each({a, b, c, d, e, f}, (val, term) =>
          expect(matrix[term]).toBeCloseTo(val)
        )
      })

      test('with matrix-like object', () => {
        ctx.transform({a, b, c, d, e, f});
        let matrix = ctx.currentTransform
        _.each({a, b, c, d, e, f}, (val, term) =>
          expect(matrix[term]).toBeCloseTo(val)
        )
      })

      test('with css-style string', () => {
        // try a range of string inits
        const transforms = {
          "matrix(1, 2, 3, 4, 5, 6)": "matrix(1, 2, 3, 4, 5, 6)",
          "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1)": "matrix(1, 0, 0, 1, 0, 0)",
          "rotate(0.5turn)": "matrix(-1, 0, 0, -1, 0, 0)",
          "rotate3d(1, 2, 3, 10deg)": "matrix3d(0.985892913511, 0.141398603856, -0.089563373741, 0, -0.137057961859, 0.989148395009, 0.052920390614, 0, 0.096074336736, -0.039898464624, 0.994574197504, 0, 0, 0, 0, 1)",
          "rotateX(10deg)": "matrix3d(1, 0, 0, 0, 0, 0.984807753012, 0.173648177667, 0, 0, -0.173648177667, 0.984807753012, 0, 0, 0, 0, 1)",
          "rotateY(10deg)": "matrix3d(0.984807753012, 0, -0.173648177667, 0, 0, 1, 0, 0, 0.173648177667, 0, 0.984807753012, 0, 0, 0, 0, 1)",
          "rotateZ(10deg)": "matrix(0.984807753012, 0.173648177667, -0.173648177667, 0.984807753012, 0, 0)",
          "translate(12px, 50px)": "matrix(1, 0, 0, 1, 12, 50)",
          "translate3d(12px, 50px, 3px)": "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 12, 50, 3, 1)",
          "translateX(2px)": "matrix(1, 0, 0, 1, 2, 0)",
          "translateY(3px)": "matrix(1, 0, 0, 1, 0, 3)",
          "translateZ(2px)": "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 2, 1)",
          "scale(2, 0.5)": "matrix(2, 0, 0, 0.5, 0, 0)",
          "scale3d(2.5, 120%, 0.3)": "matrix3d(2.5, 0, 0, 0, 0, 1.2, 0, 0, 0, 0, 0.3, 0, 0, 0, 0, 1)",
          "scaleX(2)": "matrix(2, 0, 0, 1, 0, 0)",
          "scaleY(0.5)": "matrix(1, 0, 0, 0.5, 0, 0)",
          "scaleZ(0.3)": "matrix3d(1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0.3, 0, 0, 0, 0, 1)",
          "skew(30deg, 20deg)": "matrix(1, 0.363970234266, 0.577350269190, 1, 0, 0)",
          "skewX(30deg)": "matrix(1, 0, 0.577350269190, 1, 0, 0)",
          "skewY(1.07rad)": "matrix(1, 1.827028196535, 0, 1, 0, 0)",
          "translate(10px, 20px) matrix(1, 2, 3, 4, 5, 6)": "matrix(1, 2, 3, 4, 15, 26)",
          "translate(5px, 6px) scale(2) translate(7px,8px)": "matrix(2, 0, 0, 2, 19, 22)",
          "rotate(30deg) rotate(-.1turn) rotate(.444rad)": "matrix(0.942994450354, 0.332808453321, -0.332808453321, 0.942994450354, 0, 0)",
          "none": "matrix(1, 0, 0, 1, 0, 0)",
          "unset": "matrix(1, 0, 0, 1, 0, 0)",
        }

        for (const input in transforms){
          let matrix = new DOMMatrix(input),
              roundtrip = new DOMMatrix(matrix.toString())
          expect(matrix.toString()).toEqual(transforms[input])
          expect(roundtrip.toString()).toEqual(transforms[input])
        }

        // check that the context can also take a string
        ctx.transform(`scale(${a}, ${d})`);
        let matrix = ctx.currentTransform
        _.each({a, b, c, d, e, f}, (val, term) =>
          expect(matrix[term]).toBeCloseTo(val)
        )
      })

      test('rejects invalid args', () => {
        expect( () => ctx.transform("nonesuch")).toThrow("Invalid transform matrix")
        expect( () => ctx.transform(0, 0, 0)).toThrow("not enough arguments")
        expect( () => ctx.transform(0, 0, 0, NaN, 0, 0)).not.toThrow()
      })

    })

  })

  describe("parses", () => {
    test('fonts', () => {
      let cases = {
        '20px Arial': { size: 20, family: ['Arial'] },
        '33pt Arial': { size: 44, family: ['Arial'] },
        '75pt Arial': { size: 100, family: ['Arial'] },
        '20% Arial': { size: 16 * 0.2, family:['Arial'] },
        '20mm Arial': { size: 75.59055118110237, family: ['Arial'] },
        '20px serif': { size: 20, family: ['serif'] },
        '20px sans-serif': { size: 20, family: ['sans-serif'] },
        '20px monospace': { size: 20, family: ['monospace'] },
        '50px Arial, sans-serif': { size: 50, family: ['Arial','sans-serif'] },
        'bold italic 50px Arial, sans-serif': { style: 'italic', weight: 700, size: 50, family: ['Arial','sans-serif'] },
        '50px Helvetica ,  Arial, sans-serif': { size: 50, family: ['Helvetica','Arial','sans-serif'] },
        '50px "Helvetica Neue", sans-serif': { size: 50, family: ['Helvetica Neue','sans-serif'] },
        '50px "Helvetica Neue", "foo bar baz" , sans-serif': { size: 50, family: ['Helvetica Neue','foo bar baz','sans-serif'] },
        "50px 'Helvetica Neue'": { size: 50, family: ['Helvetica Neue'] },
        'italic 20px Arial': { size: 20, style: 'italic', family: ['Arial'] },
        'oblique 20px Arial': { size: 20, style: 'oblique', family: ['Arial'] },
        'normal 20px Arial': { size: 20, style: 'normal', family: ['Arial'] },
        '300 20px Arial': { size: 20, weight: 300, family: ['Arial'] },
        '800 20px Arial': { size: 20, weight: 800, family: ['Arial'] },
        'bolder 20px Arial': { size: 20, weight: 800, family: ['Arial'] },
        'lighter 20px Arial': { size: 20, weight: 300, family: ['Arial'] },
        'normal normal normal 16px Impact': { size: 16, weight: 400, family: ['Impact'], style: 'normal', variant: 'normal' },
        'italic small-caps bolder 16px cursive': { size: 16, style: 'italic', variant: 'small-caps', weight: 800, family: ['cursive'] },
        '20px "new century schoolbook", serif': { size: 20, family: ['new century schoolbook','serif'] },
        '20px "Arial bold 300"': { size: 20, family: ['Arial bold 300'], variant: 'normal' }, // synthetic case with weight keyword inside family
      }

      _.each(cases, (spec, font) => {
        let expected = _.defaults(spec, {style:"normal", stretch:"normal", variant:"normal"}),
            parsed = css.font(font);
        expect(parsed).toMatchObject(expected)
      })

    })

    test('colors', () => {
      ctx.fillStyle = '#ffccaa';
      expect(ctx.fillStyle).toBe('#ffccaa');

      ctx.fillStyle = '#FFCCAA';
      expect(ctx.fillStyle).toBe('#ffccaa');

      ctx.fillStyle = '#FCA';
      expect(ctx.fillStyle).toBe('#ffccaa');

      ctx.fillStyle = '#0ff';
      ctx.fillStyle = '#FGG';
      expect(ctx.fillStyle).toBe('#00ffff');

      ctx.fillStyle = '#fff';
      ctx.fillStyle = 'afasdfasdf';
      expect(ctx.fillStyle).toBe('#ffffff');

      // #rgba and #rrggbbaa

      ctx.fillStyle = '#ffccaa80'
      expect(ctx.fillStyle).toBe('rgba(255, 204, 170, 0.502)')

      ctx.fillStyle = '#acf8'
      expect(ctx.fillStyle).toBe('rgba(170, 204, 255, 0.533)')

      ctx.fillStyle = '#BEAD'
      expect(ctx.fillStyle).toBe('rgba(187, 238, 170, 0.867)')

      ctx.fillStyle = 'rgb(255,255,255)';
      expect(ctx.fillStyle).toBe('#ffffff');

      ctx.fillStyle = 'rgb(0,0,0)';
      expect(ctx.fillStyle).toBe('#000000');

      ctx.fillStyle = 'rgb( 0  ,   0  ,  0)';
      expect(ctx.fillStyle).toBe('#000000');

      ctx.fillStyle = 'rgba( 0  ,   0  ,  0, 1)';
      expect(ctx.fillStyle).toBe('#000000');

      ctx.fillStyle = 'rgba( 255, 200, 90, 0.5)';
      expect(ctx.fillStyle).toBe('rgba(255, 200, 90, 0.502)');

      ctx.fillStyle = 'rgba( 255, 200, 90, 0.75)';
      expect(ctx.fillStyle).toBe('rgba(255, 200, 90, 0.749)');

      ctx.fillStyle = 'rgba( 255, 200, 90, 0.7555)';
      expect(ctx.fillStyle).toBe('rgba(255, 200, 90, 0.757)');

      ctx.fillStyle = 'rgba( 255, 200, 90, .7555)';
      expect(ctx.fillStyle).toBe('rgba(255, 200, 90, 0.757)');

      ctx.fillStyle = 'rgb(0, 0, 9000)';
      expect(ctx.fillStyle).toBe('#0000ff');

      ctx.fillStyle = 'rgba(0, 0, 0, 42.42)';
      expect(ctx.fillStyle).toBe('#000000');

      // hsl / hsla tests

      ctx.fillStyle = 'hsl(0, 0%, 0%)';
      expect(ctx.fillStyle).toBe('#000000');

      ctx.fillStyle = 'hsl(3600, -10%, -10%)';
      expect(ctx.fillStyle).toBe('#000000');

      ctx.fillStyle = 'hsl(10, 100%, 42%)';
      expect(ctx.fillStyle).toBe('#d62400');

      ctx.fillStyle = 'hsl(370, 120%, 42%)';
      expect(ctx.fillStyle).toBe('#d62400');

      ctx.fillStyle = 'hsl(0, 100%, 100%)';
      expect(ctx.fillStyle).toBe('#ffffff');

      ctx.fillStyle = 'hsl(0, 150%, 150%)';
      expect(ctx.fillStyle).toBe('#ffffff');

      ctx.fillStyle = 'hsl(237, 76%, 25%)';
      expect(ctx.fillStyle).toBe('#0f1470');

      ctx.fillStyle = 'hsl(240, 73%, 25%)';
      expect(ctx.fillStyle).toBe('#11116e');

      ctx.fillStyle = 'hsl(262, 32%, 42%)';
      expect(ctx.fillStyle).toBe('#62498d');

      ctx.fillStyle = 'hsla(0, 0%, 0%, 1)';
      expect(ctx.fillStyle).toBe('#000000');

      ctx.fillStyle = 'hsla(0, 100%, 100%, 1)';
      expect(ctx.fillStyle).toBe('#ffffff');

      ctx.fillStyle = 'hsla(120, 25%, 75%, 0.5)';
      expect(ctx.fillStyle).toBe('rgba(175, 207, 175, 0.502)');

      ctx.fillStyle = 'hsla(240, 75%, 25%, 0.75)';
      expect(ctx.fillStyle).toBe('rgba(16, 16, 112, 0.749)');

      ctx.fillStyle = 'hsla(172.0, 33.00000e0%, 42%, 1)';
      expect(ctx.fillStyle).toBe('#488e85');

      ctx.fillStyle = 'hsl(124.5, 76.1%, 47.6%)';
      expect(ctx.fillStyle).toBe('#1dd62b');

      ctx.fillStyle = 'hsl(1.24e2, 760e-1%, 4.7e1%)';
      expect(ctx.fillStyle).toBe('#1dd329');

      // case-insensitive css names

      ctx.fillStyle = "sILveR";
      expect(ctx.fillStyle).toBe("#c0c0c0");

      // wrong type args

      let transparent = 'rgba(0, 0, 0, 0)'
      ctx.fillStyle = 'transparent'
      expect(ctx.fillStyle).toBe(transparent);

      ctx.fillStyle = null
      expect(ctx.fillStyle).toBe(transparent);

      ctx.fillStyle = NaN
      expect(ctx.fillStyle).toBe(transparent);

      ctx.fillStyle = [undefined, 255, false]
      expect(ctx.fillStyle).toBe(transparent);

      ctx.fillStyle = true
      expect(ctx.fillStyle).toBe(transparent);

      ctx.fillStyle = {}
      expect(ctx.fillStyle).toBe(transparent);

      // objects with .toString methods

      ctx.fillStyle = {toString:() => 'red'}
      expect(ctx.fillStyle).toBe('#ff0000');

      ctx.fillStyle = 'transparent'
      ctx.fillStyle = {toString:'red'}
      expect(ctx.fillStyle).toBe(transparent);

      ctx.fillStyle = {toString:() => 'gobbledygook'}
      expect(ctx.fillStyle).toBe(transparent);

      ctx.fillStyle = {toString:() => NaN}
      expect(ctx.fillStyle).toBe(transparent);

    });
  })

  describe("validates", () => {
    let g, id, img, p2d
    beforeEach(async () => {
      g = ctx.createLinearGradient(0,0,10,10)
      id = ctx.getImageData(0,0,10,10)
      img = await loadAsset("checkers.png")
      p2d = new Path2D()
      p2d.rect(0,0,100,100)
      ctx.rect(0,0,100,100)
    })

    test('not enough arguments', async () => {
      let ERR = "not enough arguments"
      expect(() => ctx.transform()).toThrow(ERR)
      expect(() => ctx.transform(0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.setTransform(0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.translate(0)).toThrow(ERR)
      expect(() => ctx.scale(0)).toThrow(ERR)
      expect(() => ctx.rotate()).toThrow(ERR)
      expect(() => ctx.rect(0,0,0)).toThrow(ERR)
      expect(() => ctx.arc(0,0,0,0)).toThrow(ERR)
      expect(() => ctx.arcTo(0,0,0,0)).toThrow(ERR)
      expect(() => ctx.ellipse(0,0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.moveTo(0)).toThrow(ERR)
      expect(() => ctx.lineTo(0)).toThrow(ERR)
      expect(() => ctx.bezierCurveTo(0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.quadraticCurveTo(0,0,0)).toThrow(ERR)
      expect(() => ctx.conicCurveTo(0,0,0,0)).toThrow(ERR)
      expect(() => ctx.roundRect(0,0,0)).toThrow(ERR)
      expect(() => ctx.fillRect(0,0,0)).toThrow(ERR)
      expect(() => ctx.strokeRect(0,0,0)).toThrow(ERR)
      expect(() => ctx.clearRect(0,0,0)).toThrow(ERR)
      expect(() => ctx.fillText("text",0)).toThrow(ERR)
      expect(() => ctx.isPointInPath(10)).toThrow(ERR)
      expect(() => ctx.isPointInStroke(10)).toThrow(ERR)
      expect(() => ctx.createLinearGradient(0,0,1)).toThrow(ERR)
      expect(() => ctx.createRadialGradient(0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.createConicGradient(0,0)).toThrow(ERR)
      expect(() => ctx.setLineDash()).toThrow(ERR)
      expect(() => ctx.createImageData()).toThrow(ERR)
      expect(() => ctx.createPattern(img)).toThrow(ERR)
      expect(() => ctx.createTexture()).toThrow(ERR)
      expect(() => ctx.getImageData(1,1,10)).toThrow(ERR)
      expect(() => ctx.putImageData({},0)).toThrow(ERR)
      expect(() => ctx.putImageData(id,0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.drawImage(img)).toThrow(ERR)
      expect(() => ctx.drawImage(img,0)).toThrow(ERR)
      expect(() => ctx.drawImage(img,0,0,0)).toThrow(ERR)
      expect(() => ctx.drawImage(img,0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.drawImage(img,0,0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.drawImage(img,0,0,0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.drawCanvas(canvas)).toThrow(ERR)
      expect(() => ctx.drawCanvas(canvas,0)).toThrow(ERR)
      expect(() => ctx.drawCanvas(canvas,0,0,0)).toThrow(ERR)
      expect(() => ctx.drawCanvas(canvas,0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.drawCanvas(canvas,0,0,0,0,0,0)).toThrow(ERR)
      expect(() => ctx.drawCanvas(canvas,0,0,0,0,0,0,0)).toThrow(ERR)
      expect(() => g.addColorStop(0)).toThrow(ERR)
    })

    test('value errors', async () => {
      expect(() => ctx.ellipse(0,0,-10,-10,0,0,0,false)).toThrow("Radius value must be positive")
      expect(() => ctx.arcTo(0,0,0,0,-10)).toThrow("Radius value must be positive")
      expect(() => ctx.roundRect(0,0,0,0,-10)).toThrow("Corner radius cannot be negative")
      expect(() => ctx.createImageData(1,0)).toThrow("Dimensions must be non-zero")
      expect(() => ctx.getImageData(1,1,NaN,10)).toThrow("Expected a number")
      expect(() => ctx.getImageData(1,NaN,10,10)).toThrow("Expected a number")
      expect(() => ctx.createImageData(1,{})).toThrow("Dimensions must be non-zero")
      expect(() => ctx.createImageData(1,NaN)).toThrow("Dimensions must be non-zero")
      expect(() => ctx.putImageData(id,NaN,0)).toThrow("Expected a number")
      expect(() => ctx.putImageData(id,0,0,0,0,NaN,0)).toThrow("Expected a number for `dirtyWidth`")
      expect(() => ctx.putImageData({},0,0)).toThrow("Expected an ImageData as 1st arg")
      expect(() => ctx.drawImage()).toThrow("Expected an Image or a Canvas")
      expect(() => ctx.drawCanvas()).toThrow("Expected an Image or a Canvas")
      expect(() => ctx.fill(NaN)).toThrow("Expected `fillRule`")
      expect(() => ctx.clip(NaN)).toThrow("Expected `fillRule`")
      expect(() => ctx.stroke(NaN)).toThrow("Expected a Path2D")
      expect(() => ctx.fill(NaN, "evenodd")).toThrow("Expected a Path2D")
      expect(() => ctx.clip(NaN, "evenodd")).toThrow("Expected a Path2D")
      expect(() => ctx.fill(p2d, {})).toThrow("Expected `fillRule`")
      expect(() => ctx.createTexture([1, NaN])).toThrow("Expected a number or array")
      expect(() => ctx.createTexture(1, {path:null})).toThrow("Expected a Path2D")
      expect(() => ctx.createTexture(20, {line:{}})).toThrow("Expected a number for `line`")
      expect(() => ctx.createTexture(20, {angle:{}})).toThrow("Expected a number for `angle`")
      expect(() => ctx.createTexture(20, {offset:{}})).toThrow("Expected a number or array")
      expect(() => ctx.createTexture(20, {cap:{}})).toThrow("Expected a string")
      expect(() => ctx.createTexture(20, {cap:""})).toThrow("Expected \"butt\", \"square\"")
      expect(() => ctx.createTexture(20, {offset:[1, NaN]})).toThrow("Expected a number or array")
      expect(() => ctx.isPointInPath(0, 10, 10)).toThrow("Expected `fillRule`")
      expect(() => ctx.isPointInPath(false, 10, 10)).toThrow("Expected `fillRule`")
      expect(() => ctx.isPointInPath({}, 10, 10)).toThrow("Expected `fillRule`")
      expect(() => ctx.isPointInPath({}, 10, 10, "___")).toThrow("Expected a Path2D")
      expect(() => ctx.isPointInPath({}, 10, 10, "evenodd")).toThrow("Expected a Path2D")
      expect(() => ctx.isPointInPath(10, 10, "___")).toThrow("Expected `fillRule`")
      expect(() => ctx.isPointInPath(p2d, 10, 10, "")).toThrow("Expected `fillRule`")
      expect(() => ctx.createLinearGradient(0,0,NaN,1)).toThrow("Expected a number for")
      expect(() => ctx.createRadialGradient(0,0,NaN,0,0,0)).toThrow("Expected a number for")
      expect(() => ctx.createConicGradient(0,NaN,0)).toThrow("Expected a number for")
      expect(() => ctx.createPattern(img, "___")).toThrow("Expected `repetition`")
      expect(() => g.addColorStop(NaN, '#000')).toThrow("Expected a number")
      expect(() => g.addColorStop(0, {})).toThrow("Could not be parsed as a color")
      expect(() => ctx.setLineDash(NaN)).toThrow("Value is not a sequence")
    })

    test('NaN arguments', async () => {
      // silently fail
      expect(() => ctx.setTransform({})).not.toThrow()
      expect(() => ctx.setTransform(0,0,0,NaN,0,0)).not.toThrow()
      expect(() => ctx.translate(NaN,0)).not.toThrow()
      expect(() => ctx.scale(NaN,0)).not.toThrow()
      expect(() => ctx.rotate(NaN)).not.toThrow()
      expect(() => ctx.rect(0,0,NaN,0)).not.toThrow()
      expect(() => ctx.arc(0,0,NaN,0,0)).not.toThrow()
      expect(() => ctx.arc(0,0,NaN,0,0,false)).not.toThrow()
      expect(() => ctx.arc(0,0,NaN,0,0,new Date())).not.toThrow()
      expect(() => ctx.ellipse(0,0,0,NaN,0,0,0)).not.toThrow()
      expect(() => ctx.moveTo(NaN,0)).not.toThrow()
      expect(() => ctx.lineTo(NaN,0)).not.toThrow()
      expect(() => ctx.arcTo(0,0,0,0,NaN)).not.toThrow()
      expect(() => ctx.bezierCurveTo(0,0,0,0,NaN,0)).not.toThrow()
      expect(() => ctx.quadraticCurveTo(0,0,NaN,0)).not.toThrow()
      expect(() => ctx.conicCurveTo(0,0,NaN,0,1)).not.toThrow()
      expect(() => ctx.roundRect(0,0,0,0,NaN)).not.toThrow()
      expect(() => ctx.fillRect(0,0,NaN,0)).not.toThrow()
      expect(() => ctx.strokeRect(0,0,NaN,0)).not.toThrow()
      expect(() => ctx.clearRect(0,0,NaN,0)).not.toThrow()
      expect(() => ctx.fillText("text", 0, NaN)).not.toThrow()
      expect(() => ctx.fillText("text", 0, 0, NaN)).not.toThrow()
      expect(() => ctx.strokeText("text", 0, NaN)).not.toThrow()
      expect(() => ctx.strokeText("text", 0, 0, NaN)).not.toThrow()
      expect(() => ctx.setLineDash([NaN, 0, 0])).not.toThrow()
      expect(() => ctx.outlineText("text", NaN)).not.toThrow()
      expect(() => ctx.drawImage(img,NaN,0)).not.toThrow()
      expect(() => ctx.drawImage(img,0,0,NaN,0)).not.toThrow()
      expect(() => ctx.drawImage(img,0,0,0,0,NaN,0,0,0)).not.toThrow()
      expect(() => ctx.drawCanvas(canvas,NaN,0)).not.toThrow()
      expect(() => ctx.drawCanvas(canvas,0,0,NaN,0)).not.toThrow()
      expect(() => ctx.drawCanvas(canvas,0,0,0,0,NaN,0,0,0)).not.toThrow()

      // no error, returns false
      expect(ctx.isPointInPath(10, NaN, "evenodd")).toEqual(false)
      expect(ctx.isPointInPath(p2d, 10, NaN, "evenodd")).toEqual(false)
      expect(ctx.isPointInPath(p2d, 10)).toEqual(false)
      expect(ctx.isPointInStroke(10, NaN)).toEqual(false)
      expect(ctx.isPointInStroke(p2d, 10, NaN)).toEqual(false)
      expect(ctx.isPointInStroke(p2d, 10)).toEqual(false)
    })

  })

})
