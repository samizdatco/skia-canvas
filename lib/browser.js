"use strict"

const {asBuffer, asDownload, asZipDownload, options} = require('./io')

//
// Browser equivalents of the skia-canvas convenience initializers and polyfills for
// the Canvas object’s newPage & export methods
//

const _toURL_ = Symbol.for("toDataURL")

const loadImage = src => new Promise((onload, onerror) =>
  Object.assign(new classes.Image(), {onload, onerror, src})
)

class Canvas{
  constructor(width, height){
    let elt = document.createElement('canvas'),
        pages = []

    Object.defineProperty(elt, "async", {value:true, writable:false, enumerable:true})

    for (var [prop, get] of Object.entries({
      png: () => asBuffer(elt, 'image/png'),
      jpg: () => asBuffer(elt, 'image/jpeg'),
      pages: () => pages.concat(elt).map(c => c.getContext("2d")),
    })) Object.defineProperty(elt, prop, {get})

    return Object.assign(elt, {
      width, height,

      newPage(...size){
        var {width, height} = elt,
            page = Object.assign(document.createElement('canvas'), {width, height})
        page.getContext("2d").drawImage(elt, 0, 0)
        pages.push(page)

        var [width, height] = size.length ? size : [width, height]
        return Object.assign(elt, {width, height}).getContext("2d")
      },

      saveAs(filename, args){
        args = typeof args=='number' ? {quality:args} : args
        let opts = options(this.pages, {filename, ...args}),
            {pattern, padding, mime, quality, pages, density} = opts;
        return padding==undefined ? asDownload(pages[0].canvas, mime, quality, filename)
                                  : asZipDownload(pages, mime, quality, pattern, padding)
      },

      toBuffer(extension="png", args={}){
        args = typeof args=='number' ? {quality:args} : args
        let opts = options(this.pages, {extension, ...args}),
            {mime, quality, pages, density} = opts;
        return asBuffer(pages[0].canvas, mime, quality)
      },

      [_toURL_]: elt.toDataURL.bind(elt),
      toDataURL(extension="png", args={}){
        args = typeof args=='number' ? {quality:args} : args
        let opts = options(this.pages, {extension, ...args}),
            {mime, quality, pages, density} = opts,
            canvas = pages[0].canvas;

        return Promise.resolve( (canvas[_toURL_] || canvas.toDataURL)(mime, quality) )
      }
    })
  }
}

module.exports = {Canvas, loadImage}