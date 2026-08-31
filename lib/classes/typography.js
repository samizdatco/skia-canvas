//
// Font management & metrics
//

"use strict"

const {RustClass, readOnly, signature, inspect, REPR} = require('./neon')

// human-readable names for the registered OpenType feature tags (from the OT 1.9.1 registry)
// see: <https://learn.microsoft.com/typography/opentype/spec/featurelist>
const FEATURE_LABELS = {
  aalt:"Access All Alternates", abvf:"Above-base Forms", abvm:"Above-base Mark Positioning",
  abvs:"Above-base Substitutions", afrc:"Alternative Fractions", akhn:"Akhand",
  apkn:"Kerning for Alternate Proportional Widths", blwf:"Below-base Forms",
  blwm:"Below-base Mark Positioning", blws:"Below-base Substitutions", calt:"Contextual Alternates",
  case:"Case-Sensitive Forms", ccmp:"Glyph Composition / Decomposition", cfar:"Conjunct Form After Ro",
  chws:"Contextual Half-width Spacing", cjct:"Conjunct Forms", clig:"Contextual Ligatures",
  cpct:"Centered CJK Punctuation", cpsp:"Capital Spacing", cswh:"Contextual Swash",
  curs:"Cursive Positioning", c2pc:"Petite Capitals From Capitals", c2sc:"Small Capitals From Capitals",
  dist:"Distances", dlig:"Discretionary Ligatures", dnom:"Denominators", dtls:"Dotless Forms",
  expt:"Expert Forms", falt:"Final Glyph on Line Alternates", fin2:"Terminal Forms #2",
  fin3:"Terminal Forms #3", fina:"Terminal Forms", flac:"Flattened Accent Forms", frac:"Fractions",
  fwid:"Full Widths", half:"Half Forms", haln:"Halant Forms", halt:"Alternate Half Widths",
  hist:"Historical Forms", hkna:"Horizontal Kana Alternates", hlig:"Historical Ligatures",
  hngl:"Hangul", hojo:"Hojo Kanji Forms", hwid:"Half Widths", init:"Initial Forms",
  isol:"Isolated Forms", ital:"Italics", jalt:"Justification Alternates", jp78:"JIS78 Forms",
  jp83:"JIS83 Forms", jp90:"JIS90 Forms", jp04:"JIS2004 Forms", kern:"Kerning", lfbd:"Left Bounds",
  liga:"Standard Ligatures", ljmo:"Leading Jamo Forms", lnum:"Lining Figures", locl:"Localized Forms",
  ltra:"Left-to-right Alternates", ltrm:"Left-to-right Mirrored Forms", mark:"Mark Positioning",
  med2:"Medial Forms #2", medi:"Medial Forms", mgrk:"Mathematical Greek", mkmk:"Mark to Mark Positioning",
  mset:"Mark Positioning via Substitution", nalt:"Alternate Annotation Forms", nlck:"NLC Kanji Forms",
  nukt:"Nukta Forms", numr:"Numerators", onum:"Oldstyle Figures", opbd:"Optical Bounds",
  ordn:"Ordinals", ornm:"Ornaments", palt:"Proportional Alternate Widths", pcap:"Petite Capitals",
  pkna:"Proportional Kana", pnum:"Proportional Figures", pref:"Pre-base Forms", pres:"Pre-base Substitutions",
  pstf:"Post-base Forms", psts:"Post-base Substitutions", pwid:"Proportional Widths", qwid:"Quarter Widths",
  rand:"Randomize", rclt:"Required Contextual Alternates", rkrf:"Rakar Forms", rlig:"Required Ligatures",
  rphf:"Reph Form", rtbd:"Right Bounds", rtla:"Right-to-left Alternates", rtlm:"Right-to-left Mirrored Forms",
  ruby:"Ruby Notation Forms", rvrn:"Required Variation Alternates", salt:"Stylistic Alternates",
  sinf:"Scientific Inferiors", size:"Optical size", smcp:"Small Capitals", smpl:"Simplified Forms",
  ssty:"Math Script-style Alternates", stch:"Stretching Glyph Decomposition", subs:"Subscript",
  sups:"Superscript", swsh:"Swash", titl:"Titling", tjmo:"Trailing Jamo Forms", tnam:"Traditional Name Forms",
  tnum:"Tabular Figures", trad:"Traditional Forms", twid:"Third Widths", unic:"Unicase",
  valt:"Alternate Vertical Metrics", vapk:"Kerning for Alternate Proportional Vertical Metrics",
  vatu:"Vattu Variants", vchw:"Vertical Contextual Half-width Spacing", vert:"Vertical Alternates",
  vhal:"Alternate Vertical Half Metrics", vjmo:"Vowel Jamo Forms", vkna:"Vertical Kana Alternates",
  vkrn:"Vertical Kerning", vpal:"Proportional Alternate Vertical Metrics", vrt2:"Vertical Alternates and Rotation",
  vrtr:"Vertical Alternates for Rotation", zero:"Slashed Zero",
}

// whether features accept a 0/1..N index value rather than just a 0/1 toggle
const INDEXED_FEATURES = new Set(['aalt', 'salt', 'nalt', 'swsh', 'cswh', 'ornm'])

// add arg-type fields and missing feature labels to a Font or FontFamily
function expandFontFeatures(font){
  if (!font) return font

  let features = {}
  for (let tag of Object.keys(font.features)){
    let ss = /^ss([0-9][0-9])$/.exec(tag),
        cv = /^cv([0-9][0-9])$/.exec(tag),
        type = INDEXED_FEATURES.has(tag) || cv ? 'indexed' : 'on/off',
        label = font.features[tag] || FEATURE_LABELS[tag] ||
                (ss && `Stylistic Set ${parseInt(ss[1], 10)}`) ||
                (cv && `Character Variant ${parseInt(cv[1], 10)}`)

    features[tag] = label ? {label, type} : {type}
  }

  return Object.assign(font, {features})
}

class FontLibrary extends RustClass {
  constructor(){
    super(FontLibrary)
  }

  get families(){ return this.prop('families') }

  has(familyName){ return this.ƒ('has', familyName) }

  family(name){ return expandFontFeatures(this.ƒ('family', name)) }

  use(...args){
    let sig = signature(args)
    if (sig=='o'){
      let results = {}
      for (let [alias, paths] of Object.entries(args.shift())){
        results[alias] = this.ƒ("addFamily", alias, [paths].flat()).map(expandFontFeatures)
      }
      return results
    }else if (sig.match(/^s?[as]$/)){
      let fonts = [args.pop()].flat()
      let alias = args.shift()
      return this.ƒ("addFamily", alias, fonts).map(expandFontFeatures)
    }else{
      throw new Error("Expected an array of file paths or an object mapping family names to font files")
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
