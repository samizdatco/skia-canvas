//
// Font management & metrics
//

"use strict"

const {RustClass, readOnly, signature, inspect, REPR} = require('./neon')

const fontSource = source => {
  if (typeof source == 'string') return {path:source, index:0}

  let {path, namedInstance} = source || {}
  if (typeof path != 'string' ||
      !Number.isSafeInteger(namedInstance) ||
      namedInstance < 1 || namedInstance > 0x7fff){
    throw new Error("Expected a font path or a named font instance")
  }

  // Skia follows FreeType's convention of storing the one-based named-instance
  // index in the upper 16 bits of the collection index.
  return {path, index:namedInstance << 16}
}

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
      for (let [alias, sources] of Object.entries(args.shift())){
        let fonts = [sources].flat().map(fontSource)
        results[alias] = this.ƒ("addFamily", alias, fonts.map(({path}) => path), fonts.map(({index}) => index))
      }
      return results
    }else if (sig.match(/^s?[as]$/) || sig=='so'){
      let fonts = [args.pop()].flat().map(fontSource)
      let alias = args.shift()
      return this.ƒ("addFamily", alias, fonts.map(({path}) => path), fonts.map(({index}) => index))
    }else{
      throw new Error("Expected font sources or an object mapping family names to font sources")
    }
  }

  reset(){ return this.ƒ('reset') }
}

class TextMetrics{
  constructor(metrics){
    for (let k in metrics) readOnly(this, k, metrics[k])
  }
}


module.exports = {FontLibrary:new FontLibrary(), TextMetrics}
