const _ = require('lodash'),
      {Canvas, DOMMatrix, Path2D} = require('../lib');

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
    ctx.lineStyle = 'black'
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
      expect(p1.bounds).toMatchObject(p2.bounds)
    })

    test('an SVG string', () => {
      let p1 = new Path2D()
      p1.rect(10, 10, 100, 100)
      let p2 = new Path2D("M 10,10 h 100 v 100 h -100 Z")
      expect(p1.bounds).toMatchObject(p2.bounds)
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
      expect(pixels.data.every(px => px==255)).toBe(false)

      ctx.lineWidth = 4
      ctx.strokeStyle = 'white'
      ctx.stroke(clone)
      pixels = ctx.getImageData(0, 0, WIDTH, HEIGHT)
      expect(pixels.data.every(px => px==255)).toBe(true)
    })
  })

  describe("can use verb", () => {
    test("moveTo", () => {
      let [left, top] = [20, 30]
      p.moveTo(left, top)
      expect(p.bounds).toMatchObject({left, top})
      expect(() => p.moveTo(120) ).toThrowError("must be a number")
    })

    test("lineTo", () => {
      let [left, top] = [20, 30],
          [width, height] = [37, 86]
      p.moveTo(left, top)
      p.lineTo(left+width, top+height)
      ctx.stroke(p)
      expect(p.bounds).toMatchObject({left, top, width, height})
      expect(pixel(left+width/2, top+height/2)).toEqual(BLACK)
      expect(() => p.lineTo(120) ).toThrowError("must be a number")
    })

    test("bezierCurveTo", () => {
      p.moveTo(20,100)
      p.bezierCurveTo(120,-100, 120,300, 220,100)
      ctx.lineWidth = 6
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      expect(pixel(71, 42)).toEqual(BLACK)
      expect(pixel(168, 157)).toEqual(BLACK)
      expect(() => p.bezierCurveTo(120, 300, 400, 400) ).toThrowError("Not enough arguments")
      expect(() => p.bezierCurveTo(120, 300, null, 'foo', 400, 400) ).toThrowError("Not enough arguments")
    })

    test("quadraticCurveTo", () => {
      p.moveTo(20,100)
      p.quadraticCurveTo(120,300, 220,100)
      ctx.lineWidth = 6
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      expect(pixel(120, 199)).toEqual(BLACK)
      expect(() => p.quadraticCurveTo(120, 300) ).toThrowError("Not enough arguments")
      expect(() => p.quadraticCurveTo(120, 300, null, 'foo') ).toThrowError("Not enough arguments")
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
      expect(pixel(250, 400)).toEqual(BLACK)
      scrub()

      ctx.stroke(withWeight(1))
      expect(pixel(250, 225)).toEqual(BLACK)
      scrub()

      ctx.stroke(withWeight(10))
      expect(pixel(250, 81)).toEqual(BLACK)
      scrub()

      ctx.stroke(withWeight(100))
      expect(pixel(250, 54)).toEqual(BLACK)
      scrub()

      ctx.stroke(withWeight(1000))
      expect(pixel(250, 50)).toEqual(BLACK)
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

      expect(pixel(150, 137)).toEqual(BLACK)
      expect(pixel(150, 33)).toEqual(BLACK)
      expect(() => p.arcTo(0,0, 20,20) ).toThrowError("Missing argument")
      expect(() => p.arcTo(150, 5, null, 'foo', 25) ).toThrowError("Not enough arguments")
    })

    test("rect", () => {
      p.rect(50,50,100,100)
      ctx.lineWidth = 6
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      expect(pixel(150, 150)).toEqual(BLACK)
      expect(() => p.rect(0,0, 20) ).toThrowError("Not enough arguments")
    })

    test("arc", () => {
      p.arc(150, 150, 75, Math.PI/8, Math.PI*1.5, true)
      ctx.fillStyle = 'black'
      ctx.fill(p)

      p = new Path2D()
      p.arc(150, 150, 75, Math.PI/8, Math.PI*1.5, false)
      ctx.fillStyle = 'white'
      ctx.fill(p)

      expect(pixel(196, 112)).toEqual(BLACK)
      expect(pixel(150, 150)).toEqual(WHITE)
      expect(() => p.arc(150, 150, 75, Math.PI/8, false) ).toThrowError("Not enough arguments")
      expect(() => p.arc(150, 150, 75, Math.PI/8, Math.PI*1.5) ).not.toThrow()
    })

    test("ellipse", () => {
      // default to clockwise
      p.ellipse(100,100, 100, 50, .25*Math.PI, 0, 1.5*Math.PI)
      ctx.lineWidth = 5
      ctx.strokeStyle = 'black'
      ctx.stroke(p)

      expect(pixel(127, 175)).toEqual(BLACK)
      expect(pixel(130, 60)).toEqual(BLACK)
      expect(pixel(163, 100)).toEqual(CLEAR)

      // with ccw enabled
      p2 = new Path2D()
      p2.ellipse(100,100, 100, 50, .25*Math.PI, 0, 1.5*Math.PI, true)
      ctx.clearRect(0,0, WIDTH, HEIGHT)
      ctx.stroke(p2)

      expect(pixel(127, 175)).toEqual(CLEAR)
      expect(pixel(130, 60)).toEqual(CLEAR)
      expect(pixel(163, 100)).toEqual(BLACK)
    })
  })

  describe("can append", () => {
    test("other paths", () => {
      let left = new Path2D(),
          right = new Path2D();
      left.moveTo(20, 20)
      left.lineTo(100, 100)
      expect(left.bounds).toMatchObject({ left: 20, top: 20, right: 100, bottom: 100 })

      right.moveTo(200, 20)
      right.lineTo(200, 200)
      expect(right.bounds).toMatchObject({ left: 200, top: 20, right: 200, bottom: 200 })

      left.addPath(right)
      expect(left.bounds).toMatchObject({ left: 20, top: 20, right: 200, bottom: 200 })
    })

    test("with a transform matrix", () => {
      let left = new Path2D()
      left.moveTo(0, 0)
      left.lineTo(10, 10)
      expect(left.bounds).toMatchObject( { left: 0, top: 0, right: 10, bottom: 10 } )

      let right = new Path2D(left)
      expect(right.bounds).toMatchObject( { left: 0, top: 0, right: 10, bottom: 10 } )

      let matrix = new DOMMatrix().scale(10,10)
      left.addPath(right, matrix)
      expect(left.bounds).toMatchObject( { left: 0, top: 0, right: 100, bottom: 100 } )
    })

    test("to a closed path", () => {
      ctx.lineWidth = 5
      ctx.strokeStyle = 'black'

      let left = new Path2D()
      left.arc(100, 100, 25, 0, 2*Math.PI)
      expect(left.bounds).toMatchObject( { left: 75, top: 75, right: 125, bottom: 125 } )

      let right = new Path2D()
      right.arc(200, 100, 25, 0, 2*Math.PI)
      expect(right.bounds).toMatchObject( { left: 175, top: 75, right: 225, bottom: 125 } )

      left.addPath(right)
      expect(left.bounds).toMatchObject( { left: 75, top: 75, right: 225, bottom: 125 } )

      // adding creates a path with two separate circles
      ctx.stroke(left)
      expect(pixel(175, 100)).toEqual(BLACK)
      expect(pixel(150, 100)).toEqual(CLEAR)

      // two .arc calls in one path draws a line connecting them
      let solo = new Path2D()
      solo.arc(100, 250, 25, 0, 2*Math.PI)
      solo.arc(200, 250, 25, 0, 2*Math.PI)
      ctx.stroke(solo)
      expect(pixel(175, 250)).toEqual(BLACK)
      expect(pixel(150, 250)).toEqual(BLACK)
    })

    test("self", () => {
      let p = new Path2D()
      p.ellipse(150, 150, 75, 75, 0, Math.PI, Math.PI*2, true)
      p.addPath(p, new DOMMatrix().scale(2,2))
      ctx.fillStyle = 'black'
      ctx.fill(p)

      expect(pixel(150, 151)).toEqual(BLACK)
      expect(pixel(150, 224)).toEqual(BLACK)
      expect(pixel(300, 301)).toEqual(BLACK)
      expect(pixel(300, 449)).toEqual(BLACK)
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
      expect(top()).toEqual(BLACK)
      expect(left()).toEqual(CLEAR)
      expect(center()).toEqual(CLEAR)
      expect(right()).toEqual(CLEAR)
      expect(bottom()).toEqual(BLACK)
    })

    test("difference", () => {
      let c = a.difference(b)
      ctx.fill(c)
      expect(top()).toEqual(CLEAR)
      expect(left()).toEqual(BLACK)
      expect(center()).toEqual(CLEAR)
      expect(right()).toEqual(BLACK)
      expect(bottom()).toEqual(CLEAR)
    })

    test("intersect", () => {
      let c = a.intersect(b)
      ctx.fill(c)
      expect(top()).toEqual(CLEAR)
      expect(left()).toEqual(CLEAR)
      expect(center()).toEqual(BLACK)
      expect(right()).toEqual(CLEAR)
      expect(bottom()).toEqual(CLEAR)
    })

    test("union", () => {
      let c = a.union(b)
      ctx.fill(c)
      expect(top()).toEqual(BLACK)
      expect(left()).toEqual(BLACK)
      expect(center()).toEqual(BLACK)
      expect(right()).toEqual(BLACK)
      expect(bottom()).toEqual(BLACK)
    })

    test("xor", () => {
      let c = a.xor(b)
      ctx.fill(c, 'evenodd')
      expect(top()).toEqual(BLACK)
      expect(left()).toEqual(BLACK)
      expect(center()).toEqual(CLEAR)
      expect(right()).toEqual(BLACK)
      expect(bottom()).toEqual(BLACK)

      ctx.fill(c, 'nonzero')
      expect(center()).toEqual(BLACK)
    })

    test("simplify", () => {
      let c = a.xor(b)
      ctx.fill(c.simplify(), 'nonzero')
      expect(top()).toEqual(BLACK)
      expect(left()).toEqual(BLACK)
      expect(center()).toEqual(CLEAR)
      expect(right()).toEqual(BLACK)
      expect(bottom()).toEqual(BLACK)

      ctx.fill(c)
      expect(center()).toEqual(BLACK)
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
      expect(pixel(100, 102)).toEqual(BLACK)
      expect(pixel(100, 200)).toEqual(BLACK)
      scrub()

      ctx.stroke(start.interpolate(finish, .25))
      expect(pixel(151, 101)).toEqual(BLACK)
      expect(pixel(151, 200)).toEqual(CLEAR)
      expect(pixel(171, 200)).toEqual(BLACK)
      scrub()

      ctx.stroke(start.interpolate(finish, .5))
      expect(pixel(201, 101)).toEqual(BLACK)
      expect(pixel(201, 200)).toEqual(CLEAR)
      expect(pixel(243, 200)).toEqual(BLACK)
      scrub()

      ctx.stroke(start.interpolate(finish, .75))
      expect(pixel(251, 101)).toEqual(BLACK)
      expect(pixel(251, 200)).toEqual(CLEAR)
      expect(pixel(322, 200)).toEqual(BLACK)
      scrub()

      ctx.stroke(start.interpolate(finish, 1))
      expect(pixel(301, 101)).toEqual(BLACK)
      expect(pixel(301, 200)).toEqual(CLEAR)
      expect(pixel(395, 200)).toEqual(BLACK)
      scrub()

      ctx.stroke(start.interpolate(finish, 1.25))
      expect(pixel(351, 101)).toEqual(BLACK)
      expect(pixel(351, 200)).toEqual(CLEAR)
      expect(pixel(470, 200)).toEqual(BLACK)
      scrub()

    })

  })

  describe("can apply path effect", () => {

    test("jitter", () => {
      let line = new Path2D()
      line.moveTo(100, 100)
      line.lineTo(100, 200)

      ctx.lineWidth = 4
      ctx.stroke(line)
      let allBlack = _.range(101, 199).map(y => _.isEqual(pixel(100, y), BLACK))
      expect(allBlack).not.toContain(false)
      scrub()

      let zag = line.jitter(10, 20)
      ctx.stroke(zag)
      let notAllBlack = _.range(101, 199).map(y => _.isEqual(pixel(100, y), BLACK))
      expect(notAllBlack).toContain(false)
      expect(notAllBlack).toContain(true)
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
      expect(alpha()).toEqual(BLACK)
      expect(omega()).toEqual(BLACK)

      expect(topLeft()).toEqual(BLACK)
      expect(topRight()).toEqual(BLACK)
      expect(botLeft()).toEqual(BLACK)
      expect(botRight()).toEqual(BLACK)

      expect(hiLeft()).toEqual(CLEAR)
      expect(hiRight()).toEqual(CLEAR)
      expect(loLeft()).toEqual(CLEAR)
      expect(loRight()).toEqual(CLEAR)

      let rounded = lines.round(80)
      canvas.width = WIDTH
      ctx.lineWidth = 10
      ctx.stroke(rounded)
      expect(alpha()).toEqual(BLACK)
      expect(omega()).toEqual(BLACK)

      expect(topLeft()).toEqual(CLEAR)
      expect(topRight()).toEqual(CLEAR)
      expect(botLeft()).toEqual(CLEAR)
      expect(botRight()).toEqual(CLEAR)

      expect(hiLeft()).toEqual(BLACK)
      expect(hiRight()).toEqual(BLACK)
      expect(loLeft()).toEqual(BLACK)
      expect(loRight()).toEqual(BLACK)
    })

    test("offset", () => {
      let orig = new Path2D()
      orig.rect(10, 10, 40, 40)
      expect(orig.bounds).toMatchObject({left:10, top:10, right:50, bottom:50})

      let shifted = orig.offset(-10, -10)
      expect(shifted.bounds).toMatchObject({left:0, top:0, right:40, bottom:40})

      shifted = shifted.offset(-40, -40)
      expect(shifted.bounds).toMatchObject({left:-40, top:-40, right:0, bottom:0})

      // orig path should be unchanged
      expect(orig.bounds).toMatchObject({left:10, top:10, right:50, bottom:50})
    })

    test("transform", () => {
      let orig = new Path2D()
      orig.rect(-10, -10, 20, 20)
      expect(orig.bounds).toMatchObject({left:-10, top:-10, right:10, bottom:10})

      let shifted = orig.transform(new DOMMatrix().translate(10, 10))
      expect(shifted.bounds).toMatchObject({left:0, top:0, right:20, bottom:20})

      let shiftedByHand = orig.transform(1, 0, 0, 1, 10, 10)
      expect(shifted.edges).toEqual(shiftedByHand.edges)

      let embiggened = orig.transform(new DOMMatrix().scale(2, .5)),
          bigBounds = embiggened.bounds,
          origBounds = orig.bounds
      expect(bigBounds.left).toBeLessThan(origBounds.left)
      expect(bigBounds.right).toBeGreaterThan(origBounds.right)

      // orig path should be unchanged
      expect(orig.bounds).toMatchObject({left:-10, top:-10, right:10, bottom:10})
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
      expect(left()).toEqual(BLACK)
      expect(mid()).toEqual(BLACK)
      expect(right()).toEqual(BLACK)
      scrub()

      ctx.stroke(middle)
      expect(left()).toEqual(CLEAR)
      expect(mid()).toEqual(BLACK)
      expect(right()).toEqual(CLEAR)
      scrub()

      ctx.stroke(endpoints)
      expect(left()).toEqual(BLACK)
      expect(mid()).toEqual(CLEAR)
      expect(right()).toEqual(BLACK)
      scrub()

      ctx.stroke(start)
      expect(left()).toEqual(BLACK)
      expect(mid()).toEqual(CLEAR)
      expect(right()).toEqual(CLEAR)
      scrub()

      ctx.stroke(end)
      expect(left()).toEqual(CLEAR)
      expect(mid()).toEqual(CLEAR)
      expect(right()).toEqual(BLACK)
      scrub()

      ctx.stroke(none)
      expect(left()).toEqual(CLEAR)
      expect(mid()).toEqual(CLEAR)
      expect(right()).toEqual(CLEAR)
      scrub()

      ctx.stroke(everythingAndMore)
      expect(left()).toEqual(BLACK)
      expect(mid()).toEqual(BLACK)
      expect(right()).toEqual(BLACK)
      scrub()
    })
  })
})
