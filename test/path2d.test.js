const _ = require('lodash'),
      {Canvas, DOMMatrix, Path2D} = require('../lib');

const BLACK = [0,0,0,255],
      WHITE = [255,255,255,255],
      CLEAR = [0,0,0,0]

describe("Path2D", ()=>{
  let canvas, ctx,
      WIDTH = 512, HEIGHT = 512,
      pixel = (x, y) => Array.from(ctx.getImageData(x, y, 1, 1).data),
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

  describe("supports path operation", () => {
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

  })


})
