/// <reference types="node" />

export interface RenderOptions {
  page?: number
  density?: number
  quality?: number | undefined
  outline?: boolean
}

export interface SaveOptions extends RenderOptions {
  format?: string | undefined
  matte?: string
}

export class Canvas {
  /** @internal */
  static contexts: WeakMap<Canvas, readonly CanvasRenderingContext2D[]>

  constructor(width?: number, height?: number)

  /**
   * Cast this object as a {@link SyncCanvas} and set the `async` property to false to get the synchronous behaviours.
   *
   * @example
   * import { Canvas, SyncCanvas } from "."
   * const myCanvas = new Canvas() as any as SyncCanvas
   * myCanvas.async = false
   * const result = myCanvas.toBuffer("png") // now these functions return synchronously
   */
  async: true

  get width(): number
  get height(): number

  getContext(type?: "2d"): CanvasRenderingContext2D

  get pages(): readonly CanvasRenderingContext2D[]

  get pdf(): Promise<Buffer>
  get svg(): Promise<Buffer>
  get jpg(): Promise<Buffer>
  get png(): Promise<Buffer>

  newPage(width?: number, height?: number): CanvasRenderingContext2D

  saveAs(filename: string, options?: SaveOptions): Promise<void>
  toBuffer(format: string, options?: RenderOptions): Promise<Buffer>
  toDataURL(format: string, options?: RenderOptions): Promise<string>
}

export type SyncCanvas = {
  [P in keyof Canvas]: Canvas[P] extends Promise<infer Value>
    ? Value // Promise getter to synchronous getter
    : Canvas[P] extends (...args: infer Args) => Promise<infer Return>
    ? (...args: Args) => Return // Async functions to sync functions
    : P extends "async"
    ? false // `async` property is now false
    : Canvas[P] // Everything else stays the same
}

export interface CreateTextureOptions {
  path?: Path2D
  line?: number
  color?: string
  angle?: number
  offset?: [x: number, y: number]
}

export class CanvasRenderingContext2D extends globalThis.CanvasRenderingContext2D {
  // @ts-expect-error We're rewriting the canvas property in a non-typesafe way
  readonly canvas: Canvas

  fontVariant: string
  textTracking: number
  textWrap: boolean

  lineDashFit: "move" | "turn" | "follow"
  lineDashMarker: Path2D

  conicCurveTo(
    cpx: number,
    cpy: number,
    x: number,
    y: number,
    weight: number
  ): void

  createTexture(
    spacing: number | [width: number, height: number],
    options?: CreateTextureOptions
  ): CanvasTexture

  measureText(text: string, maxWidth?: number): TextMetrics

  outlineText(text: string): Path2D

  // @ts-expect-error We're rewriting the canvas property in a non-typesafe way
  strokeStyle:
    | globalThis.CanvasRenderingContext2D["strokeStyle"]
    | CanvasTexture
  // @ts-expect-error We're rewriting the canvas property in a non-typesafe way
  fillStyle: globalThis.CanvasRenderingContext2D["fillStyle"] | CanvasTexture

  /** @internal */
  lineStyle: string
}

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

export class CanvasTexture {}
export class CanvasGradient extends globalThis.CanvasGradient {}
export class CanvasPattern extends globalThis.CanvasPattern {}
export class DOMMatrix extends globalThis.DOMMatrix {}
export class Image extends globalThis.Image {}
export class ImageData extends globalThis.ImageData {}

export interface Path2DBounds {
  readonly top: number
  readonly left: number
  readonly bottom: number
  readonly right: number
  readonly width: number
  readonly height: number
}

type _Values<T extends {}> = T[keyof T]
export type Path2DEdge = [verb: string, ...args: number[]]

export class Path2D extends globalThis.Path2D {
  readonly bounds: Path2DBounds
  d: string
  readonly edges: readonly Path2DEdge[]

  contains(x: number, y: number): boolean

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

  transform(matrix: DOMMatrix): Path2D
  transform(
    a: number,
    b: number,
    c: number,
    d: number,
    e: number,
    f: number
  ): Path2D

  trim(start: number, end?: number, inverted?: boolean): Path2D

  unwind(): Path2D

  conicCurveTo(
    cpx: number,
    cpy: number,
    x: number,
    y: number,
    weight: number
  ): void
}

export function loadImage(src: string | Buffer): Promise<Image>

export interface FontFamily {
  family: string
  weights: number[]
  widths: string[]
  styles: string[]
}

export interface FontVariant {
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

  use(familyName: string, fontPaths?: string | readonly string[]): FontVariant[]
  use(fontPaths: readonly string[]): FontVariant[]
  use(
    families: Record<string, readonly string[] | string>
  ): Record<string, FontVariant[] | FontVariant>
}

export const FontLibrary: FontLibrary
