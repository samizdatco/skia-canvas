"use strict"

//
// Parsers for properties that take CSS-style strings as values
//

// -- Font & Variant --------------------------------------------------------------------
//    https://developer.mozilla.org/en-US/docs/Web/CSS/font-variant
//    https://www.w3.org/TR/css-fonts-3/#font-size-prop

var {parse, stringify} = require('css-font'),
    {weightMap, sizeMap, featureMap, alternatesMap} = require('./css');

var m, cache = {font:{}, variant:{}},
    featuresRE = new RegExp(`(?<= )(${Object.keys(featureMap).join('|')})(?= )`, 'ig'),
    alternatesRE = new RegExp(`(?<= )(${Object.keys(alternatesMap).join('|')})\\(([0-9]+)\\)(?= )`, 'ig'),
    normalRE = / normal |^\s*$/i,
    namedSizeRE = /(?:x?x-)?small|smaller|medium|larger|(?:x?x-)?large|normal/,
    numSizeRE =  /([\d\.]+)(px|pt|pc|in|cm|mm|%|em|ex|ch|rem|q)/,
    namedWeightRE = /normal|bold(er)?|lighter/,
    numWeightRE =  /\d{3}/;

function parseFont(str){
  if (cache.font[str]===undefined){
    try{ var info = parse(str)
      let canonical = stringify(info),
      size = parseSize(info.size),
      weight = parseWeight(info.weight),
      leading = parseSize(info.lineHeight, size),
      features = info.variant=='small-caps' ? {on:['smcp', 'onum']} : {};
      let invalid = !isFinite(size) ? `font size "${info.size}"`
                  : !isFinite(leading) ? `line height "${info.lineHeight}"`
                  : !isFinite(weight) ? `font weight "${info.weight}"`
                  : false;
      if (invalid) throw new Error(`Invalid ${invalid}`)
      else cache.font[str] = Object.assign(info, {size, weight, leading, features, canonical})
    } catch(e){ return console.warn(Object.assign(e, {name:"Warning"})) }
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

function parseWeight(str){
  return (m = numWeightRE.exec(str)) ? parseInt(m[0])
       : (m = namedWeightRE.exec(str)) ? weightMap[m[0]]
       : NaN
}

function parseVariant(str){
  if (cache.variant[str]===undefined){
    let raw = ` ${str} `,
        variants = [],
        features = {on:[], off:[]};

    if (normalRE.exec(raw)){
      variants = ['normal'];
    }else{
      for (const match of raw.matchAll(featuresRE)){
        featureMap[match[1]].forEach(feat => {
          if (feat[0] == '-') features.off.push(feat.slice(1))
          else features.on.push(feat)
        })
        variants.push(match[1]);
      }

      for (const match of raw.matchAll(alternatesRE)){
        let subPattern = alternatesMap[match[1]],
            subValue = Math.max(0, Math.min(99, parseInt(match[2], 10))),
            [feat, val] = subPattern.replace(/##/, subValue < 10 ? '0'+subValue : subValue)
                             .replace(/#/, Math.min(9, subValue)).split(' ');
        if (typeof val=='undefined') features.on.push(feat)
        else features[feat] = parseInt(val, 10)
        variants.push(`${match[1]}(${subValue})`)
      }
    }

    cache.variant[str] = {variant:variants.join(' '), features:features};
  }

  return cache.variant[str];
}

// -- Image Filters -----------------------------------------------------------------------
//    https://developer.mozilla.org/en-US/docs/Web/CSS/filter

var m, filterParam = `\\(([^\\)]*(?:\\s*\\))?)\\s*\\)`,
    allFiltersRE = new RegExp(`[a-z\-]+${filterParam}`, 'g'),
    shadowFilterRE = new RegExp(`drop-shadow${filterParam}`),
    plainFilterRE = /(blur|hue-rotate|brightness|contrast|grayscale|invert|opacity|saturate|sepia)\((.*?)\)/,
    percentValueRE = /^(\+|-)?\d{1,3}%$/,
    angleValueRE = /([\d\.]+)(deg|g?rad|turn)/;

function parseFilter(str){
  let filters = {}
  let canonical = []

  for (var spec of str.match(allFiltersRE) || []){
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

module.exports = {parseFont, parseVariant, parseSize, parseFilter}