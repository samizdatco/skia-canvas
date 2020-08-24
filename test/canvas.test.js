const _ = require('lodash'),
      fs = require('fs'),
      {Canvas} = require('../lib');

describe("Context2D", ()=>{
  let canvas, ctx
  beforeEach(()=>{
    canvas = new Canvas(512, 512)
    ctx = canvas.getContext("2d")
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
      ctx.setLineDash(null)
      expect(ctx.getLineDash()).toEqual([1,2,3,4])

    })


  })

  describe("parses", () => {
    test('colors', function () {
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

