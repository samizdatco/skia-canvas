const {inspect} = require('util'),
      {DOMMatrix} = require('./geometry'),
      {Image} = require('../native');

// accessor for calling the rust implementation of a shadowed method
const $ = (obj, s, ...args) =>{
  let fn = Symbol.for(s)
  return obj[fn](...args)
}

function RustClass(cls, methods={}){
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

  return cls
}

function signature(args){
  return args.map(args, a => ({string:'s', number:'n', object:'o'}[typeof a] || 'x')).join('')
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

function loadImage(src) {
  return new Promise(function (res, rej) {
    Object.assign(new Image(), {
      onload(img){ res(img) },
      onerror(err){ rej(err) },
      src
    })
  })
}

module.exports = {$, RustClass, signature, loadImage, toSkMatrix, fromSkMatrix}