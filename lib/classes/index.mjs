import canvas from './canvas.js'
import imagery  from './imagery.js'
import geometry  from './geometry.js'
import typography  from './typography.js'
import context from './context.js'
import gui from './gui.js'
import path  from './path.js'

const {Canvas, CanvasGradient, CanvasPattern, CanvasTexture} = canvas
const {Image, ImageData, loadImage, loadImageData}  = imagery
const {DOMPoint, DOMMatrix, DOMRect}  = geometry
const {TextMetrics, FontLibrary}  = typography
const {CanvasRenderingContext2D} = context
const {App, Window} = gui
const {Path2D}  = path

export {
  Canvas, CanvasGradient, CanvasPattern, CanvasTexture,
  Image, ImageData, loadImage, loadImageData,
  Path2D, DOMPoint, DOMMatrix, DOMRect,
  FontLibrary, TextMetrics,
  CanvasRenderingContext2D,
  App, Window,
}

export default {
  Canvas, CanvasGradient, CanvasPattern, CanvasTexture,
  Image, ImageData, loadImage, loadImageData,
  Path2D, DOMPoint, DOMMatrix, DOMRect,
  FontLibrary, TextMetrics,
  CanvasRenderingContext2D,
  App, Window,
}
