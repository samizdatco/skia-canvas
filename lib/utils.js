"use strict"

const font = require('css-font'),
      {DOMMatrix} = require('./geometry');

//
// Neon <-> Node interface
//

// accessor for calling the rust implementation of a shadowed method
const $ = (obj, s, ...args) =>{
  let fn = Symbol.for(s)
  return obj[fn](...args)
}

// getter/setter wrapper
const getset = (verb, attr) => {
  let sym = Symbol.for(attr)
  return verb=='set' ? function(val){ this[sym](val) }
       : verb=='get' ? function(){ return this[sym]() }
       : undefined;
}

function RustClass(cls){
  let proto = cls.prototype,
      shadow = (key, rename) => {
        proto[Symbol.for(rename || key)] = proto[key];
        delete proto[key]
      };

  let m, props = {};
  for (let key of Object.getOwnPropertyNames(proto)){
    if (m = key.match(/^([sg]et)_(.*)/)){
      // collect and group all the get_* and set_* methods, renaming them using symbols.
      let [verb, attr] = m.slice(1)
      props[attr] = props[attr] || {}
      props[attr][verb] = getset(verb, key)
      shadow(key)
    }else if (m = key.match(/^_(.*)/)){
      // stow flagged rust Fns behind symbols so they can be called as 'super' methods
      shadow(key, m[1])
    }
  }

  // create a getter/setter property mapping to the collected methods
  for (let [key, prop] of Object.entries(props)) Object.defineProperty(proto, key, prop)

  return cls
}

//
// Helpers to reconcile Skia and DOMMatrix’s disagreement about row/col orientation
//

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

//
// Font string decoding
//

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

function parseWeight(str){
  let sizes = {
    // proportions from https://www.w3.org/TR/css-fonts-3/#font-size-prop
    "normal":400, "bold":700, "lighter":300, "bolder":800,
  },
  numSize =  /\d{3}/,
  namedSize = /normal|bold(?:er)?|lighter/,
  m;

  if (m = numSize.exec(str)){
    return parseInt(m[0])
  }

  if (m = namedSize.exec(str)){
    return sizes[m[0]]
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
      wt = parseWeight(info.weight),
      leading = parseSize(info.lineHeight, px) || px * 1.2;

  console.log(Object.assign(info, {px, wt, leading, canonical}))
  return Object.assign(info, {px, wt, leading, canonical})
}

module.exports = {$, RustClass, parseFont, toSkMatrix, fromSkMatrix}