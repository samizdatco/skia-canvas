//
// Font management & metrics
//

"use strict"

const {RustClass, readOnly, signature, inspect, REPR} = require('./neon'),
      {sync:globSync, hasMagic} = require('glob'),
      glob = paths => [paths].flat(2).map(pth => hasMagic(pth) ? globSync(pth) : pth).flat()

class FontLibrary extends RustClass {
  constructor(){
    super(FontLibrary)
  }

  get families(){ return this.prop('families') }

  has(familyName){ return this.ƒ('has', familyName) }

  family(name){ return this.ƒ('family', name) }

  use(...args){
    let sig = signature(args)
    if (sig=='o'){
      let results = {}
      for (let [alias, paths] of Object.entries(args.shift())){
        results[alias] = this.ƒ("addFamily", alias, glob(paths))
      }
      return results
    }else if (sig.match(/^s?[as]$/)){
      let fonts = glob(args.pop())
      let alias = args.shift()
      return this.ƒ("addFamily", alias, fonts)
    }else{
      throw new Error("Expected an array of file paths or an object mapping family names to font files")
    }
  }

  reset(){ return this.ƒ('reset') }
}

class TextMetrics{
  constructor([
    width, left, right, ascent, descent,
    fontAscent, fontDescent, emAscent, emDescent,
    hanging, alphabetic, ideographic
  ], lines){
    readOnly(this, "width", width)
    readOnly(this, "actualBoundingBoxLeft", left)
    readOnly(this, "actualBoundingBoxRight", right)
    readOnly(this, "actualBoundingBoxAscent", ascent)
    readOnly(this, "actualBoundingBoxDescent", descent)
    readOnly(this, "fontBoundingBoxAscent", fontAscent)
    readOnly(this, "fontBoundingBoxDescent", fontDescent)
    readOnly(this, "emHeightAscent", emAscent)
    readOnly(this, "emHeightDescent", emDescent)
    readOnly(this, "hangingBaseline", hanging)
    readOnly(this, "alphabeticBaseline", alphabetic)
    readOnly(this, "ideographicBaseline", ideographic)
    readOnly(this, "lines", lines.map( ([x, y, width, height, baseline, startIndex, endIndex]) => (
      {x, y, width, height, baseline, startIndex, endIndex}
    )))
  }
}


module.exports = {FontLibrary:new FontLibrary(), TextMetrics}
