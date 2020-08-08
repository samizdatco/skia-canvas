const convertColor = require('color-convert'),
      {CanvasGradient, CanvasPattern} = require('../native');

function parseColorString(clr){
  var m

  // color-spaced names: rgba(), hsl(), etc.
  if (m = /^((?:rgb|hs[lv]|cmyk|xyz|lab)a?)\s*\(([^\)]*)\)/.exec(clr)) {
    let name = m[1],
        base = name.replace(/a$/, ''),
        size = base === 'cmyk' ? 4 : 3,
        parts = m[2].split(/,/)
                .map((c,i) => parseFloat(c) / (c.match(/%\s*$/) && i == size ? 100 : 1))
                .filter(c => !isNaN(c));
    if (parts.length < size) return null

    let alpha = isNaN(parts[size]) ? 1 : parts[size]
    parts.splice(size)

    let rgb = base=='rgb' ? parts : convertColor[base].rgb(parts);
    return [...rgb.map(c => c/255), alpha]
  }

  // hex strings: #(rgb|rgba|rrggbb|rrggbbaa)
  if (m = /^#?([a-f0-9]{3,8})$/i.exec(clr)) {
    let base = m[1],
        size = base.length,
        parts = (size == 5 || size == 7) ? null
              : base.split(size < 6 ? /(.)/ : /(..)/)
                    .filter(Boolean)
                    .map(s => parseInt(s + (size < 6 ? s : ''), 16) / 255);
    if (isNaN(parts[3])) parts[3] = 1
    return parts
  }

  // named css-colors
  let rgb = convertColor.keyword.rgb(clr)
  return rgb ? [...rgb.map(c => c/255), 1] : null
}

function toColor(clr){
  if (clr instanceof CanvasGradient) return [clr]
  if (clr instanceof CanvasPattern) return [clr]

  let clrString = (typeof clr == 'string') ? clr
                : (clr && typeof clr['toString']=='function') ? clr.toString()
                : null;
  return parseColorString(clrString)
}

function fromColor(components){
  // pass gradient and pattern references through without unpacking them
  if (!Array.isArray(components) || components.length<4) return components

  let a = components.slice().pop()
  let [r,g,b] = components.map(c => Math.floor(c*255))
  return a == 1 ? `#${convertColor.rgb.hex(r, g, b).toLowerCase()}`
       : `rgba(${r},${g},${b},${a.toFixed(3).replace(/0*$/, '').replace(/\.$/, '')})`
}


module.exports = {toColor, fromColor}