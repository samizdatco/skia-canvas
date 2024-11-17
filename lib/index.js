//
// Skia Canvas — CommonJS version
//

"use strict"

const {DOMPoint, DOMMatrix, DOMRect, toSkMatrix, fromSkMatrix} = require('./classes/geometry'),
      {Canvas, CanvasGradient, CanvasPattern, CanvasTexture} = require('./classes/canvas'),
      {Image, ImageData, loadImage, loadImageData} = require('./classes/imagery'),
      {TextMetrics, FontLibrary} = require('./classes/typography'),
      {CanvasRenderingContext2D} = require('./classes/context'),
      {App, Window} = require('./classes/gui'),
      {Path2D} = require('./classes/path2d')

module.exports = {
  Canvas, CanvasGradient, CanvasPattern, CanvasTexture,
  Image, ImageData, loadImage, loadImageData,
  Path2D, DOMPoint, DOMMatrix, DOMRect,
  FontLibrary, TextMetrics,
  CanvasRenderingContext2D,
  App, Window,
}
