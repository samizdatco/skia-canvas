// @ts-check

"use strict"

const path = require('path'),
      os = require('os'),
      fs = require('fs'),
      {assert} = require('../runner/assert'),
      {describe, test, beforeEach, afterEach} = require('node:test'),
      {execFileSync} = require('node:child_process'),
      {Canvas, FontLibrary} = require('../../lib')


// ---------------------------------------------------------------------------
// PDF export: verifies that exported PDFs carry selectable/searchable text —
// the text path attaches each glyph run's source UTF-8 + cluster mapping to its blob.
// ---------------------------------------------------------------------------

// These tests verify the *exported* PDF against a real reader (pdftotext / poppler).
// Where it isn't installed (e.g. CI runners), the whole group skips rather than fails.
const hasPdftotext = (() => {
  try { execFileSync('pdftotext', ['-v'], {stdio:'ignore'}); return true }
  catch { return false }
})()

describe("Typography", () => {
  /** @type {Canvas} */
  let canvas
  /** @type {import('../../lib').CanvasRenderingContext2D} */
  let ctx
  let WIDTH = 512, HEIGHT = 512,
      FONTS_DIR = 'tests/assets/fonts',
      findFont = font => path.join(FONTS_DIR, font);

  beforeEach(() => {
    canvas = new Canvas(WIDTH, HEIGHT)
    ctx = canvas.getContext("2d")
  })

  afterEach(() => {
    FontLibrary.reset()
  })

  test("fontSynthesis", () => {
    // Monoton has a single (regular) face, so bold/italic have no *real* face to match
    FontLibrary.use(findFont("Monoton-Regular.woff"))
    let render = (spec, synth) => {
      ctx.fontSynthesis = synth
      ctx.clearRect(0, 0, WIDTH, HEIGHT)
      ctx.font = `${spec} 80px Monoton`
      ctx.fillText("RR", 40, 100)
      return ctx.getImageData(0, 0, WIDTH, HEIGHT).data
    }

    // count opaque pixels for weight comparisons
    let opaque = data => data.filter((v, i) => i % 4 == 3 && v > 10).length
    // diff alpha channels of pixmaps for oblique/normal comparisons
    let differs = (a, b) => { for (let i = 3; i < a.length; i += 4) if ((a[i] > 10) !== (b[i] > 10)) return true; return false }

    assert.equal(ctx.fontSynthesis, true) // browser-parity default: synthesis on
    let regular = render("400", true)
    assert(opaque(regular) > 0)

    // default is synthesis-on
    assert(opaque(render("700", true)) > opaque(regular)) // faux bold lays down more ink
    assert(differs(render("italic", true), regular))      // faux oblique skews the glyphs

    // ensure that disabling it actually prevents fake weight/slant
    assert.equal(opaque(render("700", false)), opaque(regular))
    assert(!differs(render("italic", false), regular))

    // check that the setting isn't cached across calls (i.e., FontLibrary clears its cache)
    assert(opaque(render("700", true)) > opaque(regular))
  })

  test("fontHinting", () => {
    // a face whose hinting instructions survive in the tracked woff2 subset
    FontLibrary.use("Montserrat", [findFont("montserrat-latin/montserrat-v30-latin-regular.woff2")])

    let render = (hinting) => {
      ctx.fontHinting = hinting
      ctx.clearRect(0, 0, WIDTH, HEIGHT)
      ctx.font = '11px Montserrat'
      ctx.fillText("Illegible waveforms 10x", 10, 24)
      return ctx.getImageData(0, 0, WIDTH, HEIGHT).data
    }
    let differs = (a, b) => { for (let i = 3; i < a.length; i += 4) if (a[i] !== b[i]) return true; return false }

    // boolean property; hinting disabled by default to match browser rendering
    assert.equal(ctx.fontHinting, false)
    ctx.fontHinting = true
    assert.equal(ctx.fontHinting, true)

    // truthy coercion like fontSynthesis (invalid values never throw)
    // @ts-expect-error — deliberately mistyped: exercises truthy coercion
    ctx.fontHinting = 0
    assert.equal(ctx.fontHinting, false)
    // @ts-expect-error — deliberately mistyped: exercises truthy coercion
    ctx.fontHinting = 'yes'
    assert.equal(ctx.fontHinting, true)

    // hinted vs unhinted rasterization must differ at text sizes (DirectWrite
    // quantization is unverified, hence the win32 escape hatch)
    let unhinted = render(false)
    let hinted = render(true)
    if (os.platform() != 'win32'){
      assert(differs(hinted, unhinted))
    }

    // toggling back must return to the identical rendering (i.e., FontLibrary clears its cache)
    assert(!differs(render(false), unhinted))
  })

  test("fontSmoothing", () => {
    FontLibrary.use("Montserrat", [findFont("montserrat-latin/montserrat-v30-latin-regular.woff2")])

    let census = (smoothing) => {
      ctx.fontSmoothing = smoothing
      ctx.clearRect(0, 0, WIDTH, HEIGHT)
      ctx.font = '40px Montserrat'
      ctx.fillText("Osprey wings", 10, 55)
      let data = ctx.getImageData(0, 0, WIDTH, HEIGHT).data,
          tally = {opaque:0, partial:0}
      for (let i = 3; i < data.length; i += 4){
        if (data[i] == 255) tally.opaque++
        else if (data[i] > 0) tally.partial++
      }
      return tally
    }

    // boolean property; smoothing on by default (truthy coercion, like fontSynthesis)
    assert.equal(ctx.fontSmoothing, true)
    ctx.fontSmoothing = false
    assert.equal(ctx.fontSmoothing, false)
    // @ts-expect-error — deliberately mistyped: exercises truthy coercion
    ctx.fontSmoothing = 'yes'
    assert.equal(ctx.fontSmoothing, true)

    // smoothed edges include partial-alpha pixels; aliased coverage is strictly 0/255
    let aliased = census(false)
    assert(aliased.opaque > 0)
    assert.equal(aliased.partial, 0)
    let antialiased = census(true)
    assert(antialiased.partial > 0)

    // toggling back must re-alias (i.e., FontLibrary clears its cache)
    assert.equal(census(false).partial, 0)

    // smoothing also drives subpixel positioning: fractional placement renders
    // differently than the aliased/grid-snapped case, and round-trips after toggling
    let render = (smoothing) => {
      ctx.fontSmoothing = smoothing
      ctx.clearRect(0, 0, WIDTH, HEIGHT)
      ctx.font = '11px Montserrat'
      ctx.fillText("Illegible waveforms 10x", 10.5, 24)
      return ctx.getImageData(0, 0, WIDTH, HEIGHT).data
    }
    let differs = (a, b) => { for (let i = 3; i < a.length; i += 4) if (a[i] !== b[i]) return true; return false }
    let smoothed = render(true)
    assert(differs(render(false), smoothed))
    assert(!differs(render(true), smoothed))
  })


  describe("fontVariant", () => {
    test("defaults to normal", () => {
      assert.equal(ctx.fontVariant, "normal")
    })

    test("single keywords", () => {
      for (let kw of ["small-caps", "all-small-caps", "tabular-nums", "oldstyle-nums",
                      "lining-nums", "discretionary-ligatures", "slashed-zero", "ordinal",
                      "super", "sub"]){
        ctx.fontVariant = kw
        assert.equal(ctx.fontVariant, kw)
      }
    })

    test("normal resets", () => {
      ctx.fontVariant = "small-caps"
      ctx.fontVariant = "normal"
      assert.equal(ctx.fontVariant, "normal")
    })

    test("space-separated combos", () => {
      ctx.fontVariant = "small-caps tabular-nums"
      assert.equal(ctx.fontVariant, "small-caps tabular-nums")
      ctx.fontVariant = "oldstyle-nums lining-nums"
      assert.equal(ctx.fontVariant, "oldstyle-nums lining-nums")
    })

    test("parameterized alternates", () => {
      for (let v of ["stylistic(2)", "styleset(3)", "swash(1)",
                     "character-variant(4)", "ornaments(1)", "annotation(1)"]){
        ctx.fontVariant = v
        assert.equal(ctx.fontVariant, v)
      }
    })

    test("applies to shaping", () => {
      FontLibrary.use("TestFace", [findFont("montserrat-latin/montserrat-v30-latin-regular.woff2")])
      ctx.font = "40px TestFace"
      let text = "0123456789",
          base = ctx.measureText(text).width
      ctx.fontVariant = "tabular-nums"
      let tnum = ctx.measureText(text).width
      assert(tnum > base, `expected tabular-nums width (${tnum}) > proportional width (${base})`)
    })

    test("ignores invalid values", () => {
      ctx.fontVariant = "small-caps"
      assert.doesNotThrow(() => { ctx.fontVariant = "bogus" })
      assert.equal(ctx.fontVariant, "small-caps")
    })

    test("rejects an all-or-nothing combo", () => {
      ctx.fontVariant = "tabular-nums"
      ctx.fontVariant = "small-caps bogus"
      assert.equal(ctx.fontVariant, "tabular-nums")
    })
  })

  describe("fontStretch", () => {
    test("defaults to normal", () => {
      assert.equal(ctx.fontStretch, "normal")
    })

    test("stretch keywords", () => {
      /** @type {Array<import('../../lib').CanvasRenderingContext2D['fontStretch']>} */
      let keywords = ["ultra-condensed", "extra-condensed", "condensed", "semi-condensed",
                      "semi-expanded", "expanded", "extra-expanded", "ultra-expanded", "normal"]
      for (let kw of keywords){
        ctx.fontStretch = kw
        assert.equal(ctx.fontStretch, kw)
      }
    })

    test("ignores invalid values", () => {
      ctx.fontStretch = "condensed"
      assert.doesNotThrow(() => {
        // @ts-expect-error - "bogus" is not a valid CanvasFontStretch
        ctx.fontStretch = "bogus"
      })
      assert.equal(ctx.fontStretch, "condensed")
    })
  })

  describe("letterSpacing / wordSpacing", () => {
    test("default to 0px", () => {
      assert.equal(ctx.letterSpacing, "0px")
      assert.equal(ctx.wordSpacing, "0px")
    })

    test("absolute units", () => {
      for (let len of ["5px", "2pt", "1pc", "0.5cm", "10q"]){
        ctx.letterSpacing = len
        assert.equal(ctx.letterSpacing, len)
        ctx.wordSpacing = len
        assert.equal(ctx.wordSpacing, len)
      }
    })

    test("em / rem units", () => {
      ctx.letterSpacing = "0.1em"
      assert.equal(ctx.letterSpacing, "0.1em")
      ctx.wordSpacing = "1rem"
      assert.equal(ctx.wordSpacing, "1rem")
    })

    test("em tracks font size", () => {
      // 1em of letter-spacing should widen text twice as much at 40px as at 20px
      ctx.letterSpacing = "1em"
      ctx.font = "40px serif"
      let wide = ctx.measureText("abc").width
      ctx.font = "20px serif"
      let narrow = ctx.measureText("abc").width
      assert(wide > narrow, `expected 40px spacing (${wide}) > 20px spacing (${narrow})`)
    })

    test("rem tracks a 16px root", () => {
      // rem is root-relative, so 1rem == 16px regardless of the font size (unlike em above)
      for (let px of ["40px", "10px"]){
        ctx.font = `${px} serif`
        ctx.letterSpacing = "1rem"
        let rem = ctx.measureText("abc").width
        ctx.letterSpacing = "16px"
        let px16 = ctx.measureText("abc").width
        assert.nearEqual(rem, px16)
      }
    })

    test("ignores unresolvable units (%, ex, ch)", () => {
      ctx.letterSpacing = "4px"
      for (let bad of ["10%", "2ex", "3ch"]){
        ctx.letterSpacing = bad
        assert.equal(ctx.letterSpacing, "4px", `${bad} should have been ignored`)
      }
    })

    test("ignores invalid values", () => {
      ctx.wordSpacing = "6px"
      assert.doesNotThrow(() => { ctx.wordSpacing = "bogus" })
      assert.equal(ctx.wordSpacing, "6px")
    })
  })

  describe("textDecoration", () => {
    describe("parsing", () => {
        test("defaults to none", () => {
          assert.equal(ctx.textDecoration, "none")
        })

        test("a bare line keyword", () => {
          // a decoration with no explicit color uses `currentColor`; it must still apply rather than
          // being dropped as "invalid" (regression: underline-alone previously round-tripped to "none")
          for (let line of ["underline", "overline", "line-through"]){
            ctx.textDecoration = "none"
            ctx.textDecoration = line
            assert.equal(ctx.textDecoration, line)
          }
        })

        test("a line + style + color combo", () => {
          ctx.textDecoration = "underline wavy red"
          assert.equal(ctx.textDecoration, "underline wavy red")
        })

        test("a wide-gamut color", () => {
          ctx.textDecoration = "line-through oklch(0.7 0.1 200)"
          assert.equal(ctx.textDecoration, "line-through oklch(0.7 0.1 200)")
        })

        test("ignores junk", () => {
          // junk must no-op (retain the prior decoration), not reset to "none": a single unrecognized
          // token, an unparseable color, and junk hidden behind a valid color
          ctx.textDecoration = "underline wavy red"
          for (let junk of ["blahblah", "underline blahblah", "notaword blue"]){
            ctx.textDecoration = junk
            assert.equal(ctx.textDecoration, "underline wavy red", `"${junk}" should be ignored`)
          }
        })

        test("none resets", () => {
          ctx.textDecoration = "underline"
          ctx.textDecoration = "none"
          assert.equal(ctx.textDecoration, "none")
        })
      })

    describe("rendering", () => {
      const W = 300, H = 120, BASELINE = 70, FONT = "40px Helvetica"

      // Count dark pixels within a horizontal band [y0,y1). "mmmm" has no ascenders/descenders,
      // so the rows just above the glyphs and just below the baseline are otherwise empty —
      // any ink there comes from an over/underline.
      function inkInBand(decoration, y0, y1, {text = "mmmm", channel = null} = {}){
        const canvas = new Canvas(W, H)
        const ctx = canvas.getContext('2d')
        ctx.fillStyle = 'white'; ctx.fillRect(0, 0, W, H)
        ctx.fillStyle = 'black'
        ctx.font = FONT
        ctx.textBaseline = 'alphabetic'
        if (decoration) ctx.textDecoration = decoration
        ctx.fillText(text, 20, BASELINE)

        const {data} = ctx.getImageData(0, 0, W, H)
        let n = 0
        for (let y = y0; y < y1; y++) for (let x = 0; x < W; x++){
          const i = (y*W + x) * 4
          if (channel !== null){ if (data[i+channel] > 100 && data[i+3] > 128) n++ }
          else if (data[i] < 128 && data[i+3] > 128) n++
        }
        return n
      }

      test("underline", () => {
        const plain = inkInBand(null, BASELINE+2, BASELINE+14)
        const lined = inkInBand("underline", BASELINE+2, BASELINE+14)
        assert.equal(plain, 0, "expected no ink below baseline without a decoration")
        assert(lined > 0, "expected underline ink below the baseline")
      })

      test("overline", () => {
        const plain = inkInBand(null, 22, 42)
        const lined = inkInBand("overline", 22, 42)
        assert.equal(plain, 0, "expected no ink above the glyphs without a decoration")
        assert(lined > 0, "expected overline ink above the glyphs")
      })

      test("supports solid, double, dotted, dashed, and wavy", () => {
        for (const style of ["", "double", "dotted", "dashed", "wavy"]){
          assert(inkInBand(`underline ${style}`.trim(), BASELINE+1, BASELINE+18) > 0,
            `expected ink for underline ${style || "solid"}`)
        }
      })

      test("explicit color", () => {
        // a blue underline must leave blue (not black) pixels below the baseline
        assert(inkInBand("underline blue", BASELINE+2, BASELINE+14, {channel: 2}) > 0,
          "expected blue underline pixels")
      })

      test("skips ink around descenders", () => {
        // within the underline's inked span, descender glyphs leave gaps; plain glyphs don't
        const underlineGaps = (text) => {
          const canvas = new Canvas(W, H)
          const ctx = canvas.getContext('2d')
          ctx.fillStyle = 'white'; ctx.fillRect(0, 0, W, H)
          ctx.fillStyle = 'black'; ctx.font = FONT; ctx.textBaseline = 'alphabetic'
          ctx.textDecoration = 'underline'
          ctx.fillText(text, 20, BASELINE)
          const {data} = ctx.getImageData(0, 0, W, H)
          const inked = (x) => {
            for (let y = BASELINE+2; y < BASELINE+7; y++){
              const i = (y*W + x) * 4
              if (data[i] < 128 && data[i+3] > 128) return true
            }
            return false
          }
          let first = -1, last = -1
          for (let x = 0; x < W; x++) if (inked(x)){ if (first < 0) first = x; last = x }
          if (first < 0) return 0
          let empties = 0
          for (let x = first; x <= last; x++) if (!inked(x)) empties++
          return empties
        }
        assert(underlineGaps("pqjy") > 0, "expected the underline to break under descenders")
        assert.equal(underlineGaps("mmmm"), 0, "expected a continuous underline with no descenders")
      })

      test("skips a leading descender", () => {
        // skparagraph draws straight through a descender hugging the line's start; we gap it.
        // 'june' has exactly one descender — the leading 'j' — so the underline picks up a gap
        // under the leading glyph, while 'nune' (no descenders) stays continuous there.
        const gapsUnderLeadGlyph = (text) => {
          const canvas = new Canvas(W, H)
          const ctx = canvas.getContext('2d')
          ctx.fillStyle = 'white'; ctx.fillRect(0, 0, W, H)
          ctx.fillStyle = 'black'; ctx.font = FONT; ctx.textBaseline = 'alphabetic'
          ctx.textDecoration = 'underline'
          ctx.fillText(text, 20, BASELINE)
          const {data} = ctx.getImageData(0, 0, W, H)
          const inked = (x) => {
            for (let y = BASELINE+2; y < BASELINE+7; y++){
              const i = (y*W + x) * 4
              if (data[i] < 128 && data[i+3] > 128) return true
            }
            return false
          }
          // ink-free columns after the underline first appears, within the leading glyph (~x20–34)
          let seen = false, gaps = 0
          for (let x = 20; x < 34; x++){ if (inked(x)) seen = true; else if (seen) gaps++ }
          return gaps
        }
        assert(gapsUnderLeadGlyph("june") > 0, "expected the leading 'j' descender to be skipped")
        assert.equal(gapsUnderLeadGlyph("nune"), 0, "expected no gap under a leading non-descender")
      })

      test("line-through", () => {
        // between the strokes of "l l" the mid-height row is empty until a line-through fills it
        const plain = inkInBand(null, BASELINE-16, BASELINE-10, {text: "l l l"})
        const struck = inkInBand("line-through", BASELINE-16, BASELINE-10, {text: "l l l"})
        assert(struck > plain, "expected line-through to add ink across the gaps")
      })

      describe("inherits the text's fill", () => {
        const Y0 = BASELINE + 2, Y1 = BASELINE + 14   // underline band (below the m's, no descenders)

        // render "mmmm" with a caller-supplied fill (color/gradient/pattern) + decoration
        function render(makeFill, decoration, text = "mmmm"){
          const canvas = new Canvas(W, H)
          const ctx = canvas.getContext('2d')
          ctx.fillStyle = 'white'; ctx.fillRect(0, 0, W, H)
          ctx.font = FONT; ctx.textBaseline = 'alphabetic'
          ctx.fillStyle = makeFill(ctx)
          if (decoration) ctx.textDecoration = decoration
          ctx.fillText(text, 20, BASELINE)
          return ctx.getImageData(0, 0, W, H)
        }
        // mean color of the opaque, non-white pixels in a band [y0,y1) × [x0,x1)
        function bandColor({data}, y0, y1, x0 = 0, x1 = W){
          let r = 0, g = 0, b = 0, n = 0
          for (let y = y0; y < y1; y++) for (let x = Math.floor(x0); x < x1; x++){
            const i = (y*W + x) * 4
            if (data[i+3] > 128 && !(data[i] > 240 && data[i+1] > 240 && data[i+2] > 240)){
              r += data[i]; g += data[i+1]; b += data[i+2]; n++
            }
          }
          return { r: n ? r/n : 0, g: n ? g/n : 0, b: n ? b/n : 0, n }
        }
        // first/last inked column of the underline, for left/right sampling
        function extent({data}, y0, y1){
          let lo = -1, hi = -1
          for (let x = 0; x < W; x++) for (let y = y0; y < y1; y++){
            const i = (y*W + x) * 4
            if (data[i+3] > 128 && !(data[i] > 240 && data[i+1] > 240 && data[i+2] > 240)){
              if (lo < 0) lo = x; hi = x; break
            }
          }
          return { lo, hi }
        }
        const gradient = ctx => {
          const g = ctx.createLinearGradient(20, 0, 160, 0)
          g.addColorStop(0, 'red'); g.addColorStop(1, 'blue')
          return g
        }

        test("from a gradient", () => {
          const img = render(gradient, 'underline')
          const { lo, hi } = extent(img, Y0, Y1)
          assert(hi > lo, "expected an inked underline")
          const third = (hi - lo) / 3
          const left = bandColor(img, Y0, Y1, lo, lo + third)
          const right = bandColor(img, Y0, Y1, hi - third, hi)
          assert(left.r > left.b, "the underline's left end should follow the gradient's red")
          assert(right.b > right.r, "the underline's right end should follow the gradient's blue")
          assert(left.r > 80 && right.b > 80, "the underline should carry the fill's color, not be solid black")
        })

        test("from a pattern", () => {
          const pattern = ctx => {
            const tile = new Canvas(8, 8), t = tile.getContext('2d')
            t.fillStyle = 'red'; t.fillRect(0, 0, 8, 8)
            return ctx.createPattern(tile, 'repeat')
          }
          const c = bandColor(render(pattern, 'underline'), Y0, Y1)
          assert(c.n > 0, "expected an inked underline")
          assert(c.r > c.g && c.r > c.b, "the underline should pick up the pattern's red, not be black")
        })

        test("from a solid color", () => {
          const c = bandColor(render(() => 'green', 'underline'), Y0, Y1)
          assert(c.n > 0, "expected an inked underline")
          assert(c.g > c.r && c.g > c.b, "the underline should match the green fill")
        })

        test("unless textDecorationColor overrides", () => {
          const img = render(gradient, 'underline lime')
          const { lo, hi } = extent(img, Y0, Y1)
          const third = (hi - lo) / 3
          const left = bandColor(img, Y0, Y1, lo, lo + third)
          const right = bandColor(img, Y0, Y1, hi - third, hi)
          // solid lime everywhere — green-dominant on BOTH ends, no red→blue ramp
          assert(left.g > left.r && left.g > left.b, "the explicit color should win at the left, not the gradient")
          assert(right.g > right.r && right.g > right.b, "the explicit color should win at the right, not the gradient")
        })

        test("on a line-through too", () => {
          // the strike sits mid-glyph; sample it across the gaps in "l l l"
          const c = bandColor(render(gradient, 'line-through', 'l l l'), BASELINE-16, BASELINE-10)
          assert(c.n > 0, "expected an inked line-through")
          assert(c.r > 40 || c.b > 40, "the strike should carry the fill's color, not be solid black")
        })
      })
    })
  })

  describe("PDF export produces selectable text", {skip: hasPdftotext ? false : "pdftotext (poppler) not installed"}, () => {
    // pdftotext wraps RTL/bidi segments in Unicode bidi control characters (LRM/RLM,
    // LRE…RLO, isolates); strip them and collapse whitespace so we compare plain text.
    const BIDI_CONTROLS = /[‎‏‪-‮⁦-⁩]/g
    const normalize = s => s.replace(BIDI_CONTROLS, "").replace(/\s+/g, " ").trim()

    let counter = 0
    /**
     * @param {string} str
     * @param {{font?: string, width?: number, setup?: (ctx: import('../../lib').CanvasRenderingContext2D) => void}} [opts]
     */
    function pdfText(str, {font = "48px Helvetica", width = 600, setup} = {}){
      const canvas = new Canvas(width, 120)
      const ctx = canvas.getContext('2d')
      ctx.font = font
      if (setup) setup(ctx)
      ctx.fillText(str, 20, 70)

      const file = path.join(os.tmpdir(), `skia-canvas-pdf-${process.pid}-${counter++}.pdf`)
      fs.writeFileSync(file, canvas.toBufferSync('pdf'))
      try { return normalize(execFileSync('pdftotext', [file, '-']).toString()) }
      finally { try { fs.unlinkSync(file) } catch {} }
    }

    test("plain Latin round-trips", () => {
      assert.equal(pdfText("Hello World"), "Hello World")
    })

    test("ligatures recover source letters", () => {
      // e.g. an "fi"/"ffl" ligature glyph must copy back as "f","i" — the whole point of
      // attaching ActualText, since the font's reverse ToUnicode yields U+FB01/FB02.
      assert.equal(pdfText("Waffle fi office"), "Waffle fi office")
    })

    test("emoji ZWJ round-trips", () => {
      assert.equal(pdfText("👨‍👩‍👧 family"), "👨‍👩‍👧 family")
    })

    test("combining marks round-trip", () => {
      assert.equal(pdfText("café résumé"), "café résumé")
    })

    test("Arabic round-trips in logical order", () => {
      assert.equal(pdfText("مرحبا بالعالم"), "مرحبا بالعالم")
    })

    test("Hebrew round-trips in logical order", () => {
      assert.equal(pdfText("שלום עולם"), "שלום עולם")
    })

    test("mixed bidi recovers every segment", () => {
      // a reader may linearize mixed bidi in its own order; assert the content is all
      // present and correct rather than a specific concatenation order.
      const got = pdfText("abc مرحبا 123")
      for (const part of ["abc", "مرحبا", "123"]) {
        assert(got.includes(part), `expected ${JSON.stringify(part)} in ${JSON.stringify(got)}`)
      }
    })

    test("textured text stays selectable", () => {
      // a texture fill draws the visible glyphs as a stamped path (no text); an invisible
      // blob layer carries the utf-8 so the text is still selectable
      assert.equal(pdfText("textured", {setup: ctx => {
        ctx.fillStyle = ctx.createTexture([8, 8], {line: 2, color: 'black', angle: 0.4})
      }}), "textured")
    })

  })

  describe("SKIA_CANVAS_STRICT", () => {
    // the flag is read once when lib/classes/neon.js loads, so it can't be toggled in-process.
    // exercise every strict-mode rejection in a single child process (rather than one per case);
    // the child collects any mismatch, prints it to stderr, and exits non-zero.
    test("invalid values throw", () => {
      const {execFileSync} = require('node:child_process')
      const lib = require.resolve('../../lib')
      const child = `
        const {Canvas} = require(${JSON.stringify(lib)})
        const ctx = new Canvas(10, 10).getContext('2d')

        // [property, value, shouldThrow] — invalid values throw under strict; valid ones must not
        const cases = [
          // rejected by the JS parsers (css.js \`parsed\`)
          ['font',           'not-a-font',                      true],
          ['fontVariant',    'bogus',                           true],
          ['fontVariant',    'small-caps bogus',                true],
          ['fontStretch',    'bogus',                           true],
          ['letterSpacing',  'bogus',                           true],
          ['wordSpacing',    '2ex',                             true],
          ['textDecoration', 'notaword blue',                   true],
          ['filter',         'blur(2px)junk',                   true],
          ['filter',         'blur(2px) garbage(3)',            true],
          // rejected Rust-side via the ⚠️ canary (neon.js \`rustError\`)
          ['textDecoration', 'blahblah',                        true],
          ['textDecoration', 'underline notacolor',             true],
          // valid values must survive strict mode untouched
          ['fontVariant',    'small-caps',                      false],
          ['letterSpacing',  '1rem',                            false],
          ['textDecoration', 'line-through oklch(0.7 0.1 200)', false],
          ['filter',         'blur(2px) invert(50%)',           false],
        ]

        const fails = []
        for (const [prop, value, shouldThrow] of cases){
          let threw = null
          try { ctx[prop] = value } catch(e){ threw = e }
          if (shouldThrow && !threw)
            fails.push(prop + ' = ' + JSON.stringify(value) + ' should have thrown')
          else if (shouldThrow && !(threw instanceof TypeError))
            fails.push(prop + ' = ' + JSON.stringify(value) + ' threw ' + threw.constructor.name + ', expected TypeError')
          else if (!shouldThrow && threw)
            fails.push(prop + ' = ' + JSON.stringify(value) + ' should not have thrown (' + threw.message + ')')
        }
        if (fails.length){ console.error(fails.join('\\n')); process.exit(1) }
      `
      try {
        execFileSync(process.execPath, ['-e', child], {
          env: {...process.env, SKIA_CANVAS_STRICT: '1'},
          encoding: 'utf8',
          stdio: ['ignore', 'ignore', 'pipe'],
        })
      } catch (err) {
        assert.fail('strict-mode expectations failed:\n' + String(err.stderr || err.message).trim())
      }
    })
  })
})
