// @ts-check

"use strict"

const {assert, describe, test, beforeEach, afterEach} = require('../runner'),
      {Canvas, DOMMatrix, DOMPoint, ImageData, Path2D, FontLibrary, loadImage} = require('../../lib'),
      css = require('../../lib/classes/css')

const BLACK = [0,0,0,255],
      WHITE = [255,255,255,255],
      GREEN = [0,128,0,255],
      CLEAR = [0,0,0,0]

const _each = (obj, fn) => Object.entries(obj).forEach(([term, val]) => fn(val, term))

describe("Context2D", ()=>{
  let canvas, ctx,
      WIDTH = 512, HEIGHT = 512,
      pixel = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data),
      loadAsset = url => loadImage(`tests/assets/${url}`),
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
      _each({a:0.1, b:0, c:0, d:0.3, e:0, f:0}, (val, term) =>
        assert.nearEqual(matrix[term], val)
      )

      ctx.resetTransform()
      _each({a:1, d:1}, (val, term) =>
        assert.nearEqual(ctx.currentTransform[term], val)
      )

      ctx.currentTransform = matrix
      _each({a:0.1, d:0.3}, (val, term) =>
        assert.nearEqual(ctx.currentTransform[term], val)
      )
    })

    test('font', () => {
      assert.equal(ctx.font, '10px sans-serif')
      let font = '16px Baskerville, serif',
          canonical = css.font(font).canonical;
      ctx.font = font
      assert.equal(ctx.font, canonical)
      ctx.font = 'invalid'
      assert.equal(ctx.font, canonical)
    })

    test('globalAlpha', () => {
      assert.equal(ctx.globalAlpha, 1)
      ctx.globalAlpha = 0.25
      assert.nearEqual(ctx.globalAlpha, 0.25)
      ctx.globalAlpha = -1
      assert.nearEqual(ctx.globalAlpha, 0.25)
      ctx.globalAlpha = 3
      assert.nearEqual(ctx.globalAlpha, 0.25)
      ctx.globalAlpha = 0
      assert.equal(ctx.globalAlpha, 0)
    })

    test('globalCompositeOperation', () => {
      let ops = ["source-over", "destination-over", "copy", "destination", "clear",
                 "source-in", "destination-in", "source-out", "destination-out",
                 "source-atop", "destination-atop", "xor", "lighter", "multiply",
                 "screen", "overlay", "darken", "lighten", "color-dodge", "color-burn",
                 "hard-light", "soft-light", "difference", "exclusion", "hue",
                 "saturation", "color", "luminosity"]

      assert.equal(ctx.globalCompositeOperation, 'source-over')
      ctx.globalCompositeOperation = 'invalid'
      assert.equal(ctx.globalCompositeOperation, 'source-over')

      for (let op of ops){
        ctx.globalCompositeOperation = op
        assert.equal(ctx.globalCompositeOperation, op)
      }
    })

    test('imageSmoothingEnabled', () => {
      assert.equal(ctx.imageSmoothingEnabled, true)
      ctx.imageSmoothingEnabled = false
      assert.equal(ctx.imageSmoothingEnabled, false)
    })


    test('imageSmoothingQuality', () => {
      let vals = ["low", "medium", "high"]

      assert.equal(ctx.imageSmoothingQuality, 'low')
      ctx.imageSmoothingQuality = 'invalid'
      assert.equal(ctx.imageSmoothingQuality, 'low')

      for (let val of vals){
        ctx.imageSmoothingQuality = val
        assert.equal(ctx.imageSmoothingQuality, val)
      }
    })

    test('lineCap', () => {
      let vals = ["butt", "square", "round"]

      assert.equal(ctx.lineCap, 'butt')
      ctx.lineCap = 'invalid'
      assert.equal(ctx.lineCap, 'butt')

      for (let val of vals){
        ctx.lineCap = val
        assert.equal(ctx.lineCap, val)
      }
    })

    test('lineDash', () => {
      assert.deepEqual(ctx.getLineDash(), [])
      ctx.setLineDash([1,2,3,4])
      assert.deepEqual(ctx.getLineDash(), [1,2,3,4])
      ctx.setLineDash([NaN])
      assert.deepEqual(ctx.getLineDash(), [1,2,3,4])
    })

    test('lineJoin', () => {
      let vals = ["miter", "round", "bevel"]

      assert.equal(ctx.lineJoin, 'miter')
      ctx.lineJoin = 'invalid'
      assert.equal(ctx.lineJoin, 'miter')

      for (let val of vals){
        ctx.lineJoin = val
        assert.equal(ctx.lineJoin, val)
      }
    })

    test('lineWidth', () => {
      ctx.lineWidth = 10.0;
      assert.equal(ctx.lineWidth, 10)
      ctx.lineWidth = Infinity;
      assert.equal(ctx.lineWidth, 10)
      ctx.lineWidth = -Infinity;
      assert.equal(ctx.lineWidth, 10)
      ctx.lineWidth = -5;
      assert.equal(ctx.lineWidth, 10)
      ctx.lineWidth = 0;
      assert.equal(ctx.lineWidth, 10)
    })

    test('textAlign', () => {
      let vals = ["start", "end", "left", "center", "right", "justify"]

      assert.equal(ctx.textAlign, 'start')
      ctx.textAlign = 'invalid'
      assert.equal(ctx.textAlign, 'start')

      for (let val of vals){
        ctx.textAlign = val
        assert.equal(ctx.textAlign, val)
      }
    })

  })

  describe("can create", ()=>{
    test('a context', () => {
      assert.strictEqual(canvas.getContext("invalid"), null)
      assert.strictEqual(canvas.getContext("2d"), ctx)
      assert.strictEqual(canvas.pages[0], ctx)
      assert.strictEqual(ctx.canvas, canvas)
    })

    test('multiple pages', () => {
      let ctx2 = canvas.newPage(WIDTH*2, HEIGHT*2);
      assert.equal(canvas.width, WIDTH*2)
      assert.equal(canvas.height, HEIGHT*2)
      assert.strictEqual(canvas.pages[0], ctx)
      assert.strictEqual(canvas.pages[1], ctx2)
      assert.strictEqual(ctx.canvas, canvas)
      assert.strictEqual(ctx2.canvas, canvas)
    })

    test("ImageData", () => {
      let [width, height] = [123, 456],
          bmp = ctx.createImageData(width, height);
      assert.equal(bmp.width, width)
      assert.equal(bmp.height, height)
      assert.equal(bmp.data.length, width * height * 4)
      assert.deepEqual(Array.from(bmp.data.slice(0,4)), CLEAR)

      let blank = new ImageData(width, height)
      assert.equal(blank.width, width)
      assert.equal(blank.height, height)
      assert.equal(blank.data.length, width * height * 4)
      assert.deepEqual(Array.from(blank.data.slice(0,4)), CLEAR)

      new ImageData(blank.data, width, height)
      new ImageData(blank.data, height, width)
      new ImageData(blank.data, width)
      new ImageData(blank.data, height)
      assert.throws(() => new ImageData(blank.data, width+1, height) )
      assert.throws(() => new ImageData(blank.data, width+1) )

      // @ts-ignore
      new ImageData(blank)
      // @ts-ignore
      assert.throws(() => new ImageData(blank.data) )
    })

    describe("CanvasPattern", () => {
      test("from Image", async () => {
        let image = await loadAsset('checkers.png'),
            pattern = ctx.createPattern(image, 'repeat'),
            [width, height] = [20, 20];

        ctx.imageSmoothingEnabled = false
        ctx.fillStyle = pattern;
        ctx.fillRect(0,0,width,height)

        let bmp = ctx.getImageData(0,0,width,height)
        let blackPixel = true
        assert.equal(bmp.data.length, width * height * 4)
        for (var i=0; i<bmp.data.length; i+=4){
          if (i % (bmp.width*4) != 0) blackPixel = !blackPixel
          assert.deepEqual(Array.from(bmp.data.slice(i, i+4)),
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
          assert.deepEqual(Array.from(bmp.data.slice(i, i+4)),
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
          assert.deepEqual(Array.from(bmp.data.slice(i, i+4)),
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
            assert.deepEqual(px, [clr,clr,clr, 255])
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
          assert.doesNotThrow(() => {pat.setTransform(mag, 0, 0, mag, 0, 0)})
          assert.doesNotThrow(() => {pat.setTransform([mag, 0, 0, mag, 0, 0])})
          assert.doesNotThrow(() => {pat.setTransform({a:mag, b:0, c:0, d:mag, e:0, f:0})})
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

        assert.deepEqual(pixel(0,0), WHITE)
        assert.deepEqual(pixel(20,0), BLACK)
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

        assert.deepEqual(pixel(x, y), WHITE)
        assert.deepEqual(pixel(x+inside, y), BLACK)
        assert.deepEqual(pixel(x, y+inside), BLACK)
        assert.deepEqual(pixel(x+outside, y), [255,0,0,255])
        assert.deepEqual(pixel(x, y+outside), [255,0,0,255])
      })

      test("conic", () => {
        // draw a sweep with white at top and black on bottom
        let gradient = ctx.createConicGradient(0, 256, 256);
        ctx.fillStyle = gradient;
        gradient.addColorStop(0,'#fff');
        gradient.addColorStop(.5,'#000');
        gradient.addColorStop(1,'#fff');
        ctx.fillRect(0,0,512,512);

        assert.deepEqual(pixel(5, 256), BLACK)
        assert.deepEqual(pixel(500, 256), WHITE)

        // rotate 90° so black is left and white is right
        gradient = ctx.createConicGradient(Math.PI/2, 256, 256);
        ctx.fillStyle = gradient;
        gradient.addColorStop(0,'#fff');
        gradient.addColorStop(.5,'#000');
        gradient.addColorStop(1,'#fff');
        ctx.fillRect(0,0,512,512);

        assert.deepEqual(pixel(256, 500), WHITE)
        assert.deepEqual(pixel(256, 5), BLACK)
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

        assert.deepEqual(pixel(26, 24), CLEAR)
        assert.deepEqual(pixel(28, 26), BLACK)
        assert.deepEqual(pixel(48, 48), BLACK)
        assert.deepEqual(pixel(55, 40), CLEAR)
      })

      test("with stroked Path2D", async () => {
        ctx.strokeStyle = waves
        ctx.lineWidth = 10
        ctx.moveTo(0,0)
        ctx.lineTo(100, 100)
        ctx.stroke()

        assert.deepEqual(pixel(10, 10), CLEAR)
        assert.deepEqual(pixel(16, 16), BLACK)
        assert.deepEqual(pixel(73, 73), BLACK)
        assert.deepEqual(pixel(75, 75), CLEAR)
      })

      test("with lines", async () => {
        ctx.fillStyle = lines
        ctx.fillRect(10, 10, 80, 80)

        assert.deepEqual(pixel(22, 22), CLEAR)
        assert.deepEqual(pixel(25, 25), BLACK)
        assert.deepEqual(pixel(73, 73), CLEAR)
        assert.deepEqual(pixel(76, 76), BLACK)
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
      assert.deepEqual(pixel(10, 10), [0, 162, 213, 245])
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
      assert.notEqual(pixel(143, 117), BLACK)
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

      assert.deepEqual(pixel(0, 0), BLACK)
      assert.deepEqual(pixel(1, 0), WHITE)
      assert.deepEqual(pixel(0, 1), WHITE)
      assert.deepEqual(pixel(1, 1), BLACK)

      // b | b
      // -----
      // w | b
      ctx.save()
      ctx.clip() // nonzero
      ctx.fillStyle = 'black'
      ctx.fillRect(0, 0, 2, 2)
      ctx.restore()

      assert.deepEqual(pixel(0, 0), BLACK)
      assert.deepEqual(pixel(1, 0), BLACK)
      assert.deepEqual(pixel(0, 1), WHITE)
      assert.deepEqual(pixel(1, 1), BLACK)

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

      assert.deepEqual(pixel(10, 10), BLACK)
      assert.deepEqual(pixel(90, 90), BLACK)
      assert.deepEqual(pixel(22, 22), GREEN)
      assert.deepEqual(pixel(48, 48), GREEN)
      assert.deepEqual(pixel(52, 52), WHITE)

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

      assert.deepEqual(pixel(30, 30), BLACK)
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
      assert.deepEqual(pixel(0, 0), BLACK)
      assert.deepEqual(pixel(1, 0), WHITE)
      assert.deepEqual(pixel(0, 1), WHITE)
      assert.deepEqual(pixel(1, 1), BLACK)

      // b | b
      // -----
      // w | b
      ctx.fill() // nonzero
      assert.deepEqual(pixel(0, 0), BLACK)
      assert.deepEqual(pixel(1, 0), BLACK)
      assert.deepEqual(pixel(0, 1), WHITE)
      assert.deepEqual(pixel(1, 1), BLACK)
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

      _each(argsets, ([args, shouldDraw]) => {
        canvas.width = WIDTH
        ctx.textBaseline = 'middle'
        ctx.textAlign = 'center'
        ctx.fillText(...args)
        assert.equal(ctx.getImageData(0, 0, 20, 20).data.some(a => a), shouldDraw)
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
        assert.deepEqual(pixel(x, y), BLACK)
        assert.deepEqual(pixel(x, HEIGHT - y - 1), BLACK)
        assert.deepEqual(pixel(WIDTH - x - 1, y), BLACK)
        assert.deepEqual(pixel(WIDTH - x - 1, HEIGHT - y - 1), BLACK)
      }

      for (const [x, y] of off){
        assert.deepEqual(pixel(x, y), CLEAR)
        assert.deepEqual(pixel(x, HEIGHT - y - 1), CLEAR)
        assert.deepEqual(pixel(WIDTH - x - 1, y), CLEAR)
        assert.deepEqual(pixel(WIDTH - x - 1, HEIGHT - y - 1), CLEAR)
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
        assert.equal(bmp.width, width)
        assert.equal(bmp.height, height)
        assert.equal(bmp.data.length, width * height * 4)
        assert.deepEqual(Array.from(bmp.data.slice(0,4)), [255,0,0,64])
        assert.deepEqual(Array.from(bmp.data.slice(4,8)), [0,255,0,128])
        assert.deepEqual(Array.from(bmp.data.slice(8,12)), [0,0,255,191])
        for(var x=0; x<width; x++){
          for(var y=0; y<height; y++){
            let i = 4 * (y*width + x)
            let px = Array.from(bmp.data.slice(i,i+4))
            assert.deepEqual(pixel(x,y), px)
          }
        }
      }
    })

    test('putImageData()', () => {
      assert.throws(() => ctx.putImageData({}, 0, 0))
      assert.throws(() => ctx.putImageData(undefined, 0, 0))

      var srcImageData = ctx.createImageData(2,2)
      srcImageData.data.set([
        1,2,3,255, 5,6,7,255,
        0,1,2,255, 4,5,6,255
      ], 0)

      ctx.putImageData(srcImageData, -1, -1);
      var resImageData = ctx.getImageData(0, 0, 2, 2);
      assert.deepEqual(Array.from(resImageData.data), [
        4,5,6,255, 0,0,0,0,
        0,0,0,0,   0,0,0,0
      ])

      // try mask rect
      ctx.reset()
      ctx.putImageData(srcImageData, 0, 0, 1, 1, 1, 1);
      resImageData = ctx.getImageData(0, 0, 2, 2);
      assert.deepEqual(Array.from(resImageData.data), [
        0,0,0,0, 0,0,0,0,
        0,0,0,0, 4,5,6,255
      ])

      // try negative dimensions
      ctx.reset()
      ctx.putImageData(srcImageData, 0, 0, 1, 1, -1, -1);
      resImageData = ctx.getImageData(0, 0, 2, 2);
      assert.deepEqual(Array.from(resImageData.data), [
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

      assert.equal(ctx.isPointInPath(...inStroke), false)
      assert.equal(ctx.isPointInStroke(...inStroke), true)

      assert.equal(ctx.isPointInPath(...inFill), true)
      assert.equal(ctx.isPointInStroke(...inFill), false)

      assert.equal(ctx.isPointInPath(...inBoth), true)
      assert.equal(ctx.isPointInStroke(...inBoth), true)
    })

    test("isPointInPath(Path2D)", () => {
      let inStroke = [100, 94],
          inFill = [150, 150],
          inBoth = [100, 100];

      let path = new Path2D()
      path.rect(100,100,100,100)
      ctx.lineWidth = 12

      assert.equal(ctx.isPointInPath(path, ...inStroke), false)
      assert.equal(ctx.isPointInStroke(path, ...inStroke), true)

      assert.equal(ctx.isPointInPath(path, ...inFill), true)
      assert.equal(ctx.isPointInStroke(path, ...inFill), false)

      assert.equal(ctx.isPointInPath(path, ...inBoth), true)
      assert.equal(ctx.isPointInStroke(path, ...inBoth), true)
    })

    test("letterSpacing", () => {
        FontLibrary.use(`tests/assets/fonts/Monoton-Regular.woff`)

        let [x, y] = [40, 100]
        let size = 32
        let text = "RR"
        ctx.font = `${size}px Monoton`
        ctx.letterSpacing = '20px'
        ctx.fillStyle = 'black'
        ctx.fillText(text, x, y)

        // there should be no initial added space indenting the beginning of the line
        assert.equal(ctx.getImageData(x, y-size, 10, size).data.some(a => a), true)

        // there should be whitespace between the first and second characters
        assert.equal(ctx.getImageData(x+28, y-size, 18, size).data.some(a => a), false)

        // check whether upstream has fixed the indent bug and our compensation is now outdenting
        assert.equal(ctx.getImageData(x-20, y-size, 18, size).data.some(a => a), false)

        // make sure the extra space skia adds to the beginning/end have been subtracted
        assert.nearEqual(ctx.measureText(text).width, 74)
        ctx.textWrap = true
        assert.nearEqual(ctx.measureText(text).width, 74)
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
      assert(ø < _)
      assert(_ < __)
      assert(foo < foobar)
      assert(__foo > foo)
      assert(__foo__ > __foo)

      // start from the default, alphabetic baseline
      let msg = "Lordran gypsum",
          metrics = ctx.measureText(msg)

      // + means up, - means down when it comes to baselines
      assert.equal(metrics.alphabeticBaseline, 0)
      assert(metrics.hangingBaseline > 0)
      assert(metrics.ideographicBaseline < 0)

      // for ascenders + means up, for descenders + means down
      assert(metrics.actualBoundingBoxAscent > 0)
      assert(metrics.actualBoundingBoxDescent > 0)
      assert(metrics.actualBoundingBoxAscent > metrics.actualBoundingBoxDescent)

      // make sure the polarity has flipped for 'top' baseline
      ctx.textBaseline = "top"
      metrics = ctx.measureText("Lordran gypsum")
      assert(metrics.alphabeticBaseline < 0)
      assert(metrics.hangingBaseline < 0)
      assert(metrics.actualBoundingBoxAscent < 0)
      assert(metrics.actualBoundingBoxDescent > 0)

      // width calculations should be the same (modulo rounding) for any alignment
      let [lft, cnt, rgt] = ['left', 'center', 'right'].map(align => {
        ctx.textAlign = align
        return ctx.measureText(msg).width
      })
      assert.nearEqual(lft, cnt)
      assert.nearEqual(cnt, rgt)

      // make sure string indices account for trailing whitespace and non-8-bit characters
      let text = ' 石 ',
          {startIndex, endIndex} = ctx.measureText(text).lines[0]
      assert.equal(text.substring(startIndex, endIndex), text)
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
      assert.deepEqual(pixel(x, y - 5), CLEAR)
      assert.deepEqual(pixel(x + 25, y), GREEN)
      assert.deepEqual(pixel(x + 75, y), CLEAR)
      assert.deepEqual(pixel(x - 25, y), WHITE)
      assert.deepEqual(pixel(x - 75, y), BLACK)
      assert.deepEqual(pixel(x - 100, y), CLEAR)

      y = HEIGHT*.9 - 2
      assert.deepEqual(pixel(x + 100, y), GREEN)
      assert.deepEqual(pixel(x + 130, y), CLEAR)
      assert.deepEqual(pixel(x - 75, y), WHITE)
      assert.deepEqual(pixel(x - 200, y), BLACK)
      assert.deepEqual(pixel(0, y), CLEAR)
    })

    test('drawImage()', async () => {
      let image = await loadAsset('checkers.png')
      ctx.imageSmoothingEnabled = false

      ctx.drawImage(image, 0,0)
      assert.deepEqual(pixel(0, 0), BLACK)
      assert.deepEqual(pixel(1, 0), WHITE)
      assert.deepEqual(pixel(0, 1), WHITE)
      assert.deepEqual(pixel(1, 1), BLACK)

      ctx.drawImage(image,-256,-256,512,512)
      assert.deepEqual(pixel(0, 0), BLACK)
      assert.deepEqual(pixel(149, 149), BLACK)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.save()
      ctx.translate(WIDTH/2, HEIGHT/2)
      ctx.rotate(.25*Math.PI)
      ctx.drawImage(image,-256,-256,512,512)
      ctx.restore()
      assert.deepEqual(pixel(0, 0), CLEAR)
      assert.deepEqual(pixel(WIDTH/2, HEIGHT*.25), BLACK)
      assert.deepEqual(pixel(WIDTH/2, HEIGHT*.75), BLACK)
      assert.deepEqual(pixel(WIDTH*.25, HEIGHT/2), WHITE)
      assert.deepEqual(pixel(WIDTH*.75, HEIGHT/2), WHITE)
      assert.deepEqual(pixel(WIDTH-1, HEIGHT-1), CLEAR)

      let srcCanvas = new Canvas(3, 3),
          srcCtx = srcCanvas.getContext("2d");
      srcCtx.fillStyle = 'green'
      srcCtx.fillRect(0,0,3,3)
      srcCtx.clearRect(1,1,1,1)

      ctx.drawImage(srcCanvas, 0,0)
      assert.deepEqual(pixel(0, 0), GREEN)
      assert.deepEqual(pixel(1, 1), CLEAR)
      assert.deepEqual(pixel(2, 2), GREEN)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.drawImage(srcCanvas,-2,-2,6,6)
      assert.deepEqual(pixel(0, 0), CLEAR)
      assert.deepEqual(pixel(2, 0), GREEN)
      assert.deepEqual(pixel(2, 2), GREEN)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.save()
      ctx.translate(WIDTH/2, HEIGHT/2)
      ctx.rotate(.25*Math.PI)
      ctx.drawImage(srcCanvas,-256,-256,512,512)
      ctx.restore()
      assert.deepEqual(pixel(WIDTH/2, HEIGHT*.25), GREEN)
      assert.deepEqual(pixel(WIDTH/2, HEIGHT*.75), GREEN)
      assert.deepEqual(pixel(WIDTH*.25, HEIGHT/2), GREEN)
      assert.deepEqual(pixel(WIDTH*.75, HEIGHT/2), GREEN)
      assert.deepEqual(pixel(WIDTH/2, HEIGHT/2), CLEAR)
    })

    test('drawCanvas()', async () => {
      let srcCanvas = new Canvas(3, 3),
          srcCtx = srcCanvas.getContext("2d");
      srcCtx.fillStyle = 'green'
      srcCtx.fillRect(0,0,3,3)
      srcCtx.clearRect(1,1,1,1)

      ctx.drawCanvas(srcCanvas, 0,0)
      assert.deepEqual(pixel(0, 0), GREEN)
      assert.deepEqual(pixel(1, 1), CLEAR)
      assert.deepEqual(pixel(2, 2), GREEN)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.drawCanvas(srcCanvas,-2,-2,6,6)
      assert.deepEqual(pixel(0, 0), CLEAR)
      assert.deepEqual(pixel(2, 0), GREEN)
      assert.deepEqual(pixel(2, 2), GREEN)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.save()
      ctx.translate(WIDTH/2, HEIGHT/2)
      ctx.rotate(.25*Math.PI)
      ctx.drawCanvas(srcCanvas,-256,-256,512,512)
      ctx.restore()
      assert.deepEqual(pixel(WIDTH/2, HEIGHT*.25), GREEN)
      assert.deepEqual(pixel(WIDTH/2, HEIGHT*.75), GREEN)
      assert.deepEqual(pixel(WIDTH*.25, HEIGHT/2), GREEN)
      assert.deepEqual(pixel(WIDTH*.75, HEIGHT/2), GREEN)
      assert.deepEqual(pixel(WIDTH/2, HEIGHT/2), CLEAR)

      ctx.clearRect(0,0,WIDTH,HEIGHT)
      ctx.drawCanvas(srcCanvas, 1,1,2,2, 0,0,2,2)
      assert.deepEqual(pixel(0, 0), CLEAR)
      assert.deepEqual(pixel(0, 1), GREEN)
      assert.deepEqual(pixel(1, 0), GREEN)
      assert.deepEqual(pixel(1, 1), GREEN)

      let image = await loadAsset('checkers.png')
      assert.doesNotThrow( () => ctx.drawCanvas(image, 0, 0) )
    })

    test('reset()', async () => {
      ctx.fillStyle = 'green'
      ctx.scale(2, 2)
      ctx.translate(0, -HEIGHT/4)

      ctx.fillRect(WIDTH/4, HEIGHT/4, WIDTH/8, HEIGHT/8)
      assert.deepEqual(pixel(WIDTH * .5 + 1, 0), GREEN)
      assert.deepEqual(pixel(WIDTH * .75 - 1, 0), GREEN)

      ctx.beginPath()
      ctx.rect(WIDTH/4, HEIGHT/2, 100, 100)
      ctx.reset()
      ctx.fill()
      assert.deepEqual(pixel(WIDTH/2 + 1, HEIGHT/2 + 1), CLEAR)
      assert.deepEqual(pixel(WIDTH * .5 + 1, 0), CLEAR)
      assert.deepEqual(pixel(WIDTH * .75 - 1, 0), CLEAR)

      ctx.globalAlpha = 0.4
      ctx.reset()
      ctx.fillRect(WIDTH/2, HEIGHT/2, 3, 3)
      assert.deepEqual(pixel(WIDTH/2 + 1, HEIGHT/2 + 1), BLACK)
    })

    describe("transform()", ()=>{
      const a=0.1, b=0, c=0, d=0.3, e=0, f=0

      test('with args list', () => {
        ctx.transform(a, b, c, d, e, f)
        let matrix = ctx.currentTransform
        _each({a, b, c, d, e, f}, (val, term) =>
          assert.nearEqual(matrix[term], val)
        )
      })

      test('with DOMMatrix', () => {
        ctx.transform(new DOMMatrix().scale(0.1, 0.3));
        let matrix = ctx.currentTransform
        _each({a, b, c, d, e, f}, (val, term) =>
          assert.nearEqual(matrix[term], val)
        )
      })

      test('with matrix-like object', () => {
        ctx.transform({a, b, c, d, e, f});
        let matrix = ctx.currentTransform
        _each({a, b, c, d, e, f}, (val, term) =>
          assert.nearEqual(matrix[term], val)
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
          assert.equal(matrix.toString(), transforms[input])
          assert.equal(roundtrip.toString(), transforms[input])
        }

        // check that the context can also take a string
        ctx.transform(`scale(${a}, ${d})`);
        let matrix = ctx.currentTransform
        _each({a, b, c, d, e, f}, (val, term) =>
          assert.nearEqual(matrix[term], val)
        )
      })

      test('rejects invalid args', () => {
        assert.throws( () => ctx.transform("nonesuch"), /Invalid transform matrix/)
        assert.throws( () => ctx.transform(0, 0, 0), /not enough arguments/)
        assert.doesNotThrow( () => ctx.transform(0, 0, 0, NaN, 0, 0))
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

      _each(cases, (spec, font) => {
        let expected = {style:"normal", stretch:"normal", variant:"normal", ...spec},
            parsed = css.font(font);
        assert.matchesSubset(parsed, expected)
      })

    })

    test('colors', () => {
      ctx.fillStyle = '#ffccaa';
      assert.equal(ctx.fillStyle, '#ffccaa');

      ctx.fillStyle = '#FFCCAA';
      assert.equal(ctx.fillStyle, '#ffccaa');

      ctx.fillStyle = '#FCA';
      assert.equal(ctx.fillStyle, '#ffccaa');

      ctx.fillStyle = '#0ff';
      ctx.fillStyle = '#FGG';
      assert.equal(ctx.fillStyle, '#00ffff');

      ctx.fillStyle = '#fff';
      ctx.fillStyle = 'afasdfasdf';
      assert.equal(ctx.fillStyle, '#ffffff');

      // #rgba and #rrggbbaa

      ctx.fillStyle = '#ffccaa80'
      assert.equal(ctx.fillStyle, 'rgba(255, 204, 170, 0.502)')

      ctx.fillStyle = '#acf8'
      assert.equal(ctx.fillStyle, 'rgba(170, 204, 255, 0.533)')

      ctx.fillStyle = '#BEAD'
      assert.equal(ctx.fillStyle, 'rgba(187, 238, 170, 0.867)')

      ctx.fillStyle = 'rgb(255,255,255)';
      assert.equal(ctx.fillStyle, '#ffffff');

      ctx.fillStyle = 'rgb(0,0,0)';
      assert.equal(ctx.fillStyle, '#000000');

      ctx.fillStyle = 'rgb( 0  ,   0  ,  0)';
      assert.equal(ctx.fillStyle, '#000000');

      ctx.fillStyle = 'rgba( 0  ,   0  ,  0, 1)';
      assert.equal(ctx.fillStyle, '#000000');

      ctx.fillStyle = 'rgba( 255, 200, 90, 0.5)';
      assert.equal(ctx.fillStyle, 'rgba(255, 200, 90, 0.502)');

      ctx.fillStyle = 'rgba( 255, 200, 90, 0.75)';
      assert.equal(ctx.fillStyle, 'rgba(255, 200, 90, 0.749)');

      ctx.fillStyle = 'rgba( 255, 200, 90, 0.7555)';
      assert.equal(ctx.fillStyle, 'rgba(255, 200, 90, 0.757)');

      ctx.fillStyle = 'rgba( 255, 200, 90, .7555)';
      assert.equal(ctx.fillStyle, 'rgba(255, 200, 90, 0.757)');

      ctx.fillStyle = 'rgb(0, 0, 9000)';
      assert.equal(ctx.fillStyle, '#0000ff');

      ctx.fillStyle = 'rgba(0, 0, 0, 42.42)';
      assert.equal(ctx.fillStyle, '#000000');

      // hsl / hsla tests

      ctx.fillStyle = 'hsl(0, 0%, 0%)';
      assert.equal(ctx.fillStyle, '#000000');

      ctx.fillStyle = 'hsl(3600, -10%, -10%)';
      assert.equal(ctx.fillStyle, '#000000');

      ctx.fillStyle = 'hsl(10, 100%, 42%)';
      assert.equal(ctx.fillStyle, '#d62400');

      ctx.fillStyle = 'hsl(370, 120%, 42%)';
      assert.equal(ctx.fillStyle, '#d62400');

      ctx.fillStyle = 'hsl(0, 100%, 100%)';
      assert.equal(ctx.fillStyle, '#ffffff');

      ctx.fillStyle = 'hsl(0, 150%, 150%)';
      assert.equal(ctx.fillStyle, '#ffffff');

      ctx.fillStyle = 'hsl(237, 76%, 25%)';
      assert.equal(ctx.fillStyle, '#0f1470');

      ctx.fillStyle = 'hsl(240, 73%, 25%)';
      assert.equal(ctx.fillStyle, '#11116e');

      ctx.fillStyle = 'hsl(262, 32%, 42%)';
      assert.equal(ctx.fillStyle, '#62498d');

      ctx.fillStyle = 'hsla(0, 0%, 0%, 1)';
      assert.equal(ctx.fillStyle, '#000000');

      ctx.fillStyle = 'hsla(0, 100%, 100%, 1)';
      assert.equal(ctx.fillStyle, '#ffffff');

      ctx.fillStyle = 'hsla(120, 25%, 75%, 0.5)';
      assert.equal(ctx.fillStyle, 'rgba(175, 207, 175, 0.502)');

      ctx.fillStyle = 'hsla(240, 75%, 25%, 0.75)';
      assert.equal(ctx.fillStyle, 'rgba(16, 16, 112, 0.749)');

      ctx.fillStyle = 'hsla(172.0, 33.00000e0%, 42%, 1)';
      assert.equal(ctx.fillStyle, '#488e85');

      ctx.fillStyle = 'hsl(124.5, 76.1%, 47.6%)';
      assert.equal(ctx.fillStyle, '#1dd62b');

      ctx.fillStyle = 'hsl(1.24e2, 760e-1%, 4.7e1%)';
      assert.equal(ctx.fillStyle, '#1dd329');

      // case-insensitive css names

      ctx.fillStyle = "sILveR";
      assert.equal(ctx.fillStyle, "#c0c0c0");

      // wrong type args

      let transparent = 'rgba(0, 0, 0, 0)'
      ctx.fillStyle = 'transparent'
      assert.equal(ctx.fillStyle, transparent);

      ctx.fillStyle = null
      assert.equal(ctx.fillStyle, transparent);

      ctx.fillStyle = NaN
      assert.equal(ctx.fillStyle, transparent);

      ctx.fillStyle = [undefined, 255, false]
      assert.equal(ctx.fillStyle, transparent);

      ctx.fillStyle = true
      assert.equal(ctx.fillStyle, transparent);

      ctx.fillStyle = {}
      assert.equal(ctx.fillStyle, transparent);

      // objects with .toString methods

      ctx.fillStyle = {toString:() => 'red'}
      assert.equal(ctx.fillStyle, '#ff0000');

      ctx.fillStyle = 'transparent'
      ctx.fillStyle = {toString:'red'}
      assert.equal(ctx.fillStyle, transparent);

      ctx.fillStyle = {toString:() => 'gobbledygook'}
      assert.equal(ctx.fillStyle, transparent);

      ctx.fillStyle = {toString:() => NaN}
      assert.equal(ctx.fillStyle, transparent);

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
      let ERR =  /not enough arguments/
      assert.throws(() => ctx.transform(), ERR)
      assert.throws(() => ctx.transform(0,0,0,0,0), ERR)
      assert.throws(() => ctx.setTransform(0,0,0,0,0), ERR)
      assert.throws(() => ctx.translate(0), ERR)
      assert.throws(() => ctx.scale(0), ERR)
      assert.throws(() => ctx.rotate(), ERR)
      assert.throws(() => ctx.rect(0,0,0), ERR)
      assert.throws(() => ctx.arc(0,0,0,0), ERR)
      assert.throws(() => ctx.arcTo(0,0,0,0), ERR)
      assert.throws(() => ctx.ellipse(0,0,0,0,0,0), ERR)
      assert.throws(() => ctx.moveTo(0), ERR)
      assert.throws(() => ctx.lineTo(0), ERR)
      assert.throws(() => ctx.bezierCurveTo(0,0,0,0,0), ERR)
      assert.throws(() => ctx.quadraticCurveTo(0,0,0), ERR)
      assert.throws(() => ctx.conicCurveTo(0,0,0,0), ERR)
      assert.throws(() => ctx.roundRect(0,0,0), ERR)
      assert.throws(() => ctx.fillRect(0,0,0), ERR)
      assert.throws(() => ctx.strokeRect(0,0,0), ERR)
      assert.throws(() => ctx.clearRect(0,0,0), ERR)
      assert.throws(() => ctx.fillText("text",0), ERR)
      assert.throws(() => ctx.isPointInPath(10), ERR)
      assert.throws(() => ctx.isPointInStroke(10), ERR)
      assert.throws(() => ctx.createLinearGradient(0,0,1), ERR)
      assert.throws(() => ctx.createRadialGradient(0,0,0,0,0), ERR)
      assert.throws(() => ctx.createConicGradient(0,0), ERR)
      assert.throws(() => ctx.setLineDash(), ERR)
      assert.throws(() => ctx.createImageData(), ERR)
      assert.throws(() => ctx.createPattern(img), ERR)
      assert.throws(() => ctx.createTexture(), ERR)
      assert.throws(() => ctx.getImageData(1,1,10), ERR)
      assert.throws(() => ctx.putImageData({},0), ERR)
      assert.throws(() => ctx.putImageData(id,0,0,0,0,0), ERR)
      assert.throws(() => ctx.drawImage(img), ERR)
      assert.throws(() => ctx.drawImage(img,0), ERR)
      assert.throws(() => ctx.drawImage(img,0,0,0), ERR)
      assert.throws(() => ctx.drawImage(img,0,0,0,0,0), ERR)
      assert.throws(() => ctx.drawImage(img,0,0,0,0,0,0), ERR)
      assert.throws(() => ctx.drawImage(img,0,0,0,0,0,0,0), ERR)
      assert.throws(() => ctx.drawCanvas(canvas), ERR)
      assert.throws(() => ctx.drawCanvas(canvas,0), ERR)
      assert.throws(() => ctx.drawCanvas(canvas,0,0,0), ERR)
      assert.throws(() => ctx.drawCanvas(canvas,0,0,0,0,0), ERR)
      assert.throws(() => ctx.drawCanvas(canvas,0,0,0,0,0,0), ERR)
      assert.throws(() => ctx.drawCanvas(canvas,0,0,0,0,0,0,0), ERR)
      assert.throws(() => g.addColorStop(0), ERR)
    })

    test('value errors', async () => {
      assert.throws(() => ctx.ellipse(0,0,-10,-10,0,0,0,false), /Radius value must be positive/)
      assert.throws(() => ctx.arcTo(0,0,0,0,-10), /Radius value must be positive/)
      assert.throws(() => ctx.roundRect(0,0,0,0,-10), /Corner radius cannot be negative/)
      assert.throws(() => ctx.createImageData(1,0), /Dimensions must be non-zero/)
      assert.throws(() => ctx.getImageData(1,1,NaN,10), /Expected a number/)
      assert.throws(() => ctx.getImageData(1,NaN,10,10), /Expected a number/)
      assert.throws(() => ctx.createImageData(1,{}), /Dimensions must be non-zero/)
      assert.throws(() => ctx.createImageData(1,NaN), /Dimensions must be non-zero/)
      assert.throws(() => ctx.putImageData(id,NaN,0), /Expected a number/)
      assert.throws(() => ctx.putImageData(id,0,0,0,0,NaN,0), /Expected a number for `dirtyWidth`/)
      assert.throws(() => ctx.putImageData({},0,0), /Expected an ImageData as 1st arg/)
      assert.throws(() => ctx.drawImage(), /Expected an Image or a Canvas/)
      assert.throws(() => ctx.drawCanvas(), /Expected an Image or a Canvas/)
      assert.throws(() => ctx.fill(NaN), /Expected `fillRule`/)
      assert.throws(() => ctx.clip(NaN), /Expected `fillRule`/)
      assert.throws(() => ctx.stroke(NaN), /Expected a Path2D/)
      assert.throws(() => ctx.fill(NaN, "evenodd"), /Expected a Path2D/)
      assert.throws(() => ctx.clip(NaN, "evenodd"), /Expected a Path2D/)
      assert.throws(() => ctx.fill(p2d, {}), /Expected `fillRule`/)
      assert.throws(() => ctx.createTexture([1, NaN]), /Expected a number or array/)
      assert.throws(() => ctx.createTexture(1, {path:null}), /Expected a Path2D/)
      assert.throws(() => ctx.createTexture(20, {line:{}}), /Expected a number for `line`/)
      assert.throws(() => ctx.createTexture(20, {angle:{}}), /Expected a number for `angle`/)
      assert.throws(() => ctx.createTexture(20, {offset:{}}), /Expected a number or array/)
      assert.throws(() => ctx.createTexture(20, {cap:{}}), /Expected a string/)
      assert.throws(() => ctx.createTexture(20, {cap:""}), /Expected \"butt\", \"square\"/)
      assert.throws(() => ctx.createTexture(20, {offset:[1, NaN]}), /Expected a number or array/)
      assert.throws(() => ctx.isPointInPath(0, 10, 10), /Expected `fillRule`/)
      assert.throws(() => ctx.isPointInPath(false, 10, 10), /Expected `fillRule`/)
      assert.throws(() => ctx.isPointInPath({}, 10, 10), /Expected `fillRule`/)
      assert.throws(() => ctx.isPointInPath({}, 10, 10, "___"), /Expected a Path2D/)
      assert.throws(() => ctx.isPointInPath({}, 10, 10, "evenodd"), /Expected a Path2D/)
      assert.throws(() => ctx.isPointInPath(10, 10, "___"), /Expected `fillRule`/)
      assert.throws(() => ctx.isPointInPath(p2d, 10, 10, ""), /Expected `fillRule`/)
      assert.throws(() => ctx.createLinearGradient(0,0,NaN,1), /Expected a number for/)
      assert.throws(() => ctx.createRadialGradient(0,0,NaN,0,0,0), /Expected a number for/)
      assert.throws(() => ctx.createConicGradient(0,NaN,0), /Expected a number for/)
      assert.throws(() => ctx.createPattern(img, "___"), /Expected `repetition`/)
      assert.throws(() => g.addColorStop(NaN, '#000'), /Expected a number/)
      assert.throws(() => g.addColorStop(0, {}), /Could not be parsed as a color/)
      assert.throws(() => ctx.setLineDash(NaN), /Value is not a sequence/)
    })

    test('NaN arguments', async () => {
      // silently fail
      assert.doesNotThrow(() => ctx.setTransform({}))
      assert.doesNotThrow(() => ctx.setTransform(0,0,0,NaN,0,0))
      assert.doesNotThrow(() => ctx.translate(NaN,0))
      assert.doesNotThrow(() => ctx.scale(NaN,0))
      assert.doesNotThrow(() => ctx.rotate(NaN))
      assert.doesNotThrow(() => ctx.rect(0,0,NaN,0))
      assert.doesNotThrow(() => ctx.arc(0,0,NaN,0,0))
      assert.doesNotThrow(() => ctx.arc(0,0,NaN,0,0,false))
      assert.doesNotThrow(() => ctx.arc(0,0,NaN,0,0,new Date()))
      assert.doesNotThrow(() => ctx.ellipse(0,0,0,NaN,0,0,0))
      assert.doesNotThrow(() => ctx.moveTo(NaN,0))
      assert.doesNotThrow(() => ctx.lineTo(NaN,0))
      assert.doesNotThrow(() => ctx.arcTo(0,0,0,0,NaN))
      assert.doesNotThrow(() => ctx.bezierCurveTo(0,0,0,0,NaN,0))
      assert.doesNotThrow(() => ctx.quadraticCurveTo(0,0,NaN,0))
      assert.doesNotThrow(() => ctx.conicCurveTo(0,0,NaN,0,1))
      assert.doesNotThrow(() => ctx.roundRect(0,0,0,0,NaN))
      assert.doesNotThrow(() => ctx.fillRect(0,0,NaN,0))
      assert.doesNotThrow(() => ctx.strokeRect(0,0,NaN,0))
      assert.doesNotThrow(() => ctx.clearRect(0,0,NaN,0))
      assert.doesNotThrow(() => ctx.fillText("text", 0, NaN))
      assert.doesNotThrow(() => ctx.fillText("text", 0, 0, NaN))
      assert.doesNotThrow(() => ctx.strokeText("text", 0, NaN))
      assert.doesNotThrow(() => ctx.strokeText("text", 0, 0, NaN))
      assert.doesNotThrow(() => ctx.setLineDash([NaN, 0, 0]))
      assert.doesNotThrow(() => ctx.outlineText("text", NaN))
      assert.doesNotThrow(() => ctx.drawImage(img,NaN,0))
      assert.doesNotThrow(() => ctx.drawImage(img,0,0,NaN,0))
      assert.doesNotThrow(() => ctx.drawImage(img,0,0,0,0,NaN,0,0,0))
      assert.doesNotThrow(() => ctx.drawCanvas(canvas,NaN,0))
      assert.doesNotThrow(() => ctx.drawCanvas(canvas,0,0,NaN,0))
      assert.doesNotThrow(() => ctx.drawCanvas(canvas,0,0,0,0,NaN,0,0,0))

      // no error, returns false
      assert.equal(ctx.isPointInPath(10, NaN, "evenodd"), false)
      assert.equal(ctx.isPointInPath(p2d, 10, NaN, "evenodd"), false)
      assert.equal(ctx.isPointInPath(p2d, 10), false)
      assert.equal(ctx.isPointInStroke(10, NaN), false)
      assert.equal(ctx.isPointInStroke(p2d, 10, NaN), false)
      assert.equal(ctx.isPointInStroke(p2d, 10), false)
    })

  })

})
