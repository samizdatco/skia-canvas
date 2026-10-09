//
// Parsers for properties that take CSS-style strings as values
//

"use strict"

const {STRICT} = require('./neon')

// -- Strict Mode -----------------------------------------------------------------------

// silently ignore arg-validation errors unless SKIA_CANVAS_STRICT is enabled
function parsed(value, label, raw){
  if (value == null && STRICT){
    let error = new TypeError(`Invalid ${label}: ${JSON.stringify(raw)}`)
    Error.captureStackTrace(error, parsed)
    throw error
  }
  return value
}

// -- Font & Variant --------------------------------------------------------------------
//    https://developer.mozilla.org/en-US/docs/Web/CSS/font-variant
//    https://www.w3.org/TR/css-fonts-3/#font-size-prop

var m, cache = {font:{}, variant:{}};

const styleRE = /^(normal|italic|oblique)$/,
      angleRE = /^[+-]?\d+(?:\.\d+)?deg$/, // oblique slant angle (deg only)
      smallcapsRE = /^(normal|small-caps)$/,
      stretchRE = /^(normal|(semi-|extra-|ultra-)?(condensed|expanded))$/,
      kerningRE = /^(auto|normal|none)$/,
      openTypeTagRE = /^[\x20-\x7e]{4}$/, // opentype 4-char tag (in printable-ascii range)
      genericFamilyRE = /^(serif|sans-serif|monospace|cursive|fantasy|system-ui|ui-(serif|sans-serif|monospace|rounded)|math|emoji|fangsong)$/i,
      namedSizeRE = /^((?:xx?-)?small|smaller|medium|larger|(?:xx?-)?large|normal)$/,
      numSizeRE = /^([+-]?(?:\d+(?:\.\d+)?|\.\d+))(px|pt|pc|in|cm|mm|%|em|rem|q)$/, // omit `ex` & `ch`
      zeroValueRE = /^[+-]?(?:0+(?:\.0+)?|\.0+)$/, // a unitless zero is a valid <length>/<angle>
      lengthValueRE = /^[+-]?(?:(?:\d+(?:\.\d+)?|\.\d+)(?:px|pt|pc|in|cm|mm|q|r?em)|0+(?:\.0+)?|\.0+)$/, // <length>: like numSizeRE but no `%`, plus unitless 0
      namedWeightRE = /^(normal|bold(er)?|lighter)$/,
      numWeightRE = /^(1000|\d{1,3})$/,
      parameterizedRE = /([\w\-]+)\((.*?)\)/,
      percentRE = /^\d+(\.\d+)?%$/,
      unquote = s => s.replace(/^(['"])(.*?)\1$/, "$2"),
      isSize = s => { let [size] = s.split('/'); return namedSizeRE.test(size) || numSizeRE.test(size) },
      isWeight = s => namedWeightRE.test(s) || numWeightRE.test(s),
      splitBy = (str, sep) => {
        // split on `sep`, but not when the separator is nested within (), "", or ''
        let re = new RegExp(`\\s*${sep.replace(' ', '\\s')}\\s*`, 'y'),
            parts = [], start = 0, depth = 0, quote = null, i = 0
        while (i < str.length){
          let ch = str[i]
          if (quote){ if (ch === quote) quote = null; i++ }
          else if (ch === '"' || ch === "'"){ quote = ch; i++ }
          else if (ch === '('){ depth++; i++ }
          else if (ch === ')'){ if (depth > 0) depth--; i++ }
          else if (depth === 0 && (re.lastIndex = i, re.test(str)) && re.lastIndex > i){
            parts.push(str.slice(start, i)); start = i = re.lastIndex
          }
          else i++
        }
        parts.push(str.slice(start))
        return parts.filter(Boolean)
      }

function parseFont(str){
  if (cache.font[str]===undefined){
    try{
      if (!str) throw new Error('Font specification cannot be an empty string')

      let font = {style:'normal', variant:'normal', weight:'normal', stretch:'normal'},
          value = str.replace(/\s*\/\*s/, "/"),
          tokens = splitBy(value, ' '),
          token;

      // keywords & units are case-insensitive (but font family names preserve their case)
      while (token = tokens.shift()?.toLowerCase()) {
        if (token === 'normal') continue; // ignore normal (just reaffirms the default value)

        let match = styleRE.test(token) ? 'style'
                  : angleRE.test(token) && font.style === 'oblique' && font.obliqueAngle === undefined ? 'angle'
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

          case "angle":
            // clamp `oblique <angle>` to CSS's ±90° range
            font.obliqueAngle = Math.max(-90, Math.min(90, parseFloat(token)))
            break;

          case "size":
            // size is the pivot point between the style fields and the family name stack,
            // so start processing what's been collected
            let [emSize, leading] = token.split('/'),
                size = parseSize(emSize),
                lineHeight = leading ? parseSize(leading.replace(/(\d)$/, '$1em'), size) : undefined,
                weight = parseWeight(font.weight),
                family = splitBy(tokens.join(' '), ',').map(nm => genericFamilyRE.test(nm) ? nm.toLowerCase() : unquote(nm)),
                features = font.variant=='small-caps' ? {on:['smcp', 'onum']} : {},
                {style, stretch, variant, obliqueAngle} = font;

            // make sure all the numeric fields have legitimate values
            let invalid = !isFinite(size) ? `font size "${emSize}"`
                        : !isFinite(lineHeight) && lineHeight!==undefined ? `line height "${leading}"`
                        : !isFinite(weight) ? `font weight "${font.weight}"`
                        : family.length==0 ? `font family "${tokens.join(', ')}"`
                        : false;

            if (!invalid){
              // include a re-stringified version of the decoded/absified values
              return cache.font[str] = Object.assign(font, {
                size, lineHeight, weight, family, features,
                canonical:[
                  obliqueAngle!==undefined ? `${style} ${obliqueAngle}deg` : style,
                  (variant !== style) && variant,
                  ![variant, style].includes(weight) && weight,
                  ![variant, style, weight].includes(stretch) && stretch,
                  `${size}px${isFinite(lineHeight) ? `/${lineHeight}px`: ''}`,
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
  str = `${str}`.trim().toLowerCase()

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

  if (zeroValueRE.test(str)) return 0

  if (m = namedSizeRE.exec(str)){
    return emSize * (sizeMap[m[0]] || 1.0)
  }

  return NaN
}

function parseFlexibleSize(str){
  str = `${str}`.trim().toLowerCase()

  if (m = numSizeRE.exec(str)){
    let [size, unit] = [parseFloat(m[1]), m[2]]

    // em & rem are converted in rust (em vs font size, rem vs 16px), so bail with a placeholder px value
    if (unit == 'em' || unit == 'rem') return {size, unit, px:0}

    // everything else can convert to px here, or get passed to rust as `null` indicating an invalid arg
    let px = size * (unit == 'px' ? 1
          :  unit == 'pt' ? 1 / 0.75
          :  unit == 'pc' ? 16
          :  unit == 'in' ? 96
          :  unit == 'cm' ? 96.0 / 2.54
          :  unit == 'mm' ? 96.0 / 25.4
          :  unit == 'q' ? 96 / 25.4 / 4
          :  NaN )
    return isNaN(px) ? null : {size, unit, px}
  }

  if (zeroValueRE.test(str)) return {size:0, unit:'px', px:0}

  return null
}

function parseStretch(str){
  str = str.trim()
  if (stretchRE.test(str)) return str
  else if (percentRE.test(str)) return +(str.slice(0,-1))
}

function parseKerning(str){
  str = str.trim()
  if (kerningRE.test(str)) return str
}

// parse a CSS tag-list (fontVariationSettings or fontFeatureSettings) into a dict.
function parseTagList(str, parseValue){
  if (typeof str != 'string') return null
  str = str.trim()
  if (str == '' || str.toLowerCase() == 'normal') return {}

  let tags = new Map()
  for (let entry of splitBy(str, ',')){
    let parts = splitBy(entry.trim(), ' ')
    if (!parts.length) return null

    let tag = unquote(parts[0])
    if (parts[0] == tag || !openTypeTagRE.test(tag)) return null // reject unquoted / non-4-char tags

    let value = parseValue(parts)
    if (value == null) return null // reject the whole string on a malformed value

    tags.set(tag, value)
  }

  return Object.fromEntries(tags)
}

// parse a {tag: value} object into a dict, rejecting it entirely if any tag or value is invalid
function parseTagDict(obj, parseValue){
  let proto = obj ? Object.getPrototypeOf(obj) : undefined
  if (proto !== null && proto !== Object.prototype) return null // plain objects only (not Maps, Arrays, etc.)

  let tags = {}
  for (let [tag, raw] of Object.entries(obj)){
    let value = parseValue(raw)
    if (!openTypeTagRE.test(tag) || value == null) return null
    tags[tag] = value
  }
  return tags
}

function parseVariationSettings(spec){
  // values are mandatory and must be finite numbers
  if (spec instanceof Object && spec.toString !== Object.prototype.toString) spec = `${spec}`
  if (typeof spec == 'object') return parseTagDict(spec, value => Number.isFinite(value) ? value : null)
  return parseTagList(spec, parts => {
    if (parts.length != 2) return null
    let value = Number(parts[1])
    return isFinite(value) ? value : null
  })
}

function parseFeatureSettings(spec){
  // values are optional and must be non-negative integers or `on`/`off` keywords (or boolean, in object form)
  if (spec instanceof Object && spec.toString !== Object.prototype.toString) spec = `${spec}`
  if (typeof spec == 'object') return parseTagDict(spec, value =>
    typeof value == 'boolean' ? +value : Number.isInteger(value) && value >= 0 ? value : null
  )
  return parseTagList(spec, parts => {
    if (parts.length > 2) return null
    let raw = parts.length == 2 ? parts[1].toLowerCase() : 'on'
    let value = raw == 'on' ? 1 : raw == 'off' ? 0 : parseInt(raw, 10)
    return Number.isInteger(value) && value >= 0 ? value : null
  })
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

    for (let token of splitBy(str.toLowerCase(), ' ')){
      if (token == 'normal'){
        return {variant:token, features:{on:[], off:[]}}
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
        return null // reject the whole string if any token is inrecognized
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

  str = str.trim().replace(/\s+/g, ' ').toLowerCase()
  let colors = 0
  for (const token of splitBy(str, ' ')){
    if (token.match(/solid|double|dotted|dashed|wavy/)) style = token
    else if (token.match(/none|initial|revert(-layer)?|unset/)) line = "none"
    else if (token.match(/underline|overline|line-through/)) line = token
    else if (_val = parseFlexibleSize(token)) thickness = _val
    else if (token.match(/auto|from-font/)) inherit = token
    else { color = token; colors++ }
  }

  if (colors > 1) return null // non-matched strings are presumed to be colors (so >1 is invalid)

  return { style, line, color, thickness, inherit, str }
}


// -- Window Types -----------------------------------------------------------------------

let cursorTypes = [
  "default", "none", "context-menu", "help", "pointer", "progress", "wait", "cell", "crosshair",
  "text", "vertical-text", "alias", "copy", "move", "no-drop", "not-allowed", "grab", "grabbing",
  "e-resize", "n-resize", "ne-resize", "nw-resize", "s-resize", "se-resize", "sw-resize", "w-resize",
  "ew-resize", "ns-resize", "nesw-resize", "nwse-resize", "col-resize", "row-resize", "all-scroll",
  "zoom-in", "zoom-out",
]

function parseCursor(str){
  str = `${str}`.toLowerCase()
  return cursorTypes.includes(str) && str
}

function parseFit(mode){
  mode = `${mode}`.toLowerCase()
  return ["none", "contain-x", "contain-y", "contain", "cover", "fill", "scale-down", "resize"].includes(mode) && mode
}

// -- Corner Rounding
//    https://github.com/fserb/canvas2D/blob/master/spec/roundrect.md

function parseCornerRadii(r){
  r = [r].flat()
         .slice(0, 4)
         .map(n => n && Object.hasOwn(n, 'x') && Object.hasOwn(n, 'y') ? n : {x:n, y:n})

  if (r.some(pt => !Number.isFinite(pt.x) || !Number.isFinite(pt.y))){
    return null // silently abort
  }else if (r.some(pt => pt.x < 0 || pt.y < 0)){
    throw RangeError("Corner radius cannot be negative")
  }

  return r.length == 1 ? [r[0], r[0], r[0], r[0]]
       : r.length == 2 ? [r[0], r[1], r[0], r[1]]
       : r.length == 3 ? [r[0], r[1], r[2], r[1]]
       : r.length == 4 ? [r[0], r[1], r[2], r[3]]
       : [0, 0, 0, 0].map(n => ({x:n, y:n}))
}

// -- Image Filters -----------------------------------------------------------------------
//    https://developer.mozilla.org/en-US/docs/Web/CSS/filter

var plainFilterRE = /^(blur|hue-rotate|brightness|contrast|grayscale|invert|opacity|saturate|sepia)\((.*?)\)$/s,
    shadowFilterRE = /^drop-shadow\((.*)\)$/s,
    percentValueRE = /^[+-]?(?:\d+(?:\.\d+)?|\.\d+)%$/,
    numberValueRE = /^[+-]?(?:\d+(?:\.\d+)?|\.\d+)$/,
    angleValueRE = /^([+-]?(?:\d+(?:\.\d+)?|\.\d+))(deg|g?rad|turn)$/;

function parseFilter(str, emSize){
  str = str.toLowerCase() // function names, units, & colors are case-insensitive
  if (str.trim() == 'none') return {canonical:'none', filters:{}}

  let filters = {}
  let canonical = []

  for (var spec of splitBy(str, ' ')){
    if (m = shadowFilterRE.exec(spec)){
      // grammar is `<color>? && <length>{2,3}`, so the color can lead or trail, the blur radius is optional,
      // and an omitted color means opaque black
      let kind = 'drop-shadow',
          args = splitBy(m[1], ' '),
          dims = [], spots = [], color = null, surplus = false;

      args.forEach((arg, i) => {
        // resolve `em` units against current font size but reject `%` or named relative sizes
        let size = lengthValueRE.test(arg) ? parseSize(arg, emSize) : NaN
        if (isFinite(size)) { dims.push(size); spots.push(i) }
        else if (color === null) color = arg
        else surplus = true
      })

      // the offsets may be negative, but not the blur radius
      let contiguous = spots.length && spots[spots.length-1] - spots[0] == spots.length - 1
      if (!surplus && contiguous && dims.length >= 2 && dims.length <= 3 && !(dims[2] < 0)){
        filters[kind] = [dims[0], dims[1], dims[2] ?? 0, color ?? 'black']
        canonical.push(`${kind}(${args.join(' ')})`)
        continue
      }
    }else if (m = plainFilterRE.exec(spec)){
      let [kind, arg] = m.slice(1).map(s => s.trim())
      let val = kind=='blur' ? (lengthValueRE.test(arg) ? parseSize(arg, emSize) : NaN)
              : kind=='hue-rotate' ? parseAngle(arg)
              : parsePercentage(arg);
      // only the hue-rotate angle can be negative
      if (isFinite(val) && (kind=='hue-rotate' || val >= 0)){
        filters[kind] = val
        canonical.push(`${kind}(${arg})`)
        continue
      }
    }

    // any token that didn't match invalidates the whole value
    return null
  }

  return canonical.length ? {canonical:canonical.join(' '), filters} : null
}

function parsePercentage(str){
  return percentValueRE.test(str.trim()) ? parseFloat(str) / 100
       : numberValueRE.test(str.trim()) ? parseFloat(str)
       : NaN
}

function parseAngle(str){
  str = `${str}`.trim().toLowerCase()

  if (zeroValueRE.test(str)) return 0

  if (m = angleValueRE.exec(str)){
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
  // parsed in js (w/ strict-mode checking)
  font:              str => parsed(parseFont(str),              'font',            str),
  spacing:           str => parsed(parseFlexibleSize(str),      'spacing',         str),
  stretch:           str => parsed(parseStretch(str),           'font stretch',    str),
  kerning:           str => parsed(parseKerning(str),           'font kerning',    str),
  decoration:        str => parsed(parseTextDecoration(str),    'text decoration', str),
  filter:            (str, em) => parsed(parseFilter(str, em),  'filter',          str),
  variant:           str => parsed(parseVariant(str),           'font variant',    str),
  variationSettings: str => parsed(parseVariationSettings(str), 'font variation settings', str),
  featureSettings:   str => parsed(parseFeatureSettings(str),   'font feature settings', str),
  instance: str => parsed(typeof str == 'string' ? str : null, 'font instance', str),

  // parsed by rust
  size:parseSize,

  // path & context
  radii:parseCornerRadii,

  // gui
  cursor:parseCursor,
  fit:parseFit,
}
