const _ = require('lodash'),
      fs = require('fs'),
      {CanvasRenderingContext2D} = require('../lib');

describe("Context2D", ()=>{
  let ctx
  beforeEach(()=>{
    ctx = new CanvasRenderingContext2D(512, 512)
  })

  describe("can get & set", ()=>{

    test('globalAlpha', () => {
      expect(ctx.globalAlpha).toBe(1)
      ctx.globalAlpha = 0.25
      expect(ctx.globalAlpha).toBeCloseTo(0.25)
      ctx.globalAlpha = -1
      expect(ctx.globalAlpha).toBeCloseTo(0.25)
      ctx.globalAlpha = 3
      expect(ctx.globalAlpha).toBeCloseTo(0.25)
    })

    test('globalCompositeOperation', () => {
      expect(ctx.globalCompositeOperation).toBe('source-over')
      ctx.globalCompositeOperation = 'multiply'
      expect(ctx.globalCompositeOperation).toBe('multiply')
      ctx.globalCompositeOperation = 'invalid-operator-name'
      expect(ctx.globalCompositeOperation).toBe('multiply')
    })

    test('lineDash', () => {
      expect(ctx.getLineDash()).toEqual([])
      ctx.setLineDash([1,2,3,4])
      expect(ctx.getLineDash()).toEqual([1,2,3,4])
      expect(() => ctx.setLineDash(null)).toThrow()
    })


  })

})

