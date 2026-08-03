// @ts-check

"use strict"

const {assert} = require('../runner/assert'), {describe, test} = require('node:test'),
      {DOMMatrix, DOMPoint, DOMRect} = require('../../lib');

const cells = m => [m.a, m.b, m.c, m.d, m.e, m.f]
const nearly = (actual, expected) => actual.forEach((v, i) => assert.nearEqual(v, expected[i]))

describe("DOMMatrix", ()=>{
  describe("can be initialized with", ()=>{
    test("no arguments", () => {
      let m = new DOMMatrix()
      assert.equal(m.isIdentity, true)
      assert.equal(m.is2D, true)
      assert.deepEqual(cells(m), [1, 0, 0, 1, 0, 0])
    })

    test("an array of values", () => {
      let m = new DOMMatrix([2, 0, 0, 3, 10, 20])
      assert.deepEqual(cells(m), [2, 0, 0, 3, 10, 20])
    })

    test("a css transform string", () => {
      let m = new DOMMatrix('translate(10px, 20px) scale(2)')
      nearly(cells(m), [2, 0, 0, 2, 10, 20])
    })
  })

  describe("can transform", ()=>{
    test("points", () => {
      let p = new DOMMatrix().rotate(90).transformPoint(new DOMPoint(1, 0))
      assert.nearEqual(p.x, 0)
      assert.nearEqual(p.y, 1)
      assert.equal(p.w, 1)
    })

    test("by composing operations", () => {
      let chained = new DOMMatrix().translate(10, 20).scale(2),
          product = new DOMMatrix().translate(10, 20).multiply(new DOMMatrix().scale(2))
      nearly(cells(chained), [2, 0, 0, 2, 10, 20])
      nearly(cells(product), cells(chained))
    })

    test("invertibly", () => {
      let m = new DOMMatrix([2, 0, 0, 3, 10, 20]),
          round = m.multiply(m.inverse())
      assert.equal(round.isIdentity, true)
      nearly(cells(round), [1, 0, 0, 1, 0, 0])
    })
  })

  describe("distinguishes", ()=>{
    test("mutating & non-mutating calls", () => {
      let m = new DOMMatrix([2, 0, 0, 3, 10, 20]),
          rotated = m.rotate(45)
      assert.deepEqual(cells(m), [2, 0, 0, 3, 10, 20])
      assert.notDeepEqual(cells(rotated), cells(m))

      m.scaleSelf(2, 2)
      nearly([m.a, m.d], [4, 6])
    })

    test("2d & 3d transforms", () => {
      let flat = new DOMMatrix().translate(5, 5)
      assert.equal(flat.is2D, true)

      let deep = new DOMMatrix().translate(0, 0, 5)
      assert.equal(deep.is2D, false)
      assert.equal(deep.m43, 5)
    })
  })

  describe("supports", ()=>{
    test("flips", () => {
      let x = new DOMMatrix().flipX(),
          y = new DOMMatrix().flipY()
      nearly([x.a, x.d], [-1, 1])
      nearly([y.a, y.d], [1, -1])
    })

    test("skews", () => {
      let x = new DOMMatrix().skewX(30),
          y = new DOMMatrix().skewY(30)
      assert.nearEqual(x.c, Math.tan(Math.PI/6))
      assert.nearEqual(x.b, 0)
      assert.nearEqual(y.b, Math.tan(Math.PI/6))
      assert.nearEqual(y.c, 0)
    })

    test("css serialization", () => {
      let m = new DOMMatrix([1.5, 0, 0, 1.5, 5, 6])
      assert.equal(m.toString(), 'matrix(1.5, 0, 0, 1.5, 5, 6)')
      assert.deepEqual(cells(new DOMMatrix(m.toString())), cells(m))
    })
  })
})

describe("DOMPoint", ()=>{
  test("defaults to the origin", () => {
    let p = new DOMPoint()
    assert.deepEqual([p.x, p.y, p.z, p.w], [0, 0, 0, 1])
  })

  test("can be copied from a point-like object", () => {
    let p = DOMPoint.fromPoint({x:3, y:4})
    assert.deepEqual([p.x, p.y, p.w], [3, 4, 1])
  })

  test("can apply matrices", () => {
    let p = new DOMPoint(3, 4).matrixTransform(new DOMMatrix().translate(10, 10))
    assert.deepEqual([p.x, p.y], [13, 14])
  })
})

describe("DOMRect", ()=>{
  test("derives its edges", () => {
    let r = new DOMRect(10, 20, 30, 40)
    assert.matchesSubset(r, {x:10, y:20, width:30, height:40})
    assert.deepEqual([r.left, r.top, r.right, r.bottom], [10, 20, 40, 60])
  })

  test("can be copied from a rect-like object", () => {
    let r = DOMRect.fromRect({x:1, y:2, width:3, height:4})
    assert.deepEqual([r.x, r.y, r.width, r.height], [1, 2, 3, 4])
  })

  test("normalizes negative dimensions", () => {
    // currently failing: the spec defines left/top as min(x, x+width)/min(y, y+height)
    // but the getters in classes/geometry.js return x/y as-is
    let r = new DOMRect(10, 20, -30, -40)
    assert.deepEqual([r.left, r.top, r.right, r.bottom], [-20, -20, 10, 20])
  })
})
