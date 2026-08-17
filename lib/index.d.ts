import {Sharp} from "sharp"

//
// Geometry
//

interface DOMPointInit {
  x?: number;
  y?: number;
  z?: number;
  w?: number;
}

/** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint) */
interface DOMPoint extends DOMPointReadOnly {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint/x) */
  x: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint/y) */
  y: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint/z) */
  z: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint/w) */
  w: number;
}

declare var DOMPoint: {
  prototype: DOMPoint;
  new(x?: number, y?: number, z?: number, w?: number): DOMPoint;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPoint/fromPoint_static) */
  fromPoint(other?: DOMPointInit): DOMPoint;
};

/** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly) */
interface DOMPointReadOnly {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/x) */
  readonly x: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/y) */
  readonly y: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/z) */
  readonly z: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/w) */
  readonly w: number;
  matrixTransform(matrix?: DOMMatrixInit): DOMPoint;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMPointReadOnly/toJSON) */
  toJSON(): any;
}


/** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRect) */
interface DOMRect extends DOMRectReadOnly {
  height: number;
  width: number;
  x: number;
  y: number;
}

interface DOMRectInit {
  height?: number;
  width?: number;
  x?: number;
  y?: number;
}

declare var DOMRect: {
  prototype: DOMRect;
  new(x?: number, y?: number, width?: number, height?: number): DOMRect;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRect/fromRect_static) */
  fromRect(other?: DOMRectInit): DOMRect;
};

interface DOMRectList {
  readonly length: number;
  item(index: number): DOMRect | null;
  [index: number]: DOMRect;
}

/** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly) */
interface DOMRectReadOnly {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/bottom) */
  readonly bottom: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/height) */
  readonly height: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/left) */
  readonly left: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/right) */
  readonly right: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/top) */
  readonly top: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/width) */
  readonly width: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/x) */
  readonly x: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/DOMRectReadOnly/y) */
  readonly y: number;
  toJSON(): any;
}


//
// Images
//

/** Options passed to Node's `http`/`https` request when fetching images from remote URLs */
export type ImageRequestOptions = import("https").RequestOptions & {
  /** Optional request body (sent via `request.write()`) */
  body?: string | Buffer
}

export function loadImage(src: string | URL, options?: ImageRequestOptions): Promise<Image>
export function loadImage(src: Sharp | Buffer): Promise<Image>

export function loadImageData(src: string | Buffer | URL, width: number, height?:number): Promise<ImageData>
export function loadImageData(src: string | Buffer | URL, width: number, height:number, settings?:ImageDataSettings & ImageRequestOptions): Promise<ImageData>
export function loadImageData(src: Sharp): Promise<ImageData>

export type ColorSpace = "srgb" | "display-p3"
export type ColorType = "Alpha8" | "Gray8" | "R8UNorm" | // 1 byte/px
  "A16Float" | "A16UNorm" | "ARGB4444" | "R8G8UNorm" | "RGB565" | // 2 bytes/px
  "rgb"|"RGB888x" | "rgba"|"RGBA8888" | "bgra"|"BGRA8888" | "BGR101010x" | "BGRA1010102" | // 4 bytes/px
  "R16G16Float" | "R16G16UNorm" | "RGB101010x" | "RGBA1010102" | "SRGBA8888" | // 4 bytes/px
  "R16G16B16A16UNorm" | "RGBAF16" | "RGBAF16Norm" | // 8 bytes/px
  "RGBAF32" // 16 bytes/px

interface ImageDataSettings {
  colorSpace?: ColorSpace
  colorType?: ColorType
}

interface ImageDataExportSettings {
  /** Background color to draw beneath transparent parts of the canvas */
  matte?: CSSColor

  /** Number of pixels per grid ‘point’ (defaults to 1) */
  density?: number

  /** Number of samples used for antialising each pixel */
  msaa?: number | boolean

  /** Color space to convert pixels into (defaults to "srgb") */
  colorSpace?: ColorSpace

  /** Color type to use when exporting in "raw" format */
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
  readonly bytesPerPixel: number
  readonly data: Uint8ClampedArray
  readonly height: number
  readonly width: number
  toSharp(): Sharp
}

export class Image extends EventEmitter{
  constructor(data?: Buffer | URL | string, src?: string)
  get src(): string
  set src(src: string | URL | Buffer | Sharp)
  get width(): number
  get height(): number
  onload: ((this: Image, image: Image) => any) | null;
  onerror: ((this: Image, error: Error) => any) | null;
  readonly complete: boolean
  decode(): Promise<Image>

  /** Release native state synchronously rather than waiting for the GC's deferred finalizers. Abandons an in-flight load, rejecting any pending `decode()`. The image may no longer be used after disposal. Can also be triggered via `using`. */
  dispose(): void
  [Symbol.dispose](): void
  /** Dispose of the image then yield until the next event loop tick so other deferred finalizers can run. Can also be triggered via `await using`. */
  release(): Promise<void>
  [Symbol.asyncDispose](): Promise<void>
}

//
// DOMMatrix
//

interface DOMMatrix2DInit {
  a?: number;
  b?: number;
  c?: number;
  d?: number;
  e?: number;
  f?: number;
  m11?: number;
  m12?: number;
  m21?: number;
  m22?: number;
  m41?: number;
  m42?: number;
}

interface DOMMatrixInit extends DOMMatrix2DInit {
  is2D?: boolean;
  m13?: number;
  m14?: number;
  m23?: number;
  m24?: number;
  m31?: number;
  m32?: number;
  m33?: number;
  m34?: number;
  m43?: number;
  m44?: number;
}

interface DOMMatrix {
  a: number, b: number, c: number, d: number, e: number, f: number,
  m11: number, m12: number, m13: number, m14: number,
  m21: number, m22: number, m23: number, m24: number,
  m31: number, m32: number, m33: number, m34: number,
  m41: number, m42: number, m43: number, m44: number,

  readonly is2D: boolean
  readonly isIdentity: boolean

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

  setMatrixValue(transformList: TransformString): DOMMatrix
  transformPoint(point?: DOMPointInit): DOMPoint

  toFloat32Array(): Float32Array
  toFloat64Array(): Float64Array
  toJSON(): any
  toString(): string
  clone(): DOMMatrix
}

type TransformFunction =
  | "matrix()" | "matrix3d()"
  | "translate()" | "translateX()" | "translateY()" | "translateZ()" | "translate3d()"
  | "scale()" | "scaleX()" | "scaleY()" | "scaleZ()" | "scale3d()"
  | "rotate()" | "rotateX()" | "rotateY()" | "rotateZ()" | "rotate3d()"
  | "skew()" | "skewX()" | "skewY()"
type TransformString = TransformFunction | "none" | (string & {})

type FixedLenArray<T, L extends number> = T[] & { length: L };
type Matrix = TransformString | DOMMatrix | { a: number, b: number, c: number, d: number, e: number, f: number } | FixedLenArray<number, 6> | FixedLenArray<number, 16>

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

export type ExportFormat = "png" | "jpg" | "jpeg" | "webp" | "raw" | "pdf" | "svg" |
                           "image/png" | "image/jpeg" | "image/webp" | "application/pdf" | "image/svg+xml" |
                           "application/octet-stream";
export type FontOptions = "outline" | "device-independent"

export interface RenderOptions {
  /** Page to export: Defaults to 1 (i.e., first page) */
  page?: number

  /** Background color to draw beneath transparent parts of the canvas */
  matte?: CSSColor

  /** Number of pixels per grid ‘point’ (defaults to 1) */
  density?: number

  /** Number of samples used for antialising each pixel */
  msaa?: number | boolean
}

export interface ExportOptions extends RenderOptions {
  /** Quality for lossy encodings like JPEG & WEBP (0.0–1.0) */
  quality?: number

  /** Optionally convert text to bézier paths (SVG only) */
  outline?: boolean

  /** Optionally use 4:2:0 chroma subsampling (JPEG only) */
  downsample?: boolean

  /** Color type to use when exporting in "raw" format */
  colorType?: ColorType
}

export interface SaveOptions extends ExportOptions {
  /** Image format to use (either as a file extension or a mime-type string) */
  format?: ExportFormat
}

export interface EngineDetails {
  /** Whether the active renderer is CPU- or GPU-backed */
  renderer: "CPU" | "GPU"
  /** Backend API in use (null when compiled without GPU support) */
  api: "Vulkan" | "Metal" | null
  /** Details about the graphics hardware that was initialized */
  device: string
  /** The device driver useed by the active graphics adapter */
  driver?: string
  /** Size of the thread pool used for async rendering (set via the SKIA_CANVAS_THREADS env var; defaults to CPU core count) */
  threads: number
  /** Error thrown when attempting to initialize the GPU */
  error?: string | null
  /** Amount of additional contrast added when rendering text (see TextOptions) */
  textContrast: number
  /** Gamma value used for blending the edges of letterforms (see TextOptions) */
  textGamma: number
}

export interface TextOptions{
  /** Amount of additional contrast to add when rendering text (defaults to 0) */
  textContrast?: number

  /** Gamma value for blending the edges of letterforms (defaults to 1.4) */
  textGamma?: number
}

export interface CanvasBackend{
  /** Whether to prefer a GPU-backed renderer (defaults to true) */
  gpu?: boolean
}

export interface CanvasRenderingContext2DSettings {
  /** Color space used when rasterizing the canvas for exports & getImageData (defaults to "srgb") */
  colorSpace?: ColorSpace
  /** Hint that the canvas will be read back frequently via getImageData (defaults to false) */
  willReadFrequently?: boolean
}

/** [Skia Canvas Docs](https://skia-canvas.org/api/canvas) */
export class Canvas {
  /**
   * Gets or sets the height of a canvas element on a document.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/HTMLCanvasElement/height)
   */
  height: number;
  /**
   * Gets or sets the width of a canvas element on a document.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/HTMLCanvasElement/width)
   */
  width: number;

  /** [Skia Canvas Docs](https://skia-canvas.org/api/canvas#creating-new-canvas-objects) */
  constructor(width?: number, height?: number, options?: TextOptions & CanvasBackend)

  /**
   * Returns an object that provides methods and properties for drawing and manipulating images and graphics on a canvas element in a document. A context object includes information about colors, line widths, fonts, and other graphic parameters that can be drawn on a canvas.
   * @param type The type of canvas to create. Skia Canvas only supports a 2-D context using canvas.getContext("2d")
   * @param settings Context attributes (only honored by the first getContext call for a given canvas — use newPage() to set them on subsequent pages)
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/HTMLCanvasElement/getContext)
   */
  getContext(type: "2d", settings?: CanvasRenderingContext2DSettings): CanvasRenderingContext2D
  getContext(type?: string, settings?: CanvasRenderingContext2DSettings): CanvasRenderingContext2D | null

  /**
   * Adds a new page to the canvas, optionally resizing it and/or overriding the context
   * attributes inherited from the canvas's first page.
   *
   * [Skia Canvas Docs](https://skia-canvas.org/api/canvas#newpage)
   */
  newPage(settings?: CanvasRenderingContext2DSettings): CanvasRenderingContext2D
  newPage(width?: number, height?: number, settings?: CanvasRenderingContext2DSettings): CanvasRenderingContext2D
  readonly pages: CanvasRenderingContext2D[]

  get gpu(): boolean
  set gpu(enabled: boolean)
  readonly engine: EngineDetails

  /** Release native state synchronously rather than waiting for the GC's deferred finalizers. The canvas may no longer be used after disposal. Can also be triggered via `using`. */
  dispose(): void
  [Symbol.dispose](): void
  /** Dispose of the canvas then yield until the next event loop tick so other deferred finalizers can run. Can also be triggered via `await using`. */
  release(): Promise<void>
  [Symbol.asyncDispose](): Promise<void>

  /** @deprecated Use {@link Canvas.toFile()} instead */
  saveAs(filename: string | URL, options?: SaveOptions): Promise<void>
  /** [Skia Canvas Docs](https://skia-canvas.org/api/canvas#tofile): toFile() */
  toFile(filename: string | URL, options?: SaveOptions): Promise<void>
  /** [Skia Canvas Docs](https://skia-canvas.org/api/canvas#tobuffer) */
  toBuffer(format?: ExportFormat, options?: ExportOptions): Promise<Buffer>
  /** [Skia Canvas Docs](https://skia-canvas.org/api/canvas#tourl) */
  toURL(format?: ExportFormat, options?: ExportOptions): Promise<string>
  /** [Skia Canvas Docs](https://skia-canvas.org/api/canvas#tosharp) */
  toSharp(options?: RenderOptions): Sharp

  /** @deprecated Use {@link Canvas.toFileSync()} instead */
  saveAsSync(filename: string | URL, options?: SaveOptions): void
  /** [Skia Canvas Docs](https://skia-canvas.org/api/canvas#tofile): toFile() */
  toFileSync(filename: string | URL, options?: SaveOptions): void
  /** [Skia Canvas Docs](https://skia-canvas.org/api/canvas#tobuffer) */
  toBufferSync(format?: ExportFormat, options?: ExportOptions): Buffer
  /** @deprecated {@link Canvas.toDataURL()} is now synchronous; use it instead */
  toDataURLSync(format?: ExportFormat, options?: ExportOptions): string
  /** [Skia Canvas Docs](https://skia-canvas.org/api/canvas#tourl) */
  toURLSync(format?: ExportFormat, options?: ExportOptions): string

  /** [MDN Reference](https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/toDataURL) */
  toDataURL(format?: ExportFormat, quality?: number): string

  get raw(): Promise<Buffer>
  get pdf(): Promise<Buffer>
  get svg(): Promise<Buffer>
  get jpg(): Promise<Buffer>
  get png(): Promise<Buffer>
  get webp(): Promise<Buffer>
}

//
// Patterns
//

/**
 * An opaque object describing a pattern, based on an image, a canvas, or a video, created by the CanvasRenderingContext2D.createPattern() method.
 *
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasPattern)
 */
export class CanvasPattern{
  setTransform(transform: Matrix): void;
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void
}

/** How a {@link CanvasPattern} tiles its image.  */
export type PatternRepeat = "repeat" | "repeat-x" | "repeat-y" | "no-repeat"

// Non-polar interpolation spaces (interpolated by rectangular/cartesian coordinates)
export type RectangularColorSpace = "srgb" | "srgb-linear" | "display-p3" | "a98-rgb" | "prophoto-rgb" | "rec2020" | "lab" | "oklab"
// Cylindrical spaces with a hue angle (the only ones `hueInterpolationMethod` affects)
export type PolarColorSpace = "hsl" | "hwb" | "lch" | "oklch"
export type ColorInterpolationMethod = RectangularColorSpace | PolarColorSpace
export type HueInterpolationMethod = "shorter" | "longer" | "increasing" | "decreasing"

//
// Colors
//

type ColorFunction =
  | "rgb()" | "rgba()" | "hsl()" | "hsla()" | "hwb()"
  | "lab()" | "lch()" | "oklab()" | "oklch()" | "color()"
export type CSSColor = ColorFunction | "#" | (string & {})

/**
 * An opaque object describing a gradient. It is returned by the methods CanvasRenderingContext2D.createLinearGradient() or CanvasRenderingContext2D.createRadialGradient().
 *
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasGradient)
 */

interface CanvasGradient {
  /**
   * Adds a color stop with the given color to the gradient at the given offset. 0.0 is the offset at one end of the gradient, 1.0 is the offset at the other end.
   *
   * Throws an "IndexSizeError" DOMException if the offset is out of range. Throws a "SyntaxError" DOMException if the color cannot be parsed.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasGradient/addColorStop)
   */
  addColorStop(offset: number, color: CSSColor): void;
  /** Color space in which the gradient's colors are interpolated (default `"srgb"`). */
  colorInterpolationMethod: ColorInterpolationMethod;
  /** Hue-interpolation direction for polar spaces — hsl/hwb/lch/oklch (default `"shorter"`). */
  hueInterpolationMethod: HueInterpolationMethod;
  /** Whether colors are interpolated with premultiplied alpha (default `false`). */
  premultipliedAlpha: boolean;
}

declare var CanvasGradient: {
  prototype: CanvasGradient;
  new(): CanvasGradient;
};


export class CanvasTexture {}


//
// Context
//

type CanvasDrawable = Canvas | Image | ImageData;
type CanvasPatternSource = Canvas | Image | ImageData;
type CanvasDirection = "inherit" | "ltr" | "rtl";
type CanvasFillRule = "evenodd" | "nonzero";
type CanvasFontStretch = "condensed" | "expanded" | "extra-condensed" | "extra-expanded" | "normal" | "semi-condensed" | "semi-expanded" | "ultra-condensed" | "ultra-expanded";
type CanvasTextAlign = "center" | "end" | "left" | "right" | "start" | "justify";
type CanvasTextBaseline = "alphabetic" | "bottom" | "hanging" | "ideographic" | "middle" | "top";
type CanvasLineCap = "butt" | "round" | "square";
type CanvasLineJoin = "bevel" | "miter" | "round";
// type CanvasFontKerning = "auto" | "none" | "normal";
// type CanvasFontVariantCaps = "all-petite-caps" | "all-small-caps" | "normal" | "petite-caps" | "small-caps" | "titling-caps" | "unicase";
// type CanvasTextRendering = "auto" | "geometricPrecision" | "optimizeLegibility" | "optimizeSpeed";

type Offset = [x: number, y: number] | number
type QuadOrRect = [x1:number, y1:number, x2:number, y2:number, x3:number, y3:number, x4:number, y4:number] |
                  [left:number, top:number, right:number, bottom:number] | [width:number, height:number]
type GlobalCompositeOperation = "clear" | "color" | "color-burn" | "color-dodge" | "copy" | "darken" | "destination" | "destination-atop" | "destination-in" | "destination-out" | "destination-over" | "difference" | "exclusion" | "hard-light" | "hue" | "lighten" | "lighter" | "luminosity" | "multiply" | "overlay" | "saturation" | "screen" | "soft-light" | "source-atop" | "source-in" | "source-out" | "source-over" | "xor";
type ImageSmoothingQuality = "high" | "low" | "medium";

type FontVariantKeyword = "normal" |
/* alternates */ "historical-forms" |
/* caps */ "small-caps" | "all-small-caps" | "petite-caps" | "all-petite-caps" | "unicase" | "titling-caps" |
/* numeric */ "lining-nums" | "oldstyle-nums" | "proportional-nums" | "tabular-nums" | "diagonal-fractions" | "stacked-fractions" | "ordinal" | "slashed-zero" |
/* ligatures */ "common-ligatures" | "no-common-ligatures" | "discretionary-ligatures" | "no-discretionary-ligatures" | "historical-ligatures" | "no-historical-ligatures" | "contextual" | "no-contextual" |
/* east-asian */ "jis78" | "jis83" | "jis90" | "jis04" | "simplified" | "traditional" | "full-width" | "proportional-width" | "ruby" |
/* position */ "super" | "sub";

type FontVariantAlternate =
  | `stylistic(${number})` | `styleset(${number})` | `swash(${number})`
  | `character-variant(${number})` | `ornaments(${number})` | `annotation(${number})`;
type FontVariantSetting = FontVariantKeyword | FontVariantAlternate | (string & {});

// A `text-decoration` shorthand value: a line keyword optionally combined with a style keyword,
// a thickness (a LengthString), and a color — e.g. "underline", "line-through wavy", or
// "underline 2px dodgerblue". The finite keywords below surface in completions; the (string & {})
// escape hatch lets full combos and computed strings through (validated by the runtime, not TS).
type TextDecorationKeyword =
  /* line */ "none" | "underline" | "overline" | "line-through" |
  /* style */ "solid" | "double" | "dotted" | "dashed" | "wavy" |
  /* thickness source */ "auto" | "from-font";
type TextDecorationSetting = TextDecorationKeyword | (string & {});


export interface CreateTextureOptions {
  /** The 2D shape to be drawn in a repeating grid with the specified spacing (if omitted, parallel lines will be used) */
  path?: Path2D

  /** The lineWidth with which to stroke the path (if omitted, the path will be filled instead) */
  line?: number

  /** The lineCap style to use if stroking the path */
  cap?: CanvasLineCap

  /** The color to use for stroking/filling the path */
  color?: CSSColor

  /** The orientation of the pattern grid in radians */
  angle?: number

  /** The amount by which to shift the pattern relative to the canvas origin */
  offset?: Offset

  /** Whether to render the texture as a single path (rather than as a repeating pattern within a clipping mask) */
  outline?: boolean
}

interface CanvasCompositing {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/globalAlpha) */
  globalAlpha: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/globalCompositeOperation) */
  globalCompositeOperation: GlobalCompositeOperation;
}

interface CanvasDrawImage {
  drawImage(image: CanvasDrawable, dx: number, dy: number): void;
  drawImage(image: CanvasDrawable, dx: number, dy: number, dw: number, dh: number): void;
  drawImage(image: CanvasDrawable, sx: number, sy: number, sw: number, sh: number, dx: number, dy: number, dw: number, dh: number): void;
  drawCanvas(image: CanvasDrawable, dx: number, dy: number): void;
  drawCanvas(image: CanvasDrawable, dx: number, dy: number, dw: number, dh: number): void;
  drawCanvas(image: CanvasDrawable, sx: number, sy: number, sw: number, sh: number, dx: number, dy: number, dw: number, dh: number): void;
}

interface CanvasDrawPath {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/beginPath) */
  beginPath(): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/clip) */
  clip(fillRule?: CanvasFillRule): void;
  clip(path: Path2D, fillRule?: CanvasFillRule): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fill) */
  fill(fillRule?: CanvasFillRule): void;
  fill(path: Path2D, fillRule?: CanvasFillRule): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/isPointInPath) */
  isPointInPath(x: number, y: number, fillRule?: CanvasFillRule): boolean;
  isPointInPath(path: Path2D, x: number, y: number, fillRule?: CanvasFillRule): boolean;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/isPointInStroke) */
  isPointInStroke(x: number, y: number): boolean;
  isPointInStroke(path: Path2D, x: number, y: number): boolean;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/stroke) */
  stroke(): void;
  stroke(path: Path2D): void;
}

interface CanvasFillStrokeStyles {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fillStyle) */
  fillStyle: CSSColor | CanvasGradient | CanvasPattern | CanvasTexture;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/strokeStyle) */
  strokeStyle: CSSColor | CanvasGradient | CanvasPattern | CanvasTexture;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/createConicGradient) */
  createConicGradient(startAngle: number, x: number, y: number): CanvasGradient;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/createLinearGradient) */
  createLinearGradient(x0: number, y0: number, x1: number, y1: number): CanvasGradient;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/createPattern) */
  createPattern(image: CanvasPatternSource, repetition: PatternRepeat | "" | null): CanvasPattern | null;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/createRadialGradient) */
  createRadialGradient(x0: number, y0: number, r0: number, x1: number, y1: number, r1: number): CanvasGradient;

  /** [Skia Canvas Docs](https://skia-canvas.org/api/context#createtexture) */
  createTexture(spacing: Offset, options?: CreateTextureOptions): CanvasTexture
}

type FilterFunction =
  | "blur()" | "brightness()" | "contrast()" | "drop-shadow()" | "grayscale()"
  | "hue-rotate()" | "invert()" | "opacity()" | "saturate()" | "sepia()"

interface CanvasFilters {
  /**
   * A CSS `filter` string — one or more of the {@link FilterFunction} primitives (`blur()`,
   * `drop-shadow()`, `hue-rotate()`, …) or `"none"`.
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/filter)
   */
  filter: FilterFunction | "none" | (string & {});
}

interface CanvasImageData {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/createImageData) */
  createImageData(width: number, height: number, settings?: ImageDataSettings): ImageData;
  createImageData(imagedata: ImageData): ImageData;

  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/getImageData) */
  getImageData(x: number, y: number, width: number, height: number, settings?: ImageDataExportSettings): ImageData;

  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/putImageData) */
  putImageData(imagedata: ImageData, dx: number, dy: number): void;
  putImageData(imagedata: ImageData, dx: number, dy: number, dirtyX: number, dirtyY: number, dirtyWidth: number, dirtyHeight: number): void;
}

interface CanvasImageSmoothing {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/imageSmoothingEnabled) */
  imageSmoothingEnabled: boolean;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/imageSmoothingQuality) */
  imageSmoothingQuality: ImageSmoothingQuality;
}


interface CanvasPath {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/arc) */
  arc(x: number, y: number, radius: number, startAngle: number, endAngle: number, counterclockwise?: boolean): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/arcTo) */
  arcTo(x1: number, y1: number, x2: number, y2: number, radius: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/bezierCurveTo) */
  bezierCurveTo(cp1x: number, cp1y: number, cp2x: number, cp2y: number, x: number, y: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/closePath) */
  closePath(): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/ellipse) */
  ellipse(x: number, y: number, radiusX: number, radiusY: number, rotation: number, startAngle: number, endAngle: number, counterclockwise?: boolean): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/lineTo) */
  lineTo(x: number, y: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/moveTo) */
  moveTo(x: number, y: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/quadraticCurveTo) */
  quadraticCurveTo(cpx: number, cpy: number, x: number, y: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/rect) */
  rect(x: number, y: number, w: number, h: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/roundRect) */
  roundRect(x: number, y: number, w: number, h: number, radii?: number | DOMPointInit | (number | DOMPointInit)[]): void;
}


interface CanvasPathDrawingStyles {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/lineCap) */
  lineCap: CanvasLineCap;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/lineDashOffset) */
  lineDashOffset: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/lineJoin) */
  lineJoin: CanvasLineJoin;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/lineWidth) */
  lineWidth: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/miterLimit) */
  miterLimit: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/getLineDash) */
  getLineDash(): number[];
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/setLineDash) */
  setLineDash(segments: Iterable<number>): void;
}

interface CanvasRect {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/clearRect) */
  clearRect(x: number, y: number, w: number, h: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fillRect) */
  fillRect(x: number, y: number, w: number, h: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/strokeRect) */
  strokeRect(x: number, y: number, w: number, h: number): void;
}

interface CanvasShadowStyles {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/shadowBlur) */
  shadowBlur: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/shadowColor) */
  shadowColor: CSSColor;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/shadowOffsetX) */
  shadowOffsetX: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/shadowOffsetY) */
  shadowOffsetY: number;
}

interface CanvasState {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/reset) */
  reset(): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/restore) */
  restore(): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/save) */
  save(): void;

  // UNIMPLEMENTED
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/isContextLost) */
  // isContextLost(): boolean;
}

interface CanvasText {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fillText) */
  fillText(text: string, x: number, y: number, maxWidth?: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/measureText) */
  measureText(text: string): TextMetrics;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/strokeText) */
  strokeText(text: string, x: number, y: number, maxWidth?: number): void;
}

interface CanvasTextDrawingStyles {
    /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/direction) */
    direction: CanvasDirection;
    /**
     * A CSS `font` shorthand string, e.g. `"italic bold 24px/1.5 Palatino, serif"`.
     *
     * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/font)
     */
    font: string;
    /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fontStretch) */
    fontStretch: CanvasFontStretch;
    /**
     * A CSS length, e.g. `"2px"` or `"0.1em"`. Accepts `px`, `pt`, `pc`, `in`, `cm`, `mm`, `q`,
     * `em`, and `rem` (`em`/`rem` resolve against the current font size); other units are ignored.
     *
     * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/letterSpacing)
     */
    letterSpacing: string;
    /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/textAlign) */
    textAlign: CanvasTextAlign;
    /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/textBaseline) */
    textBaseline: CanvasTextBaseline;
    /**
     * A CSS length, e.g. `"4px"` or `"0.25em"`. Accepts `px`, `pt`, `pc`, `in`, `cm`, `mm`, `q`,
     * `em`, and `rem` (`em`/`rem` resolve against the current font size); other units are ignored.
     *
     * [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/wordSpacing)
     */
    wordSpacing: string;

    // UNIMPLEMENTED
    /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/textRendering) */
    // textRendering: CanvasTextRendering;
    /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fontKerning) */
    // fontKerning: CanvasFontKerning;
    /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/fontVariantCaps) */
    // fontVariantCaps: CanvasFontVariantCaps;
}


interface CanvasTransform {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/getTransform) */
  getTransform(): DOMMatrix;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/resetTransform) */
  resetTransform(): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/rotate) */
  rotate(angle: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/scale) */
  scale(x: number, y: number): void;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/setTransform) */
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void;

  /** transform argument extensions (accept DOMMatrix & matrix-like objectx, not just param lists) */
  setTransform(transform?: Matrix): void

  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/transform) */
  transform(a: number, b: number, c: number, d: number, e: number, f: number): void
  transform(transform: Matrix): void

  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/translate) */
  translate(x: number, y: number): void;
}

/**
 * The CanvasRenderingContext2D interface, part of the Canvas API, provides the 2D rendering context for the drawing surface of a <canvas> element. It is used for drawing shapes, text, images, and other objects.
 *
 * - [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D)
 * - [Skia Canvas Docs](https://skia-canvas.org/api/context)
 */
export interface CanvasRenderingContext2D extends CanvasCompositing, CanvasDrawImage, CanvasDrawPath, CanvasFillStrokeStyles, CanvasFilters, CanvasImageData, CanvasImageSmoothing, CanvasPath, CanvasPathDrawingStyles, CanvasRect, CanvasShadowStyles, CanvasState, CanvasText, CanvasTextDrawingStyles, CanvasTransform {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/CanvasRenderingContext2D/canvas) */
  readonly canvas: Canvas
  fontVariant: FontVariantSetting
  fontHinting: boolean
  fontSmoothing: boolean
  fontSynthesis: boolean
  textWrap: boolean
  textDecoration: TextDecorationSetting
  lineDashMarker: Path2D | null
  lineDashFit: "move" | "turn" | "follow"

  // skia/chrome beziers & convenience methods
  get currentTransform(): DOMMatrix
  set currentTransform(matrix: Matrix)
  createProjection(quad: QuadOrRect, basis?: QuadOrRect): DOMMatrix
  conicCurveTo(cpx: number, cpy: number, x: number, y: number, weight: number): void
  getContextAttributes(): {alpha: boolean, colorSpace: ColorSpace, desynchronized: boolean, willReadFrequently: boolean}

  // add optional maxWidth to work in conjunction with textWrap
  measureText(text: string, maxWidth?: number): TextMetrics
  outlineText(text: string, maxWidth?: number): Path2D | null
}

declare var CanvasRenderingContext2D: {
  prototype: CanvasRenderingContext2D;
  new(): CanvasRenderingContext2D;
};

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

/**
 * This Canvas 2D API interface is used to declare a path that can then be used on a CanvasRenderingContext2D object. The path methods of the CanvasRenderingContext2D interface are also present on this interface, which gives you the convenience of being able to retain and replay your path whenever desired.
 *
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/Path2D)
 */
interface Path2D extends CanvasPath {
  readonly bounds: Path2DBounds
  readonly edges: readonly Path2DEdge[]
  readonly length: number
  d: string

  /**
   * Adds the path given by the argument to the path
   *
   * [MDN Reference](https://developer.mozilla.org/docs/Web/API/Path2D/addPath)
   */
  addPath(path: Path2D, transform?: DOMMatrix2DInit): void;

  contains(x: number, y: number): boolean
  conicCurveTo(
    cpx: number,
    cpy: number,
    x: number,
    y: number,
    weight: number
  ): void

  complement(otherPath: Path2D): Path2D
  difference(otherPath: Path2D): Path2D
  intersect(otherPath: Path2D): Path2D
  union(otherPath: Path2D): Path2D
  xor(otherPath: Path2D): Path2D
  interpolate(otherPath: Path2D, weight: number): Path2D

  jitter(segmentLength: number, amount: number, seed?: number): Path2D
  offset(dx: number, dy: number): Path2D
  points(step?: number): readonly [x: number, y: number][]
  positionAt(distance: number): {x: number, y: number} | null
  tangentAt(distance: number): number | null
  normalAt(distance: number): number | null
  round(radius: number): Path2D
  simplify(rule?: "nonzero" | "evenodd"): Path2D
  slice(from?: number, to?: number, inverted?: boolean): Path2D
  transform(transform: Matrix): Path2D;
  transform(a: number, b: number, c: number, d: number, e: number, f: number): Path2D;
  trim(start: number, end: number, inverted?: boolean): Path2D;
  trim(start: number, inverted?: boolean): Path2D;

  unwind(): Path2D
}

declare var Path2D: {
  prototype: Path2D;
  new(path?: Path2D | string): Path2D;
}

//
// Typography
//


/**
 * The dimensions of a piece of text in the canvas, as created by the CanvasRenderingContext2D.measureText() method.
 *
 * [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics)
 */
interface TextMetrics {
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/actualBoundingBoxAscent) */
  readonly actualBoundingBoxAscent: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/actualBoundingBoxDescent) */
  readonly actualBoundingBoxDescent: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/actualBoundingBoxLeft) */
  readonly actualBoundingBoxLeft: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/actualBoundingBoxRight) */
  readonly actualBoundingBoxRight: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/alphabeticBaseline) */
  readonly alphabeticBaseline: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/emHeightAscent) */
  readonly emHeightAscent: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/emHeightDescent) */
  readonly emHeightDescent: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/fontBoundingBoxAscent) */
  readonly fontBoundingBoxAscent: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/fontBoundingBoxDescent) */
  readonly fontBoundingBoxDescent: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/hangingBaseline) */
  readonly hangingBaseline: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/ideographicBaseline) */
  readonly ideographicBaseline: number;
  /** [MDN Reference](https://developer.mozilla.org/docs/Web/API/TextMetrics/width) */
  readonly width: number;

  /** Individual metrics for each line (only applicable when context's textWrap is set to `true` ) */
  readonly lines: TextMetricsLine[]
}

declare var TextMetrics: {
  prototype: TextMetrics;
  new(): TextMetrics;
};

export interface TextMetricsLine {
  /** Left edge of line bounding box */
  readonly x: number
  /** Top edge of line bounding box */
  readonly y: number
  /** Width of line bounding box */
  readonly width: number
  /** Height of line bounding box */
  readonly height: number
  /** Vertical position of currently selected textBaseline */
  readonly baseline: number
  /** Vertical position of highest ascent for all fonts used in line */
  readonly ascent: number
  /** Vertical position of lowest descent for all fonts used in line */
  readonly descent: number
  /** Vertical position of hanging baseline (irrespective of current textBaseline setting) */
  readonly hangingBaseline: number
  /** Vertical position of alphabetic baseline (irrespective of current textBaseline setting) */
  readonly alphabeticBaseline: number
  /** Vertical position of ideographic baseline (irrespective of current textBaseline setting) */
  readonly ideographicBaseline: number
  /** Character index into source string of the beginning of this line */
  readonly startIndex: number
  /** Character index into source string of the end of this line */
  readonly endIndex: number
  /** Array of dimensions & metrics for each single-font subsection of the line */
  readonly runs: TextMetricsRun[]
}

export interface TextMetricsRun {
  /** Left edge of single-font run of characters */
  readonly x: number
  /** Top edge of single-font run of characters */
  readonly y: number
  /** Width of single-font run of characters */
  readonly width: number
  /** Height of single-font run of characters */
  readonly height: number
  /** Name of font family used in this run */
  readonly family: string
  /** Vertical position of this font's ascent metric */
  readonly ascent: number
  /** Vertical position of this font's descent metric */
  readonly descent: number
  /** Vertical position of this font's capital letters */
  readonly capHeight: number
  /** Vertical position of this font's ascender-less letters */
  readonly xHeight: number
  /** Vertical position of the stroke used for underlines */
  readonly underline: number
  /** Vertical position of the stroke used for strikethroughs */
  readonly strikethrough: number
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

interface FontLibrary {
  families: readonly string[]
  family(name: string): FontFamily | undefined
  has(familyName: string): boolean

  use(familyName: string, fontPaths?: string | readonly string[]): Font[]
  use(fontPaths: readonly string[]): Font[]
  use(
    families: Record<string, readonly string[] | string>
  ): Record<string, Font[]>

  reset(): void
}

export const FontLibrary: FontLibrary

//
// Window & App
//

import { EventEmitter } from "stream";
export type EventLoopMode = "node" | "native"
export type TextInputType = "insertText" | "deleteContentBackward" | "deleteContentForward" | "insertLineBreak" | "insertCompositionText"
export type FitStyle = "none" | "contain-x" | "contain-y" | "contain" | "cover" | "fill" | "scale-down" | "resize"
export type CursorStyle = "default" | "crosshair" | "pointer" | "move" | "text" | "wait" | "help" | "progress" | "not-allowed" | "context-menu" |
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
  background?: CSSColor
  fullscreen?: boolean
  borderless?: boolean
  resizable?: boolean
  visible?: boolean
  cursor?: CursorStyle
  canvas?: Canvas
} & TextOptions

type MouseEventProps = {
  x: number;
  y: number;
  pageX: number;
  pageY: number;
  button: number;
  buttons: number,
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
  /** Suppress the default keybindings (window-close, fullscreen-toggle, etc.) */
  preventDefault(): void
}

type PointerEventProps = {
  pointerId: number
  pointerType: "mouse" | "touch"
  isPrimary: boolean
  pressure: number
  tangentialPressure: number
  width: number
  height: number
  tiltX: number
  tiltY: number
  twist: number
  button: number
  buttons: number
  x: number
  y: number
  pageX: number
  pageY: number
  ctrlKey: boolean
  altKey: boolean
  metaKey: boolean
  shiftKey: boolean
}

type CoalescedPointerEventProps = PointerEventProps & {
  getCoalescedEvents(): PointerEventProps[]
}

type CompositionEventProps = {
  data: string
  locale: string | undefined
}

type WindowEvents = {
  mousedown: MouseEventProps
  mouseup: MouseEventProps
  mousemove: MouseEventProps
  mouseenter: MouseEventProps
  mouseleave: MouseEventProps
  pointerdown: CoalescedPointerEventProps
  pointerup: CoalescedPointerEventProps
  pointermove: CoalescedPointerEventProps
  pointerover: PointerEventProps
  pointerenter: PointerEventProps
  pointerout: PointerEventProps
  pointerleave: PointerEventProps
  pointercancel: PointerEventProps
  keydown: KeyboardEventProps
  keyup: KeyboardEventProps
  input: {
    data: string
    inputType: TextInputType
  };
  compositionstart: CompositionEventProps
  compositionupdate: CompositionEventProps
  compositionend: CompositionEventProps
  wheel: { deltaX: number; deltaY: number }
  fullscreen: { enabled: boolean }
  move: { left: number; top: number }
  resize: { height: number; width: number }
  frame: { frame: number }
  draw: { frame: number }
  blur: {}
  focus: {}
  setup: {}
  close: {}
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

  readonly id: number
  readonly ctx: CanvasRenderingContext2D
  canvas: Canvas
  visible: boolean
  fullscreen: boolean
  borderless: boolean
  resizable: boolean
  title: string
  cursor: CursorStyle
  fit: FitStyle
  left: number
  top: number
  width: number
  height: number
  page: number
  background: CSSColor
  readonly closed: boolean

  open(): void
  close(): void

  requestAnimationFrame(callback: (frame: number) => void): number
  cancelAnimationFrame(id: number): void
}

export interface App extends EventEmitter<{
  "idle": [{type: "idle", target: App}]
}>{
  readonly windows: Window[]
  readonly running: boolean
  /** @deprecated Event loop modes have been removed: the node event loop is now always available */
  get eventLoop(): undefined
  /** @deprecated Event loop modes have been removed: the node event loop is now always available */
  set eventLoop(mode: EventLoopMode)
  fps: number

  launch(): Promise<undefined>
  quit(): void
}

export const App: App
