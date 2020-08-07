const {inspect} = require('util'),
      {DOMMatrix, DOMRect, DOMPoint} = require('./geometry'),
      convertColor = require('color-convert'),
      {CanvasGradient, Image} = require('../native');


function extend(cls, methods){
  let m, proto = cls.prototype

  // stow selected 'shadow' rust Fns behind symbols
  for (let key of methods._shadow || []){
    if (proto[key]){
      let sym = Symbol.for(key)
      proto[sym] = proto[key]
      delete proto[key]
    }
  }

  // convert getFoo/setFoo pairs into a single foo method that switches between getter
  // and setter based on whether any arguments were passed
  let getset = {};
  for (let key of Object.getOwnPropertyNames(proto)){
    if (m = key.match(/([sg]et)_([A-Za-z])(.*)/)){
      let [verb, first, rest] = m.slice(1),
          prop = first.toLowerCase() + rest,
          sym = Symbol.for(key);
      getset[prop] = getset[prop] || {}
      getset[prop][verb] = sym
      proto[sym] = proto[key]
      delete proto[key]
    }
  }

  for (let [key, {get, set}] of Object.entries(getset)){
    // create a default getter/setter if a custom one isn't defined in `methods`
    if (typeof methods[key] == 'object') continue
    Object.defineProperty(proto, key, {
      get:function(){return this[get]()},
      set:function(val){this[set](val)},
    })
  }

  // merge in any added methods, stowing shadowed implementations behind symbols attrs
  for (let [attr, func] of Object.entries(methods)){
    if (attr=='_shadow') continue
    if (attr=='_describe') attr = inspect.custom
    else if (proto[attr]) proto[Symbol.for(attr)] = proto[attr]

    if (typeof func=='function') proto[attr] = func
    else if (typeof func=='object'){ Object.defineProperty(proto, attr, func) }
  }
}

function signature(args){
  return args.map(args, a => ({string:'s', number:'n', object:'o'}[typeof a] || 'x')).join('')
}

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

function toSkMatrix(jsMatrix){
  if (Array.isArray(jsMatrix)){
    var [a, b, c, d, e, f] = jsMatrix
  }else{
    var {a, b, c, d, e, f} = jsMatrix
  }
  return [a, c, e, b, d, f]
}

function fromSkMatrix(skMatrix){
  // TBD: how/if to map the perspective terms
  let [a, c, e, b, d, f, p0, p1, p2] = skMatrix
  return new DOMMatrix([a, b, c, d, e, f])
}

function loadImage(src) {
  return new Promise(function (res, rej) {
    let image = Object.assign(new Image(), {
      onload(){ res(image) },
      onerror(err){ rej(err) },
      src: src
    })
  })
}

module.exports = {extend, signature, toSkMatrix, fromSkMatrix, toColor, fromColor, loadImage}