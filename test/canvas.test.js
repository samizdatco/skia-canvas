const _ = require('lodash'),
      fs = require('fs'),
      {Canvas, DOMMatrix, FontLibrary, loadImage} = require('../lib'),
      {parseFont} = require('../lib/parse');

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
      expect(pixel(0,0)).toEqual([255,255,255,255])

      // resizing also clears content & resets state
      canvas.width = 123
      canvas.height = 456
      expect(canvas.width).toBe(123)
      expect(canvas.height).toBe(456)
      expect(ctx.fillStyle).toBe('#000000')
      expect(pixel(0,0)).toEqual([0,0,0,0])
    })
  })

})

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
          canonical = parseFont(font).canonical;
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
      ctx.setLineDash(null)
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
      let vals = ["start", "end", "left", "center", "right"]

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
      expect(Array.from(bmp.data.slice(0,4))).toEqual([0,0,0,0])
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
            blackPixel ? [0,0,0,255] : [255,255,255,255]
          )
        }
      })

      test("from Canvas", () => {
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
            blackPixel ? [0,0,0,255] : [255,255,255,255]
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
          ctx.fillRect(0,0, w*mag, h*mag);
          isCheckerboard(ctx, w*mag, h*mag);
        })
      })
    })

    describe("CanvasGradient", () => {
      test("linear", () => {
        let gradient = ctx.createLinearGradient(1,1,19,1);
        gradient.addColorStop(0,'#fff');
        gradient.addColorStop(1,'#000');
        ctx.fillStyle = gradient;
        ctx.fillRect(0,0,21,1);

        expect(pixel(0,0)).toEqual([255,255,255,255])
        expect(pixel(20,0)).toEqual([0,0,0,255])
      })

      test("radial", () => {
        let [x, y, inside, outside] = [100, 100, 45, 55],
            inner = [x, y, 25],
            outer = [x, y, 50],
            gradient = ctx.createRadialGradient(...inner, ...outer);
        gradient.addColorStop(0,'#fff');
        gradient.addColorStop(.5,'#000');
        gradient.addColorStop(1,'#000');
        gradient.addColorStop(1,'red');
        ctx.fillStyle = gradient
        ctx.fillRect(0,0, 200,200)

        expect(pixel(x, y)).toEqual([255,255,255,255])
        expect(pixel(x+inside, y)).toEqual([0,0,0,255])
        expect(pixel(x, y+inside)).toEqual([0,0,0,255])
        expect(pixel(x+outside, y)).toEqual([255,0,0,255])
        expect(pixel(x, y+outside)).toEqual([255,0,0,255])
      })

      test("conic", () => {
        // draw a sweep with white at top and black on bottom
        let gradient = ctx.createConicGradient(0, 256, 256);
        gradient.addColorStop(0,'#fff');
        gradient.addColorStop(.5,'#000');
        gradient.addColorStop(1,'#fff');
        ctx.fillStyle = gradient;
        ctx.fillRect(0,0,512,512);

        expect(pixel(256,5)).toEqual([255,255,255,255])
        expect(pixel(256,500)).toEqual([0,0,0,255])

        // rotate 90° so black is left and white is right
        gradient = ctx.createConicGradient(Math.PI/2, 256, 256);
        gradient.addColorStop(0,'#fff');
        gradient.addColorStop(.5,'#000');
        gradient.addColorStop(1,'#fff');
        ctx.fillStyle = gradient;
        ctx.fillRect(0,0,512,512);

        expect(pixel(500,256)).toEqual([255,255,255,255])
        expect(pixel(5,256)).toEqual([0,0,0,255])
      })
    })
  })

  describe("supports", () => {
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

      expect(pixel(0, 0)).toEqual([0,0,0,255])
      expect(pixel(1, 0)).toEqual([255,255,255,255])
      expect(pixel(0, 1)).toEqual([255,255,255,255])
      expect(pixel(1, 1)).toEqual([0,0,0,255])

      // b | b
      // -----
      // w | b
      ctx.clip() // nonzero
      ctx.fillStyle = 'black'
      ctx.fillRect(0, 0, 2, 2)

      expect(pixel(0, 0)).toEqual([0,0,0,255])
      expect(pixel(1, 0)).toEqual([0,0,0,255])
      expect(pixel(0, 1)).toEqual([255,255,255,255])
      expect(pixel(1, 1)).toEqual([0,0,0,255])
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
      expect(pixel(0, 0)).toEqual([0,0,0,255])
      expect(pixel(1, 0)).toEqual([255,255,255,255])
      expect(pixel(0, 1)).toEqual([255,255,255,255])
      expect(pixel(1, 1)).toEqual([0,0,0,255])

      // b | b
      // -----
      // w | b
      ctx.fill() // nonzero
      expect(pixel(0, 0)).toEqual([0,0,0,255])
      expect(pixel(1, 0)).toEqual([0,0,0,255])
      expect(pixel(0, 1)).toEqual([255,255,255,255])
      expect(pixel(1, 1)).toEqual([0,0,0,255])
    })

    test("fillText()", () => {
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

    test('getImageData()', () => {
      ctx.fillStyle = 'rgba(255,0,0, 0.25)'
      ctx.fillRect(0,0,1,6)

      ctx.fillStyle = 'rgba(0,255,0, 0.5)'
      ctx.fillRect(1,0,1,6)

      ctx.fillStyle = 'rgba(0,0,255, 0.75)'
      ctx.fillRect(2,0,1,6)

      let [width, height] = [3, 6],
          bmp = ctx.getImageData(0,0, width,height);
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
    })

    test("isPointInPath()", () => {
      let inStroke = [100, 94],
          inFill = [150, 150],
          inBoth = [100, 100];
      ctx.lineWidth = 12
      ctx.rect(100,100,100,100)

      expect(ctx.isPointInPath(...inStroke)).toBe(false)
      expect(ctx.isPointInStroke(...inStroke)).toBe(true)

      expect(ctx.isPointInPath(...inFill)).toBe(true)
      expect(ctx.isPointInStroke(...inFill)).toBe(false)

      expect(ctx.isPointInPath(...inBoth)).toBe(true)
      expect(ctx.isPointInStroke(...inBoth)).toBe(true)
    })

    test("measureText()", () => {
      let foo = ctx.measureText('foo').width,
          foobar = ctx.measureText('foobar').width,
          __foo = ctx.measureText('  foo').width;
      expect(foo).toBeLessThan(foobar)
      expect(__foo).toBeGreaterThan(foo)

      // start from the default, alphabetic baseline
      ctx.font = "20px Arial, DejaVu Sans"
      var metrics = ctx.measureText("Lordran gypsum")

      // + means up, - means down when it comes to baselines
      expect(metrics.alphabeticBaseline).toBe(0)
      expect(metrics.hangingBaseline).toBeGreaterThan(0)
      expect(metrics.ideographicBaseline).toBeLessThan(0)

      // for ascenders + means up, for descenders + means down
      expect(metrics.actualBoundingBoxAscent).toBeGreaterThan(0)
      expect(metrics.actualBoundingBoxDescent).toBeGreaterThan(0)

      ctx.textBaseline = "bottom"
      metrics = ctx.measureText("Lordran gypsum")
      expect(metrics.alphabeticBaseline).toBeGreaterThan(0)
      expect(metrics.actualBoundingBoxAscent).toBeGreaterThan(0)
      expect(metrics.actualBoundingBoxDescent).toBeLessThan(0)
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
            parsed = parseFont(font);
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

      // case-insensitive (#235)
      ctx.fillStyle = "sILveR";
      expect(ctx.fillStyle).toBe("#c0c0c0");
    });
  })
})

describe("FontLibrary", ()=>{
  const findFont = font => `${__dirname}/assets/${font}`

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
      expect(info).toHaveProperty('family')
      expect(info).toHaveProperty('weights')
      expect(typeof info.weights[0]).toBe('number');
      expect(info).toHaveProperty('widths')
      expect(typeof info.widths[0]).toBe('string');
      expect(info).toHaveProperty('styles')
      expect(typeof info.styles[0]).toBe('string');
    }
  })

  test("can register fonts", ()=>{
    let ttf = findFont("AmstelvarAlpha-VF.ttf"),
        name = "AmstelvarAlpha",
        alias = "PseudonymousBosch";

    expect(() => FontLibrary.use(ttf)).not.toThrow()
    expect(FontLibrary.has(name)).toBe(true)
    expect(FontLibrary.family(name).weights).toContain(400)

    expect(FontLibrary.has(alias)).toBe(false)
    expect(() => FontLibrary.use(alias, ttf)).not.toThrow()
    expect(FontLibrary.has(alias)).toBe(true)
    expect(FontLibrary.family(alias).weights).toContain(400)
  })
})

