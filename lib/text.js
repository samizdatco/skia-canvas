"use strict"

const font = require('css-font')

function parseSize(str, stdSize=16){
  let sizes = {
        // proportions from https://www.w3.org/TR/css-fonts-3/#font-size-prop
        "xx-small":3/5, "x-small":3/4, "small":8/9, "smaller":8/9,
        "large":6/5, "larger":6/5, "x-large":3/2, "xx-large":2/1,
      },
      numSize =  /([\d\.]+)(px|pt|pc|in|cm|mm|%|em|ex|ch|rem|q)/,
      namedSize = /(?:x?x-)?small|smaller|medium|larger|(?:x?x-)?large/,
      m;

  if (m = numSize.exec(str)){
    let [size, unit] = [parseFloat(m[1]), m[2]]
    return size * (unit == 'pt' ? 1 / 0.75
                :  unit == '%' ? stdSize / 100
                :  unit == 'pc' ? 16
                :  unit == 'in' ? 96
                :  unit == 'cm' ? 96.0 / 2.54
                :  unit == 'mm' ? 96.0 / 25.4
                :  unit == 'q' ? 96 / 25.4 / 4
                :  unit.match('r?em') ? stdSize
                :  1.0 )
  }

  if (m = namedSize.exec(str)){
    return stdSize * (sizes[m[0]] || 1.0)
  }

  return null
}

function parseFont(str){
  try{
    var info = font.parse(str)
  }catch(e){
    e.name = "Warning"
    console.error(e)
    return
  }

  let canonical = font.stringify(info),
      px = parseSize(info.size),
      leading = parseSize(info.lineHeight, px) || px * 1.2;

  return Object.assign(info, {px, leading, canonical})
}


module.exports = {parseSize, parseFont}