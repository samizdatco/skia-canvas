/// <reference lib="dom"/>
/// <reference types="node" />

export class DOMPoint extends globalThis.DOMPoint {}
export class DOMRect extends globalThis.DOMRect {}
export class CanvasGradient extends globalThis.CanvasGradient {}
export class CanvasTexture {}

//
// Images
//

export function loadImage(src: string | Buffer): Promise<Image>
export function loadImageData(src: string | Buffer, width: number, height?:number): Promise<ImageData>
export function loadImageData(src: string | Buffer, width: number, height:number, settings?:ImageDataSettings): Promise<ImageData>

export type ColorSpace = "srgb" // add "display-p3" when skia_safe supports it
export type ColorType = "Alpha8" | "Gray8" | "R8UNorm" | // 1 byte/px
  "A16Float" | "A16UNorm" | "ARGB4444" | "R8G8UNorm" | "RGB565" | // 2 bytes/px
  "rgb"|"RGB888x" | "rgba"|"RGBA8888" | "bgra"|"BGRA8888" | "BGR101010x" | "BGRA1010102" | // 4 bytes/px
  "R16G16Float" | "R16G16UNorm" | "RGB101010x" | "RGBA1010102" | "RGBA8888" |  "SRGBA8888" | // 4 bytes/px
  "R16G16B16A16UNorm" | "RGBAF16" | "RGBAF16Norm" | // 8 bytes/px
  "RGBAF32" // 16 bytes/px

interface ImageDataSettings {
  colorSpace?: ColorSpace
  colorType?: ColorType
}

export class ImageData {
  prototype: ImageData
  constructor(sw: number, sh: number, settings?: ImageDataSettings)
  constructor(data: Uint8ClampedArray | Buffer, sw: number, sh?: number, settings?: ImageDataSettings)
  constructor(image: Image, settings?: ImageDataSettings)
  constructor(imageData: ImageData)

  readonly colorSpace: ColorSpace
  readonly colorType: ColorType
  readonly data: Uint8ClampedArray
  readonly height: number
  readonly width: number
}

export class Image extends EventEmitter {
  constructor()
  get src(): string
  set src(src: string | Buffer)
  get width(): number
  get height(): number
  onload: ((this: Image, image: Image) => any) | null;
  onerror: ((this: Image, error: Error) => any) | null;
  complete: boolean
  decode(): Promise<Image>
}

//
// DOMMatrix
//

interface DOMMatrix {
  a: number, b: number, c: number, d: number, e: number, f: number,
  m11: number, m12: number, m13: number, m14: number,
  m21: number, m22: number, m23: number, m24: number,
  m31: number, m32: number, m33: number, m34: number,
  m41: number, m42: number, m43: number, m44: number,

  flipX(): DOMMatrix
  flipY(): DOMMatrix
  inverse(): DOMMatrix
  invertSelf(): DOMMatrix

  multiply(other?: DOMMatrixInit): DOMMatrix
  multiplySelf(other?: DOMMatrixInit): DOMMatrix
  preMultiplySelf(other?: DOMMatrixInit): DOMMatrix

  rotate(rotX?: number, rotY?: number, rotZ?: number): DOMMatrix
  rotateSelf(rotX?: number, rotY?: number, rotZ?: number): DOMMatrix
  rotateAxisAngle(x?: number, y?: number, z?: number, angle?: number): DOMMatrix
  rotateAxisAngleSelf(x?: number, y?: number, z?: number, angle?: number): DOMMatrix
  rotateFromVector(x?: number, y?: number): DOMMatrix
  rotateFromVectorSelf(x?: number, y?: number): DOMMatrix

  scale(scaleX?: number, scaleY?: number, scaleZ?: number, originX?: number, originY?: number, originZ?: number): DOMMatrix
  scaleSelf(scaleX?: number, scaleY?: number, scaleZ?: number, originX?: number, originY?: number, originZ?: number): DOMMatrix
  scale3d(scale?: number, originX?: number, originY?: number, originZ?: number): DOMMatrix
  scale3dSelf(scale?: number, originX?: number, originY?: number, originZ?: number): DOMMatrix

  skew(sx?: number, sy?:number): DOMMatrix
  skewSelf(sx?: number, sy?:number): DOMMatrix
  skewX(sx?: number): DOMMatrix
  skewXSelf(sx?: number): DOMMatrix
  skewY(sy?: number): DOMMatrix
  skewYSelf(sy?: number): DOMMatrix

  translate(tx?: number, ty?: number, tz?: number): DOMMatrix
  translateSelf(tx?: number, ty?: number, tz?: number): DOMMatrix

  setMatrixValue(transformList: string): DOMMatrix
  transformPoint(point?: DOMPointInit): DOMPoint

  toFloat32Array(): Float32Array
  toFloat64Array(): Float64Array
  toJSON(): any
  toString(): string
  clone(): DOMMatrix
}

type FixedLenArray<T, L extends number> = T[] & { length: L };
type Matrix = string | DOMMatrix | { a: number, b: number, c: number, d: number, e: number, f: number } | FixedLenArray<number, 6> | FixedLenArray<number, 16>

declare var DOMMatrix: {
  prototype: DOMMatrix
  new(init?: Matrix): DOMMatrix
  fromFloat32Array(array32: Float32Array): DOMMatrix
  fromFloat64Array(array64: Float64Array): DOMMatrix
  fromMatrix(other?: DOMMatrixInit): DOMMatrix
}

//
// Canvas
//

export type ExportFormat = "png" | "jpg" | "jpeg" | "webp" | "pdf" | "svg";

export interface RenderOptions {
  /** Page to export: Defaults to 1 (i.e., first page) */
  page?: number

  /** Background color to draw beneath transparent parts of the canvas */
  matte?: string

  /** Number of pixels per grid ‘point’ (defaults to 1) */
  density?: number

  /** Quality for lossy encodings like JPEG (0.0–1.0) */
  quality?: number

  /** Convert text to paths for SVG exports */
  outline?: boolean
}

export interface SaveOptions extends RenderOptions {
  /** Image format to use */
  format?: ExportFormat
}

export class Canvas {
  /** @internal */
  constructor(width?: number, height?: number)
  static contexts: WeakMap<Canvas, readonly CanvasRenderingContext2D[]>

  width: number
  height: number

  getContext(type?: "2d"): CanvasRenderingContext2D
  newPage(width?: number, height?: number): CanvasRenderingContext2D
  readonly pages: CanvasRenderingContext2D[]

  get gpu(): boolean
  set gpu(enabled: boolean)

  saveAs(filename: string, options?: SaveOptions): Promise<void>
  toBuffer(format: ExportFormat, options?: RenderOptions): Promise<Buffer>
  toDataURL(format: ExportFormat, options?: RenderOptions): Promise<string>

  saveAsSync(filename: string, options?: SaveOptions): void
  toBufferSync(format: ExportFormat, options?: RenderOptions): Buffer
  toDataURLSync(format: ExportFormat, options?: RenderOptions): string

  get pdf(): Promise<Buffer>
  get svg(): Promise<Buffer>
  get jpg(): Promise<Buffer>
  get png(): Promise<Buffer>
  get webp(): Promise<Buffer>
}

//
// CanvasPattern
//

export class CanvasPattern{
  setTransform(transform: Matrix): void;
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void
}

//
// Context
//

type Offset = [x: number, y: number] | number

export interface CreateTextureOptions {
  /** The 2D shape to be drawn in a repeating grid with the specified spacing (if omitted, parallel lines will be used) */
  path?: Path2D

  /** The lineWidth with which to stroke the path (if omitted, the path will be filled instead) */
  line?: number

  /** The color to use for stroking/filling the path */
  color?: string

  /** The orientation of the pattern grid in radians */
  angle?: number

  /** The amount by which to shift the pattern relative to the canvas origin */
  offset?: Offset
}

export type CanvasPatternSource = Canvas | Image;
export type CanvasDrawable = Canvas | Image | ImageData;

interface CanvasDrawImage {
  drawImage(image: CanvasDrawable, dx: number, dy: number): void;
  drawImage(image: CanvasDrawable, dx: number, dy: number, dw: number, dh: number): void;
  drawImage(image: CanvasDrawable, sx: number, sy: number, sw: number, sh: number, dx: number, dy: number, dw: number, dh: number): void;
  drawCanvas(image: Canvas, dx: number, dy: number): void;
  drawCanvas(image: Canvas, dx: number, dy: number, dw: number, dh: number): void;
  drawCanvas(image: Canvas, sx: number, sy: number, sw: number, sh: number, dx: number, dy: number, dw: number, dh: number): void;
}

interface CanvasFillStrokeStyles {
  fillStyle: string | CanvasGradient | CanvasPattern | CanvasTexture;
  strokeStyle: string | CanvasGradient | CanvasPattern | CanvasTexture;
  createConicGradient(startAngle: number, x: number, y: number): CanvasGradient;
  createLinearGradient(x0: number, y0: number, x1: number, y1: number): CanvasGradient;
  createRadialGradient(x0: number, y0: number, r0: number, x1: number, y1: number, r1: number): CanvasGradient;
  createPattern(image: CanvasPatternSource, repetition: string | null): CanvasPattern | null;
  createTexture(spacing: Offset, options?: CreateTextureOptions): CanvasTexture
}

type QuadOrRect = [x1:number, y1:number, x2:number, y2:number, x3:number, y3:number, x4:number, y4:number] |
                  [left:number, top:number, right:number, bottom:number] | [width:number, height:number]

type CornerRadius = number | DOMPoint

interface CanvasTransform extends Omit<globalThis.CanvasTransform, "transform" | "setTransform">{}

interface CanvasTextDrawingStyles extends Omit<globalThis.CanvasTextDrawingStyles, "fontKerning" | "fontVariantCaps" | "textRendering">{}

type FontVariantSetting = "normal" |
/* alternates */ "historical-forms" |
/* caps */ "small-caps" | "all-small-caps" | "petite-caps" | "all-petite-caps" | "unicase" | "titling-caps" |
/* numeric */ "lining-nums" | "oldstyle-nums" | "proportional-nums" | "tabular-nums" | "diagonal-fractions" | "stacked-fractions" | "ordinal" | "slashed-zero" |
/* ligatures */ "common-ligatures" | "no-common-ligatures" | "discretionary-ligatures" | "no-discretionary-ligatures" | "historical-ligatures" | "no-historical-ligatures" | "contextual" | "no-contextual" |
/* east-asian */ "jis78" | "jis83" | "jis90" | "jis04" | "simplified" | "traditional" | "full-width" | "proportional-width" | "ruby" |
/* position */ "super" | "sub";


export interface CanvasRenderingContext2D extends CanvasCompositing, CanvasDrawImage, CanvasDrawPath, CanvasFillStrokeStyles, CanvasFilters, CanvasImageSmoothing, CanvasPath, CanvasPathDrawingStyles, CanvasRect, CanvasShadowStyles, CanvasState, CanvasText, CanvasTextDrawingStyles, CanvasTransform, CanvasUserInterface {
  readonly canvas: Canvas
  fontVariant: FontVariantSetting
  textWrap: boolean
  textDecoration: string
  lineDashMarker: Path2D | null
  lineDashFit: "move" | "turn" | "follow"

  // transform argument extensions (accept DOMMatrix & matrix-like objectx, not just param lists)
  setTransform(transform?: Matrix): void
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void

  transform(transform: Matrix): void
  transform(a: number, b: number, c: number, d: number, e: number, f: number): void

  get currentTransform(): DOMMatrix
  set currentTransform(matrix: Matrix)
  createProjection(quad: QuadOrRect, basis?: QuadOrRect): DOMMatrix

  // skia/chrome beziers & convenience methods
  conicCurveTo(cpx: number, cpy: number, x: number, y: number, weight: number): void
  roundRect(x: number, y: number, width: number, height: number, radii: number | CornerRadius[]): void
  reset(): void
  // getContextAttributes(): CanvasRenderingContext2DSettings;

  // add maxWidth to work in conjunction with textWrap
  measureText(text: string, maxWidth?: number): TextMetrics
  outlineText(text: string, maxWidth?: number): Path2D

  // use the local definitions for settings w/ supported ColorType values
  createImageData(width: number, height: number, settings?: ImageDataSettings): ImageData;
  createImageData(imagedata: ImageData): ImageData;
  getImageData(x: number, y: number, width: number, height: number, settings?: ImageDataSettings): ImageData;
  putImageData(imagedata: ImageData, dx: number, dy: number): void;
  putImageData(imagedata: ImageData, dx: number, dy: number, dirtyX: number, dirtyY: number, dirtyWidth: number, dirtyHeight: number): void;
}

//
// Bézier Paths
//

export interface Path2DBounds {
  readonly top: number
  readonly left: number
  readonly bottom: number
  readonly right: number
  readonly width: number
  readonly height: number
}

export type Path2DEdge = [verb: string, ...args: number[]]

export class Path2D extends globalThis.Path2D {
  d: string
  readonly bounds: Path2DBounds
  readonly edges: readonly Path2DEdge[]

  contains(x: number, y: number): boolean
  conicCurveTo(
    cpx: number,
    cpy: number,
    x: number,
    y: number,
    weight: number
  ): void

  roundRect(x: number, y: number, width: number, height: number, radii: number | CornerRadius[]): void

  complement(otherPath: Path2D): Path2D
  difference(otherPath: Path2D): Path2D
  intersect(otherPath: Path2D): Path2D
  union(otherPath: Path2D): Path2D
  xor(otherPath: Path2D): Path2D
  interpolate(otherPath: Path2D, weight: number): Path2D

  jitter(segmentLength: number, amount: number, seed?: number): Path2D
  offset(dx: number, dy: number): Path2D
  points(step?: number): readonly [x: number, y: number][]
  round(radius: number): Path2D
  simplify(rule?: "nonzero" | "evenodd"): Path2D
  transform(transform: Matrix): Path2D;
  transform(a: number, b: number, c: number, d: number, e: number, f: number): Path2D;
  trim(start: number, end: number, inverted?: boolean): Path2D;
  trim(start: number, inverted?: boolean): Path2D;

  unwind(): Path2D
}

//
// Typography
//

export interface TextMetrics extends globalThis.TextMetrics {
  lines: TextMetricsLine[]
}

export interface TextMetricsLine {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
  readonly baseline: number
  readonly startIndex: number
  readonly endIndex: number
}

export interface FontFamily {
  family: string
  weights: number[]
  widths: string[]
  styles: string[]
}

export interface Font {
  family: string
  weight: number
  style: string
  width: string
  file: string
}

export interface FontLibrary {
  families: readonly string[]
  family(name: string): FontFamily | undefined
  has(familyName: string): boolean

  use(familyName: string, fontPaths?: string | readonly string[]): Font[]
  use(fontPaths: readonly string[]): Font[]
  use(
    families: Record<string, readonly string[] | string>
  ): Record<string, Font[] | Font>

  reset(): void
}

export const FontLibrary: FontLibrary

//
// Window & App
//

import { EventEmitter } from "stream";
export type FitStyle = "none" | "contain-x" | "contain-y" | "contain" | "cover" | "fill" | "scale-down" | "resize"
export type CursorStyle = "default" | "crosshair" | "hand" | "arrow" | "move" | "text" | "wait" | "help" | "progress" | "not-allowed" | "context-menu" |
                          "cell" | "vertical-text" | "alias" | "copy" | "no-drop" | "grab" | "grabbing" | "all-scroll" | "zoom-in" | "zoom-out" |
                          "e-resize" | "n-resize" | "ne-resize" | "nw-resize" | "s-resize" | "se-resize" | "sw-resize" | "w-resize" | "ew-resize" |
                          "ns-resize" | "nesw-resize" | "nwse-resize" | "col-resize" | "row-resize" | "none"

export type WindowOptions = {
  title?: string
  left?: number
  top?: number
  width?: number
  height?: number
  fit?: FitStyle
  page?: number
  background?: string
  fullscreen?: boolean
  visible?: boolean
  cursor?: CursorStyle
  canvas?: Canvas
}

type MouseEventProps = {
  x: number;
  y: number;
  pageX: number;
  pageY: number;
  button: number;
  ctrlKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

type KeyboardEventProps = {
  key: string
  code: string
  location: number
  repeat: boolean
  ctrlKey: boolean
  altKey: boolean
  metaKey: boolean
  shiftKey: boolean
}

type WindowEvents = {
  mousedown: MouseEventProps
  mouseup: MouseEventProps
  mousemove: MouseEventProps
  keydown: KeyboardEventProps
  keyup: KeyboardEventProps
  input: {
    data: string
    inputType: 'insertText'
  };
  wheel: { deltaX: number; deltaY: number }
  fullscreen: { enabled: boolean }
  move: { left: number; top: number }
  resize: { height: number; width: number }
  frame: { frame: number }
  draw: { frame: number }
  blur: {}
  focus: {}
  setup: {}
}

export class Window extends EventEmitter<{
  [EventName in keyof WindowEvents]: [
    {
      target: Window;
      type: EventName;
    } & WindowEvents[EventName]
  ]
}>{
  constructor(width: number, height: number, options?: WindowOptions)
  constructor(options?: WindowOptions)

  readonly ctx: CanvasRenderingContext2D
  canvas: Canvas
  visible: boolean
  fullscreen: boolean
  title: string
  cursor: CursorStyle
  fit: FitStyle
  left: number
  top: number
  width: number
  height: number
  page: number
  background: string

  close(): void
}

export interface App{
  readonly windows: Window[]
  readonly running: boolean
  fps: number

  launch(): void
  quit(): void
}

export const App: App
