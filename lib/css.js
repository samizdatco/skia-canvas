"use strict"

//
// Parsers for properties that take CSS-style strings as values
//

// -- Font & Variant --------------------------------------------------------------------
//    https://developer.mozilla.org/en-US/docs/Web/CSS/font-variant
//    https://www.w3.org/TR/css-fonts-3/#font-size-prop

var splitBy = require('string-split-by'),
    {DOMPoint} = require('./geometry'),
    m, cache = {font:{}, variant:{}};

const styleRE = /^(normal|italic|oblique)$/,
      smallcapsRE = /^(normal|small-caps)$/,
      stretchRE = /^(normal|(semi-|extra-|ultra-)?(condensed|expanded))$/,
      namedSizeRE = /(?:xx?-)?small|smaller|medium|larger|(?:xx?-)?large|normal/,
      numSizeRE = /^([\d\.]+)(px|pt|pc|in|cm|mm|%|em|ex|ch|rem|q)/,
      namedWeightRE = /^(normal|bold(er)?|lighter)$/,
      numWeightRE = /^(1000|\d{1,3})$/,
      parameterizedRE = /([\w\-]+)\((.*?)\)/,
      unquote = s => s.replace(/^(['"])(.*?)\1$/, "$2"),
      isSize = s => namedSizeRE.test(s) || numSizeRE.test(s),
      isWeight = s => namedWeightRE.test(s) || numWeightRE.test(s);

function parseFont(str){
  if (cache.font[str]===undefined){
    try{
      if (typeof str !== 'string') throw new Error('Font specification must be a string')
      if (!str) throw new Error('Font specification cannot be an empty string')

      let font = {style:'normal', variant:'normal', weight:'normal', stretch:'normal'},
          value = str.replace(/\s*\/\*s/, "/"),
          tokens = splitBy(value, /\s+/),
          token;

      while (token = tokens.shift()) {
        let match = styleRE.test(token) ? 'style'
                  : smallcapsRE.test(token) ? 'variant'
                  : stretchRE.test(token) ? 'stretch'
                  : isWeight(token) ? 'weight'
                  : isSize(token) ? 'size'
                  : null;

        switch (match){
          case "style":
          case "variant":
          case "stretch":
          case "weight":
            font[match] = token
            break;

          case "size":
            // size is the pivot point between the style fields and the family name stack,
            // so start processing what's been collected
            let [emSize, leading] = splitBy(token, '/'),
                size = parseSize(emSize),
                lineHeight = parseSize((leading || '1.2').replace(/(\d)$/, '$1em'), size),
                weight = parseWeight(font.weight),
                family = splitBy(tokens.join(' '), /\s*,\s*/).map(unquote),
                features = font.variant=='small-caps' ? {on:['smcp', 'onum']} : {},
                {style, stretch, variant} = font;

            // make sure all the numeric fields have legitimate values
            let invalid = !isFinite(size) ? `font size "${emSize}"`
                        : !isFinite(lineHeight) ? `line height "${leading}"`
                        : !isFinite(weight) ? `font weight "${font.weight}"`
                        : family.length==0 ? `font family "${tokens.join(', ')}"`
                        : false;

            if (!invalid){
              // include a re-stringified version of the decoded/absified values
              return cache.font[str] = Object.assign(font, {
                size, lineHeight, weight, family, features,
                canonical:[
                  style,
                  (variant !== style) && variant,
                  ([variant, style].indexOf(weight) == -1) && weight,
                  ([variant, style, weight].indexOf(stretch) == -1) && stretch,
                  `${size}px/${lineHeight}px`,
                  family.map(nm => nm.match(/\s/) ? `"${nm}"` : nm).join(", ")
                ].filter(Boolean).join(' ')
              })
            }
            throw new Error(`Invalid ${invalid}`)

          default:
            throw new Error(`Unrecognized font attribute "${token}"`)
        }
      }
      throw new Error('Could not find a font size value')
    } catch(e) {
      // console.warn(Object.assign(e, {name:"Warning"}))
      cache.font[str] = null
    }
  }
  return cache.font[str]
}

function parseSize(str, emSize=16){
  if (m = numSizeRE.exec(str)){
    let [size, unit] = [parseFloat(m[1]), m[2]]
    return size * (unit == 'px' ? 1
                :  unit == 'pt' ? 1 / 0.75
                :  unit == '%' ? emSize / 100
                :  unit == 'pc' ? 16
                :  unit == 'in' ? 96
                :  unit == 'cm' ? 96.0 / 2.54
                :  unit == 'mm' ? 96.0 / 25.4
                :  unit == 'q' ? 96 / 25.4 / 4
                :  unit.match('r?em') ? emSize
                :  NaN )
  }

  if (m = namedSizeRE.exec(str)){
    return emSize * (sizeMap[m[0]] || 1.0)
  }

  return NaN
}

function parseFlexibleSize(str){
  if (m = numSizeRE.exec(str)){
    let [size, unit] = [parseFloat(m[1]), m[2]],
        px = size * (unit == 'px' ? 1
          :  unit == 'pt' ? 1 / 0.75
          :  unit == 'pc' ? 16
          :  unit == 'in' ? 96
          :  unit == 'cm' ? 96.0 / 2.54
          :  unit == 'mm' ? 96.0 / 25.4
          :  unit == 'q' ? 96 / 25.4 / 4
          :  NaN )
    return {size, unit, px}
  }
  return null
}

function parseStretch(str){
  return (m = stretchRE.exec(str)) ? m[0] : undefined
}

function parseWeight(str){
  return (m = numWeightRE.exec(str)) ? parseInt(m[0]) || NaN
       : (m = namedWeightRE.exec(str)) ? weightMap[m[0]]
       : NaN
}

function parseVariant(str){
  if (cache.variant[str]===undefined){
    let variants = [],
        features = {on:[], off:[]};

    for (let token of splitBy(str, /\s+/)){
      if (token == 'normal'){
        return {variants:[token], features:{on:[], off:[]}}
      }else if (token in featureMap){
        featureMap[token].forEach(feat => {
          if (feat[0] == '-') features.off.push(feat.slice(1))
          else features.on.push(feat)
        })
        variants.push(token);
      }else if (m = parameterizedRE.exec(token)){
        let subPattern = alternatesMap[m[1]],
            subValue = Math.max(0, Math.min(99, parseInt(m[2], 10))),
            [feat, val] = subPattern.replace(/##/, subValue < 10 ? '0'+subValue : subValue)
                             .replace(/#/, Math.min(9, subValue)).split(' ');
        if (typeof val=='undefined') features.on.push(feat)
        else features[feat] = parseInt(val, 10)
        variants.push(`${m[1]}(${subValue})`)
      }else{
        throw new Error(`Invalid font variant "${token}"`)
      }
    }

    cache.variant[str] = {variant:variants.join(' '), features:features};
  }

  return cache.variant[str];
}


function parseTextDecoration(str){
  let style = 'solid',
      line = 'none',
      color = 'currentColor',
      inherit = 'auto',
      thickness,
      _val
  str = str.trim().replace(/\s+/, ' ')
  for (const token of str.split(' ')){
    if (token.match(/solid|double|dotted|dashed|wavy/)) style = token
    else if (token.match(/none|initial|revert(-layer)?|unset/)) line = "none"
    else if (token.match(/underline|overline|line-through/)) line = token
    else if (_val = parseFlexibleSize(token)) thickness = _val
    else if (token.match(/auto|from-font/)) inherit = token
    else color = token
  }

  return { style, line, color, thickness, inherit, str }
}


// -- Window Types -----------------------------------------------------------------------
let cursorTypes = [
  "default", "crosshair", "hand", "arrow", "move", "text", "wait", "help", "progress", "not-allowed", "context-menu",
  "cell", "vertical-text", "alias", "copy", "no-drop", "grab", "grabbing", "all-scroll", "zoom-in", "zoom-out",
  "e-resize", "n-resize", "ne-resize", "nw-resize", "s-resize", "se-resize", "sw-resize", "w-resize", "ew-resize",
  "ns-resize", "nesw-resize", "nwse-resize", "col-resize", "row-resize", "none"
]
function parseCursor(str){
  return cursorTypes.includes(str)
}

function parseFit(mode){
  return ["none", "contain-x", "contain-y", "contain", "cover", "fill", "scale-down", "resize"].includes(mode)
}

// -- Corner Rounding
//    https://github.com/fserb/canvas2D/blob/master/spec/roundrect.md

function parseCornerRadii(r){
  r = [r].flat()
         .map(n => n instanceof DOMPoint ? n : new DOMPoint(n, n))
         .slice(0, 4)

  if (r.some(pt => !Number.isFinite(pt.x) || !Number.isFinite(pt.y))){
    return null // silently abort
  }else if (r.some(pt => pt.x < 0 || pt.y < 0)){
    throw new Error("Corner radius cannot be negative")
  }

  return r.length == 1 ? [r[0], r[0], r[0], r[0]]
       : r.length == 2 ? [r[0], r[1], r[0], r[1]]
       : r.length == 3 ? [r[0], r[1], r[2], r[1]]
       : r.length == 4 ? [r[0], r[1], r[2], r[3]]
       : [0, 0, 0, 0].map(n => new DOMPoint(n, n))
}

// -- Image Filters -----------------------------------------------------------------------
//    https://developer.mozilla.org/en-US/docs/Web/CSS/filter

var plainFilterRE = /(blur|hue-rotate|brightness|contrast|grayscale|invert|opacity|saturate|sepia)\((.*?)\)/,
    shadowFilterRE = /drop-shadow\((.*)\)/,
    percentValueRE = /^(\+|-)?\d+%$/,
    angleValueRE = /([\d\.]+)(deg|g?rad|turn)/;

function parseFilter(str){
  let filters = {}
  let canonical = []

  for (var spec of splitBy(str, /\s+/) || []){
    if (m = shadowFilterRE.exec(spec)){
      let kind = 'drop-shadow',
          args = m[1].trim().split(/\s+/),
          lengths = args.slice(0,3),
          color = args.slice(3).join(' '),
          dims = lengths.map(s => parseSize(s)).filter(isFinite);
      if (dims.length==3 && !!color){
        filters[kind] = [...dims, color]
        canonical.push(`${kind}(${lengths.join(' ')} ${color.replace(/ /g,'')})`)
      }
    }else if (m = plainFilterRE.exec(spec)){
      let [kind, arg] = m.slice(1)
      let val = kind=='blur' ? parseSize(arg)
              : kind=='hue-rotate' ? parseAngle(arg)
              : parsePercentage(arg);
      if (isFinite(val)){
        filters[kind] = val
        canonical.push(`${kind}(${arg.trim()})`)
      }
    }
  }

  return str.trim() == 'none' ? {canonical:'none', filters}
       : canonical.length ? {canonical:canonical.join(' '), filters}
       : null
}

function parsePercentage(str){
  return percentValueRE.test(str.trim()) ? parseInt(str, 10) / 100 : NaN
}

function parseAngle(str){
  if (m = angleValueRE.exec(str.trim())){
    let [amt, unit] = [parseFloat(m[1]), m[2]]
    return unit== 'deg' ? amt
         : unit== 'rad' ? 360 * amt / (2 * Math.PI)
         : unit=='grad' ? 360 * amt / 400
         : unit=='turn' ? 360 * amt
         : NaN
  }
}

//
// Font attribute keywords & corresponding values
//

const weightMap = {
  "lighter":300,
  "normal":400,
  "bold":700,
  "bolder":800
}

const sizeMap = {
  "xx-small":3/5,
  "x-small":3/4,
  "small":8/9,
  "smaller":8/9,
  "large":6/5,
  "larger":6/5,
  "x-large":3/2,
  "xx-large":2/1,
  "normal": 1.2 // special case for lineHeight
}

const featureMap = {
  "normal": [],

  // font-variant-ligatures
  "common-ligatures": ["liga", "clig"],
  "no-common-ligatures": ["-liga", "-clig"],
  "discretionary-ligatures": ["dlig"],
  "no-discretionary-ligatures": ["-dlig"],
  "historical-ligatures": ["hlig"],
  "no-historical-ligatures": ["-hlig"],
  "contextual": ["calt"],
  "no-contextual": ["-calt"],

  // font-variant-position
  "super": ["sups"],
  "sub": ["subs"],

  // font-variant-caps
  "small-caps": ["smcp"],
  "all-small-caps": ["c2sc", "smcp"],
  "petite-caps": ["pcap"],
  "all-petite-caps": ["c2pc", "pcap"],
  "unicase": ["unic"],
  "titling-caps": ["titl"],

  // font-variant-numeric
  "lining-nums": ["lnum"],
  "oldstyle-nums": ["onum"],
  "proportional-nums": ["pnum"],
  "tabular-nums": ["tnum"],
  "diagonal-fractions": ["frac"],
  "stacked-fractions": ["afrc"],
  "ordinal": ["ordn"],
  "slashed-zero": ["zero"],

  // font-variant-east-asian
  "jis78": ["jp78"],
  "jis83": ["jp83"],
  "jis90": ["jp90"],
  "jis04": ["jp04"],
  "simplified": ["smpl"],
  "traditional": ["trad"],
  "full-width": ["fwid"],
  "proportional-width": ["pwid"],
  "ruby": ["ruby"],

  // font-variant-alternates (non-parameterized)
  "historical-forms": ["hist"],
}

const alternatesMap = {
  "stylistic": "salt #",
  "styleset": "ss##",
  "character-variant": "cv##",
  "swash": "swsh #",
  "ornaments": "ornm #",
  "annotation": "nalt #",
}


module.exports = {
  font:parseFont,
  variant:parseVariant,
  size:parseSize,
  spacing:parseFlexibleSize,
  stretch:parseStretch,
  decoration:parseTextDecoration,
  filter:parseFilter,
  cursor:parseCursor,
  fit:parseFit,
  radii:parseCornerRadii,
}