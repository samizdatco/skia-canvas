//
// Skia Canvas — CommonJS version
//

"use strict"

const {Canvas, CanvasGradient, CanvasPattern, CanvasTexture} = require('./classes/canvas'),
      {Image, ImageData, loadImage, loadImageData} = require('./classes/imagery'),
      {DOMPoint, DOMMatrix, DOMRect} = require('./classes/geometry'),
      {TextMetrics, FontLibrary} = require('./classes/typography'),
      {CanvasRenderingContext2D} = require('./classes/context'),
      {App, Window} = require('./classes/gui'),
      {Path2D} = require('./classes/path')

module.exports = {
  Canvas, CanvasGradient, CanvasPattern, CanvasTexture,
  Image, ImageData, loadImage, loadImageData,
  Path2D, DOMPoint, DOMMatrix, DOMRect,
  FontLibrary, TextMetrics,
  CanvasRenderingContext2D,
  App, Window,
}
