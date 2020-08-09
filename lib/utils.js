"use strict"

const {inspect} = require('util'),
      {DOMMatrix} = require('./geometry');

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

// Helpers to reconcile Skia and DOMMatrix’s disagreement
// about matrix row/col orientation

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

module.exports = {$, RustClass, toSkMatrix, fromSkMatrix}