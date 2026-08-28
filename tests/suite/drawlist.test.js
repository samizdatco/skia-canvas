// @ts-check
//
// Drawlist batching (beziers.md §5) — kernel behavior, flush barriers, re-entrancy,
// source discipline, and strict-mode parity. These exercise the JS-layer verb queue
// and its flush points, not path/context geometry (that lives in path2d/context2d).
//
"use strict"

const {assert} = require('../runner/assert'), 
      {describe, test} = require('node:test'),
      {execFileSync} = require('node:child_process'),
      fs = require('node:fs'),
      path = require('node:path'),
      {Path2D, Canvas} = require('../../lib')

describe("DrawList", () => {

  describe("kernel", () => {
    // — valueOf coercion (documented change #1): args are coerced as they're enqueued —
    test("valueOf coerces at enqueue", () => {
      let p = new Path2D(); p.moveTo(1, 1)
      // @ts-expect-error — objects with valueOf() are coerced at enqueue time
      p.lineTo({valueOf: () => 7}, {valueOf: () => 8})
      assert.equal(p.d, "M1 1L7 8")
    })
  
    // Source discipline (§5d): only the kernel (neon.js, and path.js which now hosts the
    // drawlist kernel) may dereference the boxed-struct symbol [ø]; every other object-handle
    // crossing must go through core().
    test("no [ø] dereference outside the kernel", () => {
      let dir = path.join(__dirname, '../../lib/classes')
      let offenders = fs.readdirSync(dir)
        .filter(f => f.endsWith('.js') && f !== 'neon.js' && f !== 'path.js')
        .filter(f => /\[ø\]/.test(fs.readFileSync(path.join(dir, f), 'utf8')))
      assert.deepEqual(offenders, [], `these files bypass core() by reading [ø]: ${offenders.join(', ')}`)
    })
  })
  
  describe("flush barriers", () => {
    // — receiver barrier: a query flushes the object's own pending verbs —
    test("a query flushes pending verbs", () => {
      let p = new Path2D(); p.rect(0, 0, 30, 30)              // pending, never queried
      assert.deepEqual([p.bounds.left, p.bounds.right], [0, 30])
    })
  
    // — pending queues drain before a handle crosses via core() —
    test("fill(path) drains the arg queue", () => {
      let ctx = new Canvas(100, 100).getContext('2d')
      let p = new Path2D(); p.rect(20, 20, 40, 40)           // pending
      ctx.fillStyle = 'black'; ctx.fill(p)
      assert.deepEqual([...ctx.getImageData(40, 40, 1, 1).data], [0, 0, 0, 255])
    })
  
    test("addPath drains both queues", () => {
      let a = new Path2D(); a.rect(0, 0, 10, 10)             // pending receiver
      let b = new Path2D(); b.rect(50, 50, 10, 10)           // pending argument
      a.addPath(b)
      assert.deepEqual([a.bounds.left, a.bounds.right], [0, 60])
    })
  
    test("new Path2D(pending) drains the source", () => {
      let a = new Path2D(); a.rect(5, 5, 20, 20)             // pending
      let b = new Path2D(a)
      assert.equal(b.d, a.d)
      assert.ok(b.d.length > 0)
    })    
  })
  
  describe("reëntrancy", () => {
    // — re-entrancy (coerce-then-commit, §5f) —
    test("nested valueOf replays once, in order", () => {
      let p = new Path2D()
      p.moveTo(0, 0); p.lineTo(10, 10)
      let hot = { valueOf(){ p.lineTo(50, 50); void p.bounds; return 20 } } // nested enqueue + barrier flush
      // @ts-expect-error — deliberately passing a valueOf-bearing object
      p.lineTo(hot, 30)
      // outer record committed exactly once, after the nested verb, using fresh queue state
      assert.equal(p.d, "M0 0L10 10L50 50L20 30")
    })
  
    test("a throwing valueOf leaves the queue untouched", () => {
      let p = new Path2D(); p.moveTo(0, 0); p.lineTo(10, 10)
      // @ts-expect-error — deliberately passing a throwing valueOf
      assert.throws(() => p.lineTo({valueOf(){ throw new Error('boom') }}, 5), /boom/)
      assert.equal(p.d, "M0 0L10 10") // the failed verb added nothing
    })    
  })

  describe("error parity", () => {
    // — non-strict: a verb with a NaN arg is dropped from the batch (adds nothing to `d`) —
    test("invalid verbs are dropped from the path", () => {
      let p = new Path2D(); p.moveTo(5, 5); p.lineTo(10, 10); p.lineTo(NaN, 0)
      assert.equal(p.d, "M5 5L10 10")
  
      let q = new Path2D(); q.roundRect(NaN, 0, 0, 0, 5)
      assert.equal(q.d, "")
    })
  
    // — draw-verb error parity: the batched wrappers still raise the un-batched arg errors —
    test("fill(NaN) throws fillRule error", () => {
      let ctx = new Canvas(10, 10).getContext('2d')
      // @ts-expect-error — deliberately invalid arg
      assert.throws(() => ctx.fill(NaN), /Expected `fillRule`/)
    })
  
    test("fill(NaN, 'evenodd') throws Path2D error", () => {
      let ctx = new Canvas(10, 10).getContext('2d')
      // @ts-expect-error — deliberately invalid arg (2-arg form wants a Path2D)
      assert.throws(() => ctx.fill(NaN, 'evenodd'), /Expected a Path2D/)
    })
  
    test("stroke(NaN) throws Path2D error", () => {
      let ctx = new Canvas(10, 10).getContext('2d')
      // @ts-expect-error — deliberately invalid arg
      assert.throws(() => ctx.stroke(NaN), /Expected a Path2D/)
    })    
  
    // — strict-mode parity: SKIA_CANVAS_STRICT is read once when neon.js loads, so it can't
    // be toggled in-process. Exercise every strict-mode rejection in a single child process
    // (rather than one per case); the child collects mismatches, prints them to stderr, and
    // exits non-zero. —
    test("SKIA_CANVAS_STRICT", () => {
      const lib = require.resolve('../../lib')
      const child = `
        const {Path2D} = require(${JSON.stringify(lib)})
  
        // [thunk, expected-message] — under strict, each invalid arg must throw a TypeError
        const cases = [
          [() => new Path2D().lineTo(NaN, 0),                    /Expected a number for \`x\` as 1st arg/],
          [() => new Path2D().ellipse(0, 0, 0, NaN, 0, 0, 0),    /Expected a number for \`yRadius\` as 4th arg/],
          [() => new Path2D().bezierCurveTo(0, 0, 0, 0, 0, NaN), /Expected a number for \`y\` as 6th arg/],
          [() => new Path2D().roundRect(NaN, 0, 0, 0, 5),        /Expected a number for \`x\` as 1st arg/],
        ]
  
        const fails = []
        for (const [fn, re] of cases){
          let e = null
          try { fn() } catch(err){ e = err }
          if (!e) fails.push(fn + ' should have thrown')
          else if (!(e instanceof TypeError)) fails.push(fn + ' threw ' + e.constructor.name + ', expected TypeError')
          else if (!re.test(e.message)) fails.push(fn + ' message ' + JSON.stringify(e.message) + ' does not match ' + re)
        }
        if (fails.length){ console.error(fails.join('\\n')); process.exit(1) }
      `
      try {
        execFileSync(process.execPath, ['-e', child], {
          env: {...process.env, SKIA_CANVAS_STRICT: '1'},
          encoding: 'utf8', stdio: ['ignore', 'ignore', 'pipe'],
        })
      } catch (err) {
        assert.fail('strict-mode expectations failed:\n' + String(err.stderr || err.message).trim())
      }
    })
  })

  describe("Context2D", () => {
    // — Context2D flip: transforms, state stack, and draw verbs are all batched now —
    test("transforms bake into batched verbs", () => {
      let c = new Canvas(200, 200); c.gpu = false; let ctx = c.getContext('2d')
      ctx.fillStyle = 'black'
      ctx.translate(100, 100)
      ctx.fillRect(-10, -10, 20, 20) // centered at (100,100) after translate
      assert.deepEqual([...ctx.getImageData(100, 100, 1, 1).data], [0, 0, 0, 255])
      assert.deepEqual([...ctx.getImageData(5, 5, 1, 1).data], [0, 0, 0, 0]) // origin untouched
    })
  
    test("getTransform flushes pending transforms", () => {
      let c = new Canvas(50, 50); c.gpu = false; let ctx = c.getContext('2d')
      ctx.translate(12, 34) // queued
      let m = ctx.getTransform() // barrier → flush → read native matrix
      assert.equal(m.e, 12); assert.equal(m.f, 34)
    })
  
    test("save/restore captures paint at save time", () => {
      let c = new Canvas(60, 20); c.gpu = false; let ctx = c.getContext('2d')
      ctx.fillStyle = 'rgb(255,0,0)'
      ctx.save()
      ctx.fillStyle = 'rgb(0,0,255)'
      ctx.fillRect(0, 0, 20, 20)   // blue
      ctx.restore()                 // back to red
      ctx.fillRect(40, 0, 20, 20)  // red
      assert.deepEqual([...ctx.getImageData(10, 10, 1, 1).data], [0, 0, 255, 255])
      assert.deepEqual([...ctx.getImageData(50, 10, 1, 1).data], [255, 0, 0, 255])
    })
  
    test("beginPath/arc/fill marker loop draws each marker", () => {
      let c = new Canvas(200, 60); c.gpu = false; let ctx = c.getContext('2d')
      ctx.fillStyle = 'black'
      for (let i = 0; i < 3; i++){
        ctx.beginPath(); ctx.arc(30 + i * 60, 30, 12, 0, 6.29); ctx.fill()
      }
      for (let i = 0; i < 3; i++){
        assert.deepEqual([...ctx.getImageData(30 + i * 60, 30, 1, 1).data], [0, 0, 0, 255], `marker ${i}`)
      }
    })
  
    test("fill(path) drains the ctx queue then draws the arg", () => {
      let c = new Canvas(80, 80); c.gpu = false; let ctx = c.getContext('2d')
      ctx.fillStyle = 'black'
      ctx.rect(0, 0, 5, 5)              // pending on the ctx queue (current path)
      let p = new Path2D(); p.rect(20, 20, 40, 40)
      ctx.fill(p)                       // core(p) + ƒ('fill') — both queues drain
      assert.deepEqual([...ctx.getImageData(40, 40, 1, 1).data], [0, 0, 0, 255])
    })
  
    test("clip is a barrier constraining later draws", () => {
      let c = new Canvas(100, 100); c.gpu = false; let ctx = c.getContext('2d')
      ctx.beginPath(); ctx.rect(0, 0, 40, 40); ctx.clip() // barrier
      ctx.fillStyle = 'black'; ctx.fillRect(0, 0, 100, 100)
      assert.deepEqual([...ctx.getImageData(20, 20, 1, 1).data], [0, 0, 0, 255]) // inside clip
      assert.deepEqual([...ctx.getImageData(70, 70, 1, 1).data], [0, 0, 0, 0])   // outside clip
    })
  })

})
