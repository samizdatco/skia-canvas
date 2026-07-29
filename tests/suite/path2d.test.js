// @ts-check

"use strict"

const {assert, describe, test, beforeEach, afterEach} = require('../runner'),
      {Canvas, DOMMatrix, Path2D, DOMPoint} = require('../../lib');

const BLACK = [0,0,0,255],
      WHITE = [255,255,255,255],
      CLEAR = [0,0,0,0],
      TAU = Math.PI * 2

describe("Path2D", ()=>{
  let canvas, ctx,
      WIDTH = 512, HEIGHT = 512,
      pixel = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data),
      scrub = () => ctx.clearRect(0,0,WIDTH,HEIGHT),
      p;

  beforeEach(()=>{
    canvas = new Canvas(WIDTH, HEIGHT)
    ctx = canvas.getContext("2d")
    ctx.lineWidth = 4
    p = new Path2D()
  })

  describe("can be initialized with", ()=>{
    test('no arguments', () => {
      let p1 = new Path2D()
      p1.rect(10, 10, 100, 100)
    })

    test('another Path2D', () => {
      let p1 = new Path2D()
      p1.rect(10, 10, 100, 100)
      let p2 = new Path2D(p1)
      assert.matchesSubset(p1.bounds, p2.bounds)
    })

    test('an SVG string', () => {
      let p1 = new Path2D()
      p1.rect(10, 10, 100, 100)
      let p2 = new Path2D("M 10,10 h 100 v 100 h -100 Z")
      assert.matchesSubset(p1.bounds, p2.bounds)
    })

    test('a stream of edges', () => {
      let p = new Path2D()

      p.moveTo(100, 100)
      p.lineTo(200, 100)
      p.lineTo(200, 200)
      p.lineTo(100, 200)
      p.closePath()
      p.moveTo(250, 200)
      p.arc(200, 200, 50, 0, TAU)
      p.moveTo(300, 100)
      p.bezierCurveTo(400, 100, 300, 200, 400, 200)
      p.moveTo(400,220)
      p.quadraticCurveTo(400, 320, 300, 320)

      let clone = new Path2D()
      for (const [verb, ...pts] of p.edges){
        clone[verb](...pts)
      }

      ctx.fillStyle = 'white'
      ctx.fillRect(0, 0, WIDTH, HEIGHT)

      ctx.lineWidth = 1
      ctx.stroke(p)
      let pixels = ctx.getImageData(0, 0, WIDTH, HEIGHT)
      assert.deepEqual(pixels.data.every(px => px==255), false)

      ctx.lineWidth = 4
      ctx.strokeStyle = 'white'
      ctx.stroke(clone)
      pixels = ctx.getImageData(0, 0, WIDTH, HEIGHT)
      assert.deepEqual(pixels.data.every(px => px==255), true)
    })
  })

  describe("can get & set", () => {
    test("d", () => {
      p.moveTo(10, 10)
      p.lineTo(100, 40)
      p.quadraticCurveTo(150, 200, 100, 300)
      p.bezierCurveTo(60, 350, 300, 380, 380, 300)
      p.closePath()

      // the getter's SVG string losslessly reconstructs the path
      let clone = new Path2D(p.d)
      assert.equal(clone.d, p.d)
      assert.deepEqual(clone.edges, p.edges)
      assert.matchesSubset(clone.bounds, p.bounds)

      // the setter replaces the path's previous contents
      p.d = "M 50 50 h 100 v 100 h -100 Z"
      assert.matchesSubset(p.bounds, {left:50, top:50, right:150, bottom:150})
      ctx.fill(p)
      assert.deepEqual(pixel(100, 100), BLACK)
      assert.deepEqual(pixel(30, 30), CLEAR)
    })
  })

  describe("can use verb", () => {
    test("moveTo", () => {
      let [left, top] = [20, 30]
      p.moveTo(left, top)
      assert.matchesSubset(p.bounds, {left, top})
      assert.throws(() => p.moveTo(120) , /not enough arguments/)
    })

    test("lineTo", () => {
      let [left, top] = [20, 30],
          [width, height] = [37, 86]
      p.moveTo(left, top)
      p.lineTo(left+width, top+height)
      ctx.stroke(p)
      assert.matchesSubset(p.bounds, {left, top, width, height})
      assert.deepEqual(pixel(left+width/2, top+height/2), BLACK)
      assert.throws(() => p.lineTo(120) , /not enough arguments/)
    })

    test("bezierCurveTo", () => {
      p.moveTo(20,100)
      p.bezierCurveTo(120,-100, 120,300, 220,100)
      ctx.lineWidth = 6
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      assert.deepEqual(pixel(71, 42), BLACK)
      assert.deepEqual(pixel(168, 157), BLACK)
      assert.throws(() => p.bezierCurveTo(120, 300, 400, 400) , /not enough arguments/)
      assert.doesNotThrow(() => p.bezierCurveTo(120, 300, null, 'foo', NaN, 400) )
    })

    test("quadraticCurveTo", () => {
      p.moveTo(20,100)
      p.quadraticCurveTo(120,300, 220,100)
      ctx.lineWidth = 6
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      assert.deepEqual(pixel(120, 199), BLACK)
      assert.throws(() => p.quadraticCurveTo(120, 300) , /not enough arguments/)
      assert.doesNotThrow(() => p.quadraticCurveTo(NaN, 300, null, 'foo') )
    })

    test("conicTo", () => {
      ctx.lineWidth = 5

      let withWeight = weight => {
        let path = new Path2D()
        path.moveTo(100,400)
        path.conicCurveTo(250, 50, 400, 400, weight)
        return path
      }

      ctx.stroke(withWeight(0))
      assert.deepEqual(pixel(250, 400), BLACK)
      scrub()

      ctx.stroke(withWeight(1))
      assert.deepEqual(pixel(250, 225), BLACK)
      scrub()

      ctx.stroke(withWeight(10))
      assert.deepEqual(pixel(250, 81), BLACK)
      scrub()

      ctx.stroke(withWeight(100))
      assert.deepEqual(pixel(250, 54), BLACK)
      scrub()

      ctx.stroke(withWeight(1000))
      assert.deepEqual(pixel(250, 50), BLACK)
      scrub()
    })

    test("arcTo", () => {
      p.moveTo(100, 100)
      p.arcTo(150, 5, 200, 100, 25)
      p.lineTo(200, 100)
      p.moveTo(100, 100)
      p.arcTo(150, 200, 200, 100, 50)
      p.lineTo(200, 100)
      ctx.lineWidth = 6
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      assert.deepEqual(pixel(150, 137), BLACK)
      assert.deepEqual(pixel(150, 33), BLACK)

      // collinear points degenerate to a straight line ending at the control point
      let flat = new Path2D()
      flat.moveTo(50, 300)
      flat.arcTo(200, 300, 350, 300, 60)
      assert.deepEqual(flat.edges, [["moveTo", 50, 300], ["lineTo", 200, 300]])
      ctx.stroke(flat)
      assert.deepEqual(pixel(125, 300), BLACK)

      assert.throws(() => p.arcTo(0,0, 20,20) , /not enough arguments/)
      assert.doesNotThrow(() => p.arcTo(150, 5, null, 'foo', NaN) )
    })

    test("rect", () => {
      p.rect(50,50,100,100)
      ctx.lineWidth = 6
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      assert.deepEqual(pixel(150, 150), BLACK)

      // negative dimensions are normalized
      scrub()
      let neg = new Path2D()
      neg.rect(300, 300, -200, -200)
      assert.matchesSubset(neg.bounds, {left:100, top:100, right:300, bottom:300})
      ctx.fill(neg)
      assert.deepEqual(pixel(200, 200), BLACK)
      assert.deepEqual(pixel(50, 50), CLEAR)

      assert.throws(() => p.rect(0,0, 20) , /not enough arguments/)
    })

    test("roundRect", () => {
      let dim = WIDTH/2
      let radii = [50, 25, 15, new DOMPoint(20, 10)]
      p.roundRect(dim, dim, dim, dim, radii)
      p.roundRect(dim, dim, -dim, -dim, radii)
      p.roundRect(dim, dim, -dim, dim, radii)
      p.roundRect(dim, dim, dim, -dim, radii)
      ctx.fill(p)

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

      // an omitted radius defaults to a 0-radius (square-cornered) rect
      let sq = new Path2D(); sq.roundRect(10, 10, 50, 50)
      assert.ok(sq.d.length > 0, `expected non-empty d, got "${sq.d}"`)
      assert.deepEqual([sq.bounds.left, sq.bounds.top, sq.bounds.right, sq.bounds.bottom], [10, 10, 60, 60])
    })

    describe("winds shapes to match the browser", ()=>{
      // Regression guards for the rect/roundRect corner-ordering fixes. These shapes fill
      // identically no matter where their closed contour starts or which way it winds, so the
      // divergences we fixed are invisible to a plain fill — they only surface in dash phase,
      // the current point after the verb, and nonzero-winding overlaps. Assert the raw edge
      // list (and cross-check that Context2D delegates to the same builders).

      test("rect() follows the spec's fixed corner order", ()=>{
        // moveTo(x,y) → (x+w,y) → (x+w,y+h) → (x,y+h) → close, whatever the sign of w/h
        let pos = new Path2D(); pos.rect(110, 110, 90, 70)
        assert.deepEqual(pos.edges, [
          ["moveTo", 110, 110], ["lineTo", 200, 110], ["lineTo", 200, 180],
          ["lineTo", 110, 180], ["lineTo", 110, 110], ["closePath"],
        ])

        // mixed-sign dims must wind the SAME direction — add_rect's normalized winding used to
        // reverse the traversal for these
        let negW = new Path2D(); negW.rect(110, 110, -90, 70)
        assert.deepEqual(negW.edges, [
          ["moveTo", 110, 110], ["lineTo", 20, 110], ["lineTo", 20, 180],
          ["lineTo", 110, 180], ["lineTo", 110, 110], ["closePath"],
        ])

        let negH = new Path2D(); negH.rect(110, 110, 90, -70)
        assert.deepEqual(negH.edges, [
          ["moveTo", 110, 110], ["lineTo", 200, 110], ["lineTo", 200, 40],
          ["lineTo", 110, 40], ["lineTo", 110, 110], ["closePath"],
        ])
      })

      test("roundRect() starts at the top-left corner, going clockwise", ()=>{
        // browser origin is (x + topLeftRadius, y) with the first edge running along the top
        // toward the top-right corner. (Skia's default rrect start index 6/7 begins at the
        // bottom-left instead.)
        let rr = new Path2D(); rr.roundRect(10, 20, 100, 80, 15)
        let [move, line] = rr.edges
        assert.deepEqual(move, ["moveTo", 25, 20])   // (x + rTL, y)
        assert.deepEqual(line, ["lineTo", 95, 20])   // top edge → (x + w − rTR, y)
      })

      test("Context2D rect/roundRect render identically to the Path2D builders", ()=>{
        // ctx.rect/roundRect delegate to these builders. Preceded by real geometry and drawn as
        // a dashed stroke, any divergence in start vertex, winding, or new-subpath-vs-connecting-
        // line shows up as differing pixels (a plain fill would hide all three).
        let render = (target, build) => {
          scrub(); ctx.save(); ctx.setLineDash([9, 9])
          if (target == 'ctx'){ ctx.beginPath(); build(ctx); ctx.stroke() }
          else { let q = new Path2D(); build(q); ctx.stroke(q) }
          ctx.restore()
          return ctx.getImageData(0, 0, WIDTH, HEIGHT).data
        }
        let diff = (a, b) => { let n = 0; for (let i=0; i<a.length; i++) if (a[i] !== b[i]) n++; return n }

        let cases = {
          "rect":              o => { o.moveTo(20, 20); o.lineTo(40, 40); o.rect(80, 80, 200, 150) },
          "rect (mixed-sign)": o => { o.moveTo(20, 20); o.lineTo(40, 40); o.rect(280, 280, -200, 150) },
          "roundRect":         o => { o.moveTo(20, 20); o.lineTo(40, 40); o.roundRect(80, 320, 200, 150, 25) },
        }
        for (let [name, build] of Object.entries(cases)){
          assert.equal(diff(render('ctx', build), render('path', build)), 0, `${name}: ctx vs Path2D mismatch`)
        }
      })
    })

    test("arcTo transforms its radius with the CTM", ()=>{
      // arcTo's radius must scale with the context transform — a device-space circular arc with a
      // fixed radius wouldn't. ctx.arcTo bakes the CTM as it builds; drawing the same Path2D
      // through the transformed context is the browser reference. Under a scale they must match
      // (before the fix ctx kept the radius in device space and diverged by thousands of pixels).
      let render = (target) => {
        scrub(); ctx.save(); ctx.translate(30, 20); ctx.scale(1.6, 1.6)
        let build = o => { o.moveTo(40, 40); o.arcTo(160, 40, 160, 160, 50); o.lineTo(40, 160) }
        if (target == 'ctx'){ ctx.beginPath(); build(ctx); ctx.fillStyle = 'black'; ctx.fill() }
        else { let q = new Path2D(); build(q); ctx.fillStyle = 'black'; ctx.fill(q) }
        ctx.restore()
        return ctx.getImageData(0, 0, WIDTH, HEIGHT).data
      }
      let diff = (a, b) => { let n = 0; for (let i=0; i<a.length; i++) if (a[i] !== b[i]) n++; return n }
      assert.equal(diff(render('ctx'), render('path')), 0)
    })

    test("arc", () => {
      p.arc(150, 150, 75, Math.PI/8, Math.PI*1.5, true)
      ctx.fillStyle = 'black'
      ctx.fill(p)

      p = new Path2D()
      p.arc(150, 150, 75, Math.PI/8, Math.PI*1.5, false)
      ctx.fillStyle = 'white'
      ctx.fill(p)

      assert.deepEqual(pixel(196, 112), BLACK)
      assert.deepEqual(pixel(150, 150), WHITE)
      assert.throws(() => p.arc(150, 150, 75, Math.PI/8) , /not enough arguments/)
      assert.doesNotThrow(() => p.arc(150, 150, 75, Math.PI/8, Math.PI*1.5) )
    })

    test("arc (f64 angle precision)", () => {
      // startAngle/endAngle used to be narrowed to f32 so very large angles lost precision and
      // the arc began in the wrong place. compare the new (accurate) calculation to the old
      const cx = 150, cy = 150, r = 75

      const arcStart = a => {
        let path = new Path2D()
        path.arc(cx, cy, r, a, a + 0.5, false)
        let [verb, x, y] = path.edges[0]
        assert.equal(verb, "moveTo")
        return [x, y]
      }

      // ensure the starting point is in the right location
      for (const a of [1e4 + 0.3, 1e6 + 0.123, 1e7 + 1.5, 1e8 + 0.7]){
        let [x, y] = arcStart(a)
        assert.nearEqual(x, cx + r * Math.cos(a))
        assert.nearEqual(y, cy + r * Math.sin(a))
      }

      // ensure it's not where the f32 math would put it
      for (const a of [1e7 + 1.5, 1e8 + 0.7]){
        let [x, y] = arcStart(a)
        let af = Math.fround(a)
        let f32x = cx + r * Math.cos(af), f32y = cy + r * Math.sin(af)
        assert.ok(Math.hypot(f32x - x, f32y - y) > 25,
          `f32-narrowed angle ${a} would misplace the arc start by tens of px`)
      }
    })

    test("ellipse", () => {
      // default to clockwise
      p.ellipse(100,100, 100, 50, .25*Math.PI, 0, 1.5*Math.PI)
      ctx.lineWidth = 5
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      assert.deepEqual(pixel(127, 175), BLACK)
      assert.deepEqual(pixel(130, 60), BLACK)
      assert.deepEqual(pixel(163, 100), CLEAR)

      // with ccw enabled
      let p2 = new Path2D()
      p2.ellipse(100,100, 100, 50, .25*Math.PI, 0, 1.5*Math.PI, true)
      ctx.clearRect(0,0, WIDTH, HEIGHT)
      ctx.stroke(p2)

      assert.deepEqual(pixel(127, 175), CLEAR)
      assert.deepEqual(pixel(130, 60), CLEAR)
      assert.deepEqual(pixel(163, 100), BLACK)

      // full ellipse from offset angles, clockwise
      p.ellipse(100,100, 100, 50, .25*Math.PI, -.5*Math.PI, 1.5*Math.PI, false)
      ctx.lineWidth = 5
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      assert.deepEqual(pixel(127, 175), BLACK)
      assert.deepEqual(pixel(130, 60), BLACK)
      assert.deepEqual(pixel(163, 100), BLACK)

      // full ellipse from offset angles, CCW
      p.ellipse(100,100, 100, 50, .25*Math.PI, -.5*Math.PI, 1.5*Math.PI, true)
      ctx.lineWidth = 5
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      assert.deepEqual(pixel(127, 175), BLACK)
      assert.deepEqual(pixel(130, 60), BLACK)
      assert.deepEqual(pixel(163, 100), BLACK)

    })
  })

  describe("can append", () => {
    test("other paths", () => {
      let left = new Path2D(),
          right = new Path2D();
      left.moveTo(20, 20)
      left.lineTo(100, 100)
      assert.matchesSubset(left.bounds, { left: 20, top: 20, right: 100, bottom: 100 })

      right.moveTo(200, 20)
      right.lineTo(200, 200)
      assert.matchesSubset(right.bounds, { left: 200, top: 20, right: 200, bottom: 200 })

      left.addPath(right)
      assert.matchesSubset(left.bounds, { left: 20, top: 20, right: 200, bottom: 200 })
    })

    test("with a transform matrix", () => {
      let left = new Path2D()
      left.moveTo(0, 0)
      left.lineTo(10, 10)
      assert.matchesSubset(left.bounds,  { left: 0, top: 0, right: 10, bottom: 10 } )

      let right = new Path2D(left)
      assert.matchesSubset(right.bounds,  { left: 0, top: 0, right: 10, bottom: 10 } )

      let matrix = new DOMMatrix().scale(10,10)
      left.addPath(right, matrix)
      assert.matchesSubset(left.bounds,  { left: 0, top: 0, right: 100, bottom: 100 } )
    })

    test("to a closed path", () => {
      ctx.lineWidth = 5
      ctx.strokeStyle = 'black'

      let left = new Path2D()
      left.arc(100, 100, 25, 0, 2*Math.PI)
      assert.matchesSubset(left.bounds,  { left: 75, top: 75, right: 125, bottom: 125 } )

      let right = new Path2D()
      right.arc(200, 100, 25, 0, 2*Math.PI)
      assert.matchesSubset(right.bounds,  { left: 175, top: 75, right: 225, bottom: 125 } )

      left.addPath(right)
      assert.matchesSubset(left.bounds,  { left: 75, top: 75, right: 225, bottom: 125 } )

      // adding creates a path with two separate circles
      ctx.stroke(left)
      assert.deepEqual(pixel(175, 100), BLACK)
      assert.deepEqual(pixel(150, 100), CLEAR)

      // two .arc calls in one path draws a line connecting them
      let solo = new Path2D()
      solo.arc(100, 250, 25, 0, 2*Math.PI)
      solo.arc(200, 250, 25, 0, 2*Math.PI)
      ctx.stroke(solo)
      assert.deepEqual(pixel(175, 250), BLACK)
      assert.deepEqual(pixel(150, 250), BLACK)
    })

    test("self", () => {
      let p = new Path2D()
      p.ellipse(150, 150, 75, 75, 0, Math.PI, Math.PI*2, true)
      p.addPath(p, new DOMMatrix().scale(2,2))
      ctx.fillStyle = 'black'
      ctx.fill(p)

      assert.deepEqual(pixel(150, 151), BLACK)
      assert.deepEqual(pixel(150, 223), BLACK)
      assert.deepEqual(pixel(300, 301), BLACK)
      assert.deepEqual(pixel(300, 448), BLACK)
    })

    test("convex contours", () => {
      // skia's add_path append fast-path can leave the destination's cached
      // convexity stale, so append_path replays verbs instead — both contours
      // must fill even when the first was already drawn (and its convexity cached)
      let base = new Path2D()
      base.arc(100, 100, 50, 0, TAU)
      ctx.fillStyle = 'black'
      ctx.fill(base)

      let extra = new Path2D()
      extra.arc(300, 300, 50, 0, TAU)
      base.addPath(extra)

      scrub()
      ctx.fill(base)
      assert.deepEqual(pixel(100, 100), BLACK)
      assert.deepEqual(pixel(300, 300), BLACK)
      assert.deepEqual(pixel(200, 200), CLEAR)
    })

  })

  describe("can combine paths using", () => {
    let a, b,
        top = () => pixel(60, 20),
        left = () => pixel(20, 60),
        center = () => pixel(60, 60),
        right = () => pixel(100, 60),
        bottom = () => pixel(60, 100)

    beforeEach(()=>{
      a = new Path2D("M 10,50 h 100 v 20 h -100 Z")
      b = new Path2D("M 50,10 h 20 v100 h -20 Z")
      ctx.fillStyle = 'black'
    })

    test("complement", () => {
      let c = a.complement(b)
      ctx.fill(c)
      assert.deepEqual(top(), BLACK)
      assert.deepEqual(left(), CLEAR)
      assert.deepEqual(center(), CLEAR)
      assert.deepEqual(right(), CLEAR)
      assert.deepEqual(bottom(), BLACK)
    })

    test("difference", () => {
      let c = a.difference(b)
      ctx.fill(c)
      assert.deepEqual(top(), CLEAR)
      assert.deepEqual(left(), BLACK)
      assert.deepEqual(center(), CLEAR)
      assert.deepEqual(right(), BLACK)
      assert.deepEqual(bottom(), CLEAR)
    })

    test("intersect", () => {
      let c = a.intersect(b)
      ctx.fill(c)
      assert.deepEqual(top(), CLEAR)
      assert.deepEqual(left(), CLEAR)
      assert.deepEqual(center(), BLACK)
      assert.deepEqual(right(), CLEAR)
      assert.deepEqual(bottom(), CLEAR)
    })

    test("union", () => {
      let c = a.union(b)
      ctx.fill(c)
      assert.deepEqual(top(), BLACK)
      assert.deepEqual(left(), BLACK)
      assert.deepEqual(center(), BLACK)
      assert.deepEqual(right(), BLACK)
      assert.deepEqual(bottom(), BLACK)
    })

    test("xor", () => {
      let c = a.xor(b)
      ctx.fill(c, 'evenodd')
      assert.deepEqual(top(), BLACK)
      assert.deepEqual(left(), BLACK)
      assert.deepEqual(center(), CLEAR)
      assert.deepEqual(right(), BLACK)
      assert.deepEqual(bottom(), BLACK)

      ctx.fill(c, 'nonzero')
      assert.deepEqual(center(), BLACK)
    })

    test("simplify", () => {
      let c = a.xor(b)
      ctx.fill(c.simplify('evenodd'))
      assert.deepEqual(top(), BLACK)
      assert.deepEqual(left(), BLACK)
      assert.deepEqual(center(), CLEAR)
      assert.deepEqual(right(), BLACK)
      assert.deepEqual(bottom(), BLACK)

      ctx.fill(c.simplify())
      assert.deepEqual(center(), BLACK)
    })

    test("unwind", () => {
      let d = new Path2D()
      d.rect(0,0,30,30)
      d.rect(10,10,10,10)
      ctx.fill(d.offset(50,40))
      assert.deepEqual(pixel(65, 55), BLACK)
      ctx.fill(d.offset(100,40), 'evenodd')
      assert.deepEqual(pixel(115, 55), CLEAR)
      ctx.fill(d.simplify().offset(150,40), 'evenodd')
      assert.deepEqual(pixel(165, 55), BLACK)
      ctx.fill(d.unwind().offset(200,40))
      assert.deepEqual(pixel(215, 55), CLEAR)
    })

    test("interpolate", () => {
      let start = new Path2D()
      start.moveTo(100, 100)
      start.bezierCurveTo(100, 100, 100, 200, 100, 200)
      start.bezierCurveTo(100, 200, 100, 300, 100, 300)

      let finish = new Path2D()
      finish.moveTo(300, 100)
      finish.bezierCurveTo(400, 100, 300, 200, 400, 200)
      finish.bezierCurveTo(300, 200, 400, 300, 300, 300)

      ctx.lineWidth = 4

      ctx.stroke(start.interpolate(finish, 0))
      assert.deepEqual(pixel(100, 102), BLACK)
      assert.deepEqual(pixel(100, 200), BLACK)
      scrub()

      ctx.stroke(start.interpolate(finish, .25))
      assert.deepEqual(pixel(151, 101), BLACK)
      assert.deepEqual(pixel(151, 200), CLEAR)
      assert.deepEqual(pixel(171, 200), BLACK)
      scrub()

      ctx.stroke(start.interpolate(finish, .5))
      assert.deepEqual(pixel(201, 101), BLACK)
      assert.deepEqual(pixel(201, 200), CLEAR)
      assert.deepEqual(pixel(243, 200), BLACK)
      scrub()

      ctx.stroke(start.interpolate(finish, .75))
      assert.deepEqual(pixel(251, 101), BLACK)
      assert.deepEqual(pixel(251, 200), CLEAR)
      assert.deepEqual(pixel(322, 200), BLACK)
      scrub()

      ctx.stroke(start.interpolate(finish, 1))
      assert.deepEqual(pixel(301, 101), BLACK)
      assert.deepEqual(pixel(301, 200), CLEAR)
      assert.deepEqual(pixel(395, 200), BLACK)
      scrub()

      ctx.stroke(start.interpolate(finish, 1.25))
      assert.deepEqual(pixel(351, 101), BLACK)
      assert.deepEqual(pixel(351, 200), CLEAR)
      assert.deepEqual(pixel(470, 200), BLACK)
      scrub()

    })

  })

  describe("can apply path effect", () => {

    test("jitter", () => {
      let rng = [...Array(99).keys()].map(k => k + 101)
      let blackPixel = BLACK.toString()

      let line = new Path2D()
      line.moveTo(100, 100)
      line.lineTo(100, 200)

      ctx.lineWidth = 4
      ctx.stroke(line)
      let allBlack = rng.map(y => pixel(100, y).toString() == blackPixel)
      assert.doesNotContain(allBlack, false)
      scrub()

      let zag = line.jitter(10, 20)
      ctx.stroke(zag)
      let notAllBlack = rng.map(y => pixel(100, y).toString() == blackPixel)
      assert.contains(notAllBlack, false)
      assert.contains(notAllBlack, true)
    })

    test("round", () => {
      // hit by both
      let alpha = () => pixel(50, 220),
          omega = () => pixel(300, 30)

      // hit by un-rounded lines
      let topLeft = () => pixel(100, 30),
          topRight = () => pixel(200, 30),
          botLeft = () => pixel(150, 220),
          botRight = () => pixel(250, 220)

      // hit by rounded lines
      let hiLeft = () => pixel(100, 64),
          hiRight = () => pixel(200, 64),
          loLeft = () => pixel(150, 186),
          loRight = () => pixel(250, 186)

      let lines = new Path2D()
      lines.moveTo(50, 225)
      lines.lineTo(100, 25)
      lines.lineTo(150, 225)
      lines.lineTo(200, 25)
      lines.lineTo(250, 225)
      lines.lineTo(300, 25)

      ctx.lineWidth = 10
      ctx.stroke(lines)
      assert.deepEqual(alpha(), BLACK)
      assert.deepEqual(omega(), BLACK)

      assert.deepEqual(topLeft(), BLACK)
      assert.deepEqual(topRight(), BLACK)
      assert.deepEqual(botLeft(), BLACK)
      assert.deepEqual(botRight(), BLACK)

      assert.deepEqual(hiLeft(), CLEAR)
      assert.deepEqual(hiRight(), CLEAR)
      assert.deepEqual(loLeft(), CLEAR)
      assert.deepEqual(loRight(), CLEAR)

      let rounded = lines.round(80)
      canvas.width = WIDTH
      ctx.lineWidth = 10
      ctx.stroke(rounded)
      assert.deepEqual(alpha(), BLACK)
      assert.deepEqual(omega(), BLACK)

      assert.deepEqual(topLeft(), CLEAR)
      assert.deepEqual(topRight(), CLEAR)
      assert.deepEqual(botLeft(), CLEAR)
      assert.deepEqual(botRight(), CLEAR)

      assert.deepEqual(hiLeft(), BLACK)
      assert.deepEqual(hiRight(), BLACK)
      assert.deepEqual(loLeft(), BLACK)
      assert.deepEqual(loRight(), BLACK)
    })

    test("offset", () => {
      let orig = new Path2D()
      orig.rect(10, 10, 40, 40)
      assert.matchesSubset(orig.bounds, {left:10, top:10, right:50, bottom:50})

      let shifted = orig.offset(-10, -10)
      assert.matchesSubset(shifted.bounds, {left:0, top:0, right:40, bottom:40})

      shifted = shifted.offset(-40, -40)
      assert.matchesSubset(shifted.bounds, {left:-40, top:-40, right:0, bottom:0})

      // orig path should be unchanged
      assert.matchesSubset(orig.bounds, {left:10, top:10, right:50, bottom:50})
    })

    test("transform", () => {
      let orig = new Path2D()
      orig.rect(-10, -10, 20, 20)
      assert.matchesSubset(orig.bounds, {left:-10, top:-10, right:10, bottom:10})

      let shifted = orig.transform(new DOMMatrix().translate(10, 10))
      assert.matchesSubset(shifted.bounds, {left:0, top:0, right:20, bottom:20})

      let shiftedByHand = orig.transform(1, 0, 0, 1, 10, 10)
      assert.deepEqual(shifted.edges, shiftedByHand.edges)

      let embiggened = orig.transform(new DOMMatrix().scale(2, .5)),
          bigBounds = embiggened.bounds,
          origBounds = orig.bounds
      assert(bigBounds.left < origBounds.left)
      assert(bigBounds.right > origBounds.right)

      // orig path should be unchanged
      assert.matchesSubset(orig.bounds, {left:-10, top:-10, right:10, bottom:10})
    })

    test("trim", () => {
      let left = () => pixel(64, 137),
          mid = () => pixel(200, 50),
          right = () => pixel(336, 137)

      let orig = new Path2D()
      orig.arc(200, 200, 150, Math.PI, 0)

      let middle = orig.trim(.25, .75),
          endpoints = orig.trim(.25, .75, true),
          start = orig.trim(.25),
          end = orig.trim(-.25),
          none = orig.trim(.75, .25),
          everythingAndMore = orig.trim(-12345, 98765)

      ctx.lineWidth = 10
      ctx.stroke(orig)
      assert.deepEqual(left(), BLACK)
      assert.deepEqual(mid(), BLACK)
      assert.deepEqual(right(), BLACK)
      scrub()

      ctx.stroke(middle)
      assert.deepEqual(left(), CLEAR)
      assert.deepEqual(mid(), BLACK)
      assert.deepEqual(right(), CLEAR)
      scrub()

      ctx.stroke(endpoints)
      assert.deepEqual(left(), BLACK)
      assert.deepEqual(mid(), CLEAR)
      assert.deepEqual(right(), BLACK)
      scrub()

      ctx.stroke(start)
      assert.deepEqual(left(), BLACK)
      assert.deepEqual(mid(), CLEAR)
      assert.deepEqual(right(), CLEAR)
      scrub()

      ctx.stroke(end)
      assert.deepEqual(left(), CLEAR)
      assert.deepEqual(mid(), CLEAR)
      assert.deepEqual(right(), BLACK)
      scrub()

      ctx.stroke(none)
      assert.deepEqual(left(), CLEAR)
      assert.deepEqual(mid(), CLEAR)
      assert.deepEqual(right(), CLEAR)
      scrub()

      ctx.stroke(everythingAndMore)
      assert.deepEqual(left(), BLACK)
      assert.deepEqual(mid(), BLACK)
      assert.deepEqual(right(), BLACK)
      scrub()
    })
  })

  describe("validates", () => {
    test('not enough arguments', async () => {
      let ERR =  /not enough arguments/
      assert.throws(() => p.transform(), ERR)
      assert.throws(() => p.transform(0,0,0,0,0), ERR)
      assert.throws(() => p.rect(0,0,0), ERR)
      assert.throws(() => p.roundRect(0,0,0), ERR)
      assert.throws(() => p.arc(0,0,0,0), ERR)
      assert.throws(() => p.arcTo(0,0,0,0), ERR)
      assert.throws(() => p.ellipse(0,0,0,0,0,0), ERR)
      assert.throws(() => p.moveTo(0), ERR)
      assert.throws(() => p.lineTo(0), ERR)
      assert.throws(() => p.bezierCurveTo(0,0,0,0,0), ERR)
      assert.throws(() => p.quadraticCurveTo(0,0,0), ERR)
      assert.throws(() => p.conicCurveTo(0,0,0,0), ERR)
      assert.throws(() => p.complement(), ERR)
      assert.throws(() => p.interpolate(), ERR)
      assert.throws(() => p.offset(0), ERR)
      assert.throws(() => p.round(), ERR)
      assert.throws(() => p.contains(0), ERR)
      assert.throws(() => p.addPath(), ERR)
    })

    test('value errors', async () => {
      assert.throws(() => p.transform(0,0,0,NaN,0,0), /Expected a DOMMatrix/)
      assert.throws(() => p.complement({}), /Expected a Path2D/)
      assert.throws(() => p.interpolate(p), /Expected a number/)
      assert.throws(() => p.arc(0,0,-10,0,0), {name:'IndexSizeError'})
      assert.throws(() => p.arcTo(0,0,0,0,-10), {name:'IndexSizeError'})
      assert.throws(() => p.ellipse(0,0,-10,-10,0,0,0), {name:'IndexSizeError'})
      assert.throws(() => p.roundRect(0,0,0,0,-10), /Corner radius cannot be negative/)
      assert.throws(() => p.addPath(p, []), /Invalid transform matrix/)
    })

    test('NaN arguments', async () => {
      assert.doesNotThrow(() => p.rect(0,0,NaN,0))
      assert.doesNotThrow(() => p.arc(0,0,NaN,0,0))
      assert.doesNotThrow(() => p.arc(0,0,NaN,0,0,false))
      assert.doesNotThrow(() => p.arc(0,0,NaN,0,0,new Date()))
      assert.doesNotThrow(() => p.ellipse(0,0,0,NaN,0,0,0))
      assert.doesNotThrow(() => p.moveTo(NaN,0))
      assert.doesNotThrow(() => p.lineTo(NaN,0))
      assert.doesNotThrow(() => p.arcTo(0,0,0,0,NaN))
      assert.doesNotThrow(() => p.bezierCurveTo(0,0,0,0,NaN,0))
      assert.doesNotThrow(() => p.quadraticCurveTo(0,0,NaN,0))
      assert.doesNotThrow(() => p.conicCurveTo(0,0,NaN,0,1))
      assert.doesNotThrow(() => p.roundRect(0,0,0,0,NaN))
      assert.doesNotThrow(() => p.roundRect(NaN,0,0,0,5))
      assert.doesNotThrow(() => p.roundRect(0,0,NaN,0,5))
      assert.doesNotThrow(() => p.transform({}))
    })
  })
})
