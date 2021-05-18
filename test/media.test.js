const _ = require('lodash'),
      fs = require('fs'),
      glob = require('glob').sync,
      {Image, FontLibrary, loadImage} = require('../lib'),
      {parseFont} = require('../lib/parse'),
      simple = require('simple-get')

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

    expect(() => FontLibrary.use(alias, ttf)).not.toThrow()
    expect(FontLibrary.has(alias)).toBe(true)
    expect(FontLibrary.family(alias).weights).toContain(400)
  })
})

