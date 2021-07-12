# Skia Canvas

Skia Canvas is a browser-less implementation of the HTML Canvas drawing API for Node.js. It is based on Google’s [Skia](https://skia.org) graphics engine and as a result produces very similar results to Chrome’s `<canvas>` element.

While the primary goal of this project is to provide a reliable emulation of the [standard API](https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API) according to the [spec](https://html.spec.whatwg.org/multipage/canvas.html), it also extends it in a number of areas that are more relevant to the generation of static graphics files rather than ‘live’ display in a browser.

In particular, Skia Canvas:

  - is fast and compact since all the heavy lifting is done by native code written in Rust and C++
  - can generate output in both raster (JPEG & PNG) and vector (PDF & SVG) image formats
  - can save images to [files][saveAs], return them as [Buffers][toBuffer], or encode [dataURL][toDataURL_ext] strings
  - uses native threads and [EventQueues](https://docs.rs/neon/0.8.2-napi/neon/event/struct.EventQueue.html) for asynchronous rendering and file I/O
  - can create [multiple ‘pages’][newPage] on a given canvas and then [output][saveAs] them as a single, multi-page PDF or an image-sequence saved to multiple files
  - can [simplify][p2d_simplify], [blunt][p2d_round], [combine][bool-ops], and [atomize][p2d_points] bézier paths using [efficient](https://www.youtube.com/watch?v=OmfliNQsk88) boolean operations or point-by-point [interpolation][p2d_interpolate]
  - fully supports the [CSS filter effects][filter] image processing operators
  - offers rich typographic control including:

    - multi-line, [word-wrapped](#textwrap) text
    - line-by-line [text metrics](#measuretextstr-width)
    - small-caps, ligatures, and other opentype features accessible using standard [font-variant](#fontvariant) syntax
    - proportional letter-spacing (a.k.a. [‘tracking’](#texttracking)) and leading
    - support for [variable fonts][VariableFonts] and transparent mapping of weight values
    - use of non-system fonts [loaded](#usefamilyname-fontpaths) from local files


### Basic Usage
```js
const {Canvas, loadImage} = require('skia-canvas'),
      rand = n => Math.floor(n * Math.random());

let canvas = new Canvas(600, 600),
    ctx = canvas.getContext("2d"),
    {width, height} = canvas;

// draw a sea of blurred dots filling the canvas
ctx.filter = 'blur(12px) hue-rotate(20deg)'
for (let i=0; i<800; i++){
  ctx.fillStyle = `hsl(${rand(40)}deg, 80%, 50%)`
  ctx.beginPath()
  ctx.arc(rand(width), rand(height), rand(20)+5, 0, 2*Math.PI)
  ctx.fill()
}

// mask all of the dots that don't overlap with the text
ctx.filter = 'none'
ctx.globalCompositeOperation = 'destination-in'
ctx.font='italic 480px Times, DejaVu Serif'
ctx.textAlign = 'center'
ctx.textBaseline = 'top'
ctx.fillText('¶', width/2, 0)

// draw a background behind the clipped text
ctx.globalCompositeOperation = 'destination-over'
ctx.fillStyle = '#182927'
ctx.fillRect(0,0, width,height)

// save the graphic...
canvas.saveAs("pilcrow.png")
// ...or use a shorthand for canvas.toBuffer("png")
fs.writeFileSync("pilcrow.png", canvas.png)
// ...or embed it in a string
console.log(`<img src="${canvas.toDataURL("png")}">`)
```

## Installation

If you’re running on a supported platform, installation should be as simple as:
```console
$ npm install skia-canvas
```

This will download a pre-compiled library from the project’s most recent [release](https://github.com/samizdatco/skia-canvas/releases).

### Dependencies

Nearly everything you need is statically linked into the library.

A notable exception is the [Fontconfig](https://www.freedesktop.org/wiki/Software/fontconfig/) library (and its associated [FreeType](https://www.freetype.org) renderer) which must be installed separately if you’re running on Linux.


### Platform Support

The underlying Rust library uses [N-API](https://nodejs.org/api/n-api.html) v6 which allows it to run on Node.js versions:
  - 10.20+
  - 12.17+
  - 14.0, 15.0, and later

Pre-compiled binaries are available for:

  - Linux (x86)
  - macOS (x86 & Apple silicon)
  - Windows (x86)

### Running in Docker

The library is compatible with Linux systems using glibc 2.24 or later. Currently the `rust-skia` library [will not compile](https://github.com/rust-skia/rust-skia/issues/356) against the [musl](https://musl.libc.org) library used by Alpine Linux—though this may change in the future. For now, if you are setting up a [Dockerfile](https://nodejs.org/en/docs/guides/nodejs-docker-webapp/) that uses [`node`](https://hub.docker.com/_/node) as its basis, you’ll want to set your `FROM` image to one of the (Debian-derived) defaults like `node:16`, `node:14`, `node:12`, `node:buster`, `node:stretch`, or simply:
```dockerfile
FROM node
```

You can also use the ‘slim’ image if you manually install fontconfig:

```dockerfile
FROM node:slim
RUN apt-get update && apt-get install -y -q --no-install-recommends libfontconfig1 
```

### Compiling from Source

If prebuilt binaries aren’t available for your system you’ll need to compile the portions of this library that directly interface with Skia.

Start by installing:

  1. The [Rust compiler](https://www.rust-lang.org/tools/install) and cargo package manager using [`rustup`](https://rust-lang.github.io/rustup/)
  2. A C compiler toolchain like LLVM/Clang or MSVC
  3. Python 2.7 (used by Skia's [build process](https://skia.org/docs/user/build/))
  4. On Linux: Fontconfig, OpenSSL, X11, and Mesa

[Detailed instructions](https://github.com/rust-skia/rust-skia#building) for setting up these dependencies on different operating systems can be found in the ‘Building’ section of the Rust Skia documentation. Once all the necessary compilers and libraries are present, running `npm run build` will give you a usable library (after a fairly lengthy compilation process).



# API Documentation

> Documentation for the key classes and their attributes are listed below—properties are printed in **bold** and methods have parentheses attached to the name. The instances where Skia Canvas’s behavior goes beyond the standard are marked by a ⚡ symbol, linking to further details below.

The library exports a number of classes emulating familiar browser objects including:

 - [Canvas][Canvas] ⧸[⚡](#canvas)
 - [CanvasGradient][CanvasGradient]
 - [CanvasPattern][CanvasPattern]
 - [CanvasRenderingContext2D][CanvasRenderingContext2D] ⧸[⚡](#canvasrenderingcontext2d)
 - [DOMMatrix][DOMMatrix]
 - [Image][Image]
 - [ImageData][ImageData]
 - [Path2D][Path2D] ⧸[⚡](#path2d)

In addition, the module contains:

- [loadImage()](#loadimage) a utility function for loading `Image` objects asynchronously
- [FontLibrary](#fontlibrary) a class allowing you to inspect the system’s installed fonts and load additional ones


## Canvas

The Canvas object is a stand-in for the HTML `<canvas>` element. It defines image dimensions and provides a [rendering context](#canvasrenderingcontext2d) to draw to it. Once you’re ready to save or display what you’ve drawn, the canvas can [save][saveAs] it to a file, or hand it off to you as a [data buffer][toBuffer] or [string][toDataURL_ext] to process manually.


| Image Dimensions             | Rendering Contexts            | Output                                           |
| --                           | --                            | --                                               |
| [**width**][canvas_width]    | [**pages**][canvas_pages] ⚡  | [**async**][canvas_async]  ⚡                    |
| [**height**][canvas_height]  | [getContext()][getContext]    | [**pdf**, **png**, **svg**, **jpg**][shorthands] ⚡ |
|                              | [newPage()][newPage] ⚡       | [saveAs()][saveAs] ⚡                            |
|                              |                               | [toBuffer()][toBuffer] ⚡                        |
|                              |                               | [toDataURL()][toDataURL_mdn] [⚡][toDataURL_ext] |

[canvas_width]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/width
[canvas_height]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/height
[canvas_async]: #async
[canvas_pages]: #pages
[getContext]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/getContext
[saveAs]: #saveasfilename-page-format-density1-quality092-outlinefalse
[toBuffer]: #tobufferformat-page-density-quality-outline
[newPage]: #newpagewidth-height
[toDataURL_mdn]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/toDataURL
[toDataURL_ext]: #todataurlformat-page-density-quality-outline
[shorthands]: #pdf-svg-jpg-and-png

#### Creating new `Canvas` objects

Rather than calling a DOM method to create a new canvas, you can simply call the `Canvas` constructor with the width and height (in pixels) of the image you’d like to begin drawing.

```js
let defaultCanvas = new Canvas() // without arguments, defaults to 300 × 150 px
let squareCanvas = new Canvas(512, 512) // creates a 512 px square
```

##### PROPERTIES

#### `.async`

When the canvas renders images and writes them to disk, it does so in a background thread so as not to block execution within your script. As a result you’ll generally want to deal with the canvas from within an `async` function and be sure to use the `await` keyword when accessing any of its output methods or shorthand properties:
  - [`saveAs()`][saveAs]
  - [`toBuffer()`][toBuffer]
  - [`toDataURL()`][toDataURL_ext]
  - [`.pdf`, `.svg`, `.jpg`, and `.png`][shorthands]

In cases where this is not the desired behavior, you can switch these methods into a synchronous mode for a particular canvas by setting its `async` property to `false`. For instance, both of the example functions below will generate PNG & PDF from the canvas, though the first will be more efficient (particularly for parallel contexts like request-handlers in an HTTP server or batch exports):
```js

let canvas = new Canvas()
console.log(canvas.async) // -> true by default

async function normal(){
  let pngURL = await canvas.toDataURL("png")
  let pdfBuffer = await canvas.pdf
}

function synchronous(){
  canvas.async = false // switch into synchronous mode
  let pngURL = canvas.toDataURL("png")
  let pdfBuffer = canvas.pdf
}
```




#### `.pages`

The canvas’s `.pages` attribute is an array of [`CanvasRenderingContext2D`][CanvasRenderingContext2D] objects corresponding to each ‘page’ that has been created. The first page is added when the canvas is initialized and additional ones can be added by calling the `newPage()` method. Note that all the pages remain drawable persistently, so you don’t have to constrain yourself to modifying the ‘current’ page as you render your document or image sequence.

#### `.pdf`, `.svg`, `.jpg`, and `.png`

These properties are syntactic sugar for calling the `toBuffer()` method. Each returns a Node [`Buffer`][Buffer] object with the contents of the canvas in the given format. If more than one page has been added to the canvas, only the most recent one will be included unless you’ve accessed the `.pdf` property in which case the buffer will contain a multi-page PDF.

##### METHODS

#### `newPage(width, height)`

This method allows for the creation of additional drawing contexts that are fully independent of one another but will be part of the same output batch. It is primarily useful in the context of creating a multi-page PDF but can be used to create multi-file image-sequences in other formats as well. Creating a new page with a different size than the previous one will update the parent Canvas object’s `.width` and `.height` attributes but will not affect any other pages that have been created previously.

The method’s return value is a `CanvasRenderingContext2D` object which you can either save a reference to or recover later from the `.pages` array.

#### `saveAs(filename, {page, format, density=1, quality=0.92, outline=false})`

The `saveAs` method takes a file path and writes the canvas’s current contents to disk. If the filename ends with an extension that makes its format clear, the second argument is optional. If the filename is ambiguous, you can pass an options object with a `format` string using names like `"png"` and `"jpeg"` or a full mime type like `"application/pdf"`.

The way multi-page documents are handled depends on the `filename` argument. If the filename contains the string `"{}"`, it will be used as template for generating a numbered sequence of files—one per page. If no curly braces are found in the filename, only a single file will be saved. That single file will be multi-page in the case of PDF output but for other formats it will contain only the most recently added page.

An integer can optionally be placed between the braces to indicate the number of padding characters to use for numbering. For instance `"page-{}.svg"` will generate files of the form `page-1.svg` whereas `"frame-{4}.png"` will generate files like `frame-0001.png`.

##### page
The optional `page` argument accepts an integer that allows for the individual selection of pages in a multi-page canvas. Note that page indexing starts with page 1 **not** 0. The page value can also be negative, counting from the end of the canvas’s `.pages` array. For instance, `.saveAs("currentPage.png", {page:-1})` is equivalent to omitting `page` since they both yield the canvas’s most recently added page.

##### density
By default, the images will be at a 1:1 ratio with the canvas's `width` and `height` dimensions (i.e., a 72 × 72 canvas will yield a 72 pixel × 72 pixel bitmap). But with screens increasingly operating at higher densities, you’ll frequently want to generate images where an on-canvas 'point' may occupy multiple pixels. The optional `density` argument allows you to specify this magnification factor using an integer ≥1. As a shorthand, you can also select a density by choosing a filename using the `@nx` naming convention:

```js
canvas.saveAs('image.png', {density:2}) // choose the density explicitly
canvas.saveAs('image@3x.png') // equivalent to setting the density to 3
```

##### quality
The `quality` option is a number between 0 and 1.0 that controls the level of JPEG compression both when making JPEG files directly and when embedding them in a PDF. If omitted, quality will default to 0.92.

##### outline
When generating SVG output containing text, you have two options for how to handle the fonts that were used. By default, SVG files will contain `<text>` elements that refer to the fonts by name in the embedded stylesheet. This requires that viewers of the SVG have the same fonts available on their system (or accessible as webfonts). Setting the optional `outline` argument to `true` will trace all the letterforms and ‘burn’ them into the file as bézier paths. This will result in a much larger file (and one in which the original text strings will be unrecoverable), but it will be viewable regardless of the specifics of the system it’s displayed on.

#### `toBuffer(format, {page, density, quality, outline})`

Node [`Buffer`][Buffer] objects containing various image formats can be created by passing either a format string like `"svg"` or a mime-type like `"image/svg+xml"`. An ‘@’ suffix can be added to the format string to specify a pixel-density (for instance, `"jpg@2x"`). The optional arguments behave the same as in the `saveAs` method.

#### `toDataURL(format, {page, density, quality, outline})`

This method accepts the same arguments and behaves similarly to `.toBuffer`. However instead of returning a Buffer, it returns a string of the form `"data:<mime-type>;base64,<image-data>"` which can be used as a `src` attribute in `<img>` tags, embedded into CSS, etc.


## CanvasRenderingContext2D

Most of your interaction with the canvas will actually be directed toward its ‘rendering context’, a supporting object you can acquire by calling the canvas’s [getContext()](https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/getContext) and [newPage()][newPage] methods.


| Canvas State                             | Drawing Primitives                           | Stroke & Fill Style                  | Compositing Effects                                      |
|------------------------------------------|----------------------------------------------|--------------------------------------|----------------------------------------------------------|
| [**canvas**][canvas_attr] ⧸[⚡](#canvas) | [drawImage()][drawImage()]                   | [**fillStyle**][fillStyle]           | [**filter**][filter]                                     |
| [beginPath()][beginPath()]               | [clearRect()][clearRect()]                   | [**lineCap**][lineCap]               | [**globalAlpha**][globalAlpha]                           |
| [isPointInPath()][isPointInPath()]       | [fillRect()][fillRect()]                     | [**lineDashOffset**][lineDashOffset] | [**globalCompositeOperation**][globalCompositeOperation] |
| [isPointInStroke()][isPointInStroke()]   | [strokeRect()][strokeRect()]                 | [**lineJoin**][lineJoin]             | [**shadowBlur**][shadowBlur]                             |
| [save()][save()]                         | [fillText()][fillText()] ⧸[⚡][drawText]     | [**lineWidth**][lineWidth]           | [**shadowColor**][shadowColor]                           |
| [restore()][restore()]                   | [strokeText()][strokeText()] ⧸[⚡][drawText] | [**miterLimit**][miterLimit]         | [**shadowOffsetX**][shadowOffsetX]                       |
| [clip()][clip()]                         | [fill()][fill()]                             | [**strokeStyle**][strokeStyle]       | [**shadowOffsetY**][shadowOffsetY]                       |
|                                          | [stroke()][stroke()]                         | [getLineDash()][getLineDash()]       |                                                          |
|                                          |                                              | [setLineDash()][setLineDash()]       |                                                          |


| Bezier Paths                             | Typography                                                  | Pattern & Image                                    | Transform                                |
|------------------------------------------|-------------------------------------------------------------|----------------------------------------------------|------------------------------------------|
| [moveTo()][moveTo()]                     | [**direction**][direction]                                  | [**imageSmoothingEnabled**][imageSmoothingEnabled] | [**currentTransform**][currentTransform] |
| [lineTo()][lineTo()]                     | [**font**][font] ⧸[⚡](#font)                               | [**imageSmoothingQuality**][imageSmoothingQuality] | [getTransform()][getTransform()]         |
| [arcTo()][arcTo()]                       | [**fontVariant** ⚡](#fontvariant)                          | [createPattern()][createPattern()]                 | [setTransform()][setTransform()]         |
| [bezierCurveTo()][bezierCurveTo()]       | [**textAlign**][textAlign]                                  | [createConicGradient()][createConicGradient()]     | [resetTransform()][resetTransform()]     |
| [conicCurveTo() ⚡][conicCurveTo]        | [**textBaseline**][textBaseline]                            | [createLinearGradient()][createLinearGradient()]   | [transform()][transform()]               |
| [quadraticCurveTo()][quadraticCurveTo()] | [**textTracking** ⚡](#texttracking)                        | [createRadialGradient()][createRadialGradient()]   | [translate()][translate()]               |
| [closePath()][closePath()]               | [**textWrap** ⚡](#textwrap)                                | [createImageData()][createImageData()]             | [rotate()][rotate()]                     |
| [arc()][arc()]                           | [measureText()][measureText()] ⧸[⚡](#measuretextstr-width) | [getImageData()][getImageData()]                   | [scale()][scale()]                       |
| [ellipse()][ellipse()]                   | [outlineText() ⚡][outlineText()]                                                            | [putImageData()][putImageData()]                   |                                          |
| [rect()][rect()]                         |

##### PROPERTIES

#### `.font`

By default any [`line-height`][lineHeight] value included in a font specification (separated from the font size by a `/`) will be preserved but ignored. If the `textWrap` property is set to `true`, the line-height will control the vertical spacing between lines.

#### `.fontVariant`

The context’s [`.font`][font] property follows the CSS 2.1 standard and allows the selection of only a single font-variant type: `normal` vs `small-caps`. The full range of CSS 3 [font-variant][font-variant] values can be used if assigned to the context’s `.fontVariant` property (presuming the currently selected font supports them). Note that setting `.font` will also update the current `.fontVariant` value, so be sure to set the variant *after* selecting a typeface.

#### `.textTracking`

To loosen or tighten letter-spacing, set the `.textTracking` property to an integer representing the amount of space to add/remove in terms of 1/1000’s of an ‘em’ (a.k.a. the current font size). Positive numbers will space out the text (e.g., `100` is a good value for setting all-caps) while negative values will pull the letters closer together (this is only rarely a good idea).

The tracking value defaults to `0` and settings will persist across changes to the `.font` property.

#### `.textWrap`

The standard canvas has a rather impoverished typesetting system, allowing for only a single line of text and an approach to width-management that horizontally scales the letterforms (a type-crime if ever there was one). Skia Canvas allows you to opt-out of this single-line world by setting the `.textWrap` property to `true`. Doing so affects the behavior of the `fillText()`, `strokeText()`, and `measureText()`

##### METHODS

#### `conicCurveTo(cpx, cpy, x, y, weight)`

Adds a line segment connecting the current point to (*x, y*) but curving toward the control point (*cpx, cpy*) along the way. The `weight` argument controls how close the curve will come to the control point. If the weight is `0`, the result will be a straight line from the current point to (*x, y*). With a weight of `1.0`, the function is equivalent to calling `quadraticCurveTo()`. Weights greater than `1.0` will pull the line segment ever closer to the control point.

#### `fillText(str, x, y, [width])` & `strokeText(str, x, y, [width])`

The text-drawing methods’ behavior is mostly standard unless `.textWrap` has been set to `true`, in which case there are 3 main effects:

  1. Manual line breaking via `"\n"` escapes will be honored rather than converted to spaces
  2. The optional `width` argument accepted by `fillText`, `strokeText` and `measureText` will be interpreted as a ‘column width’ and used to word-wrap long lines
  3. The line-height setting in the `.font` value will be used to set the inter-line leading rather than simply being ignored.

Even when `.textWrap` is `false`, the text-drawing methods will never choose a more-condensed weight or otherwise attempt to squeeze your entire string into the measure specified by `width`. Instead the text will be typeset up through the last word that fits and the rest will be omitted. This can be used in conjunction with the `.lines` property of the object returned by `measureText()` to incrementally lay out a long string into, for example, a multi-column layout with an even number of lines in each.

#### `measureText(str, [width])`

The `measureText()` method returns a [TextMetrics][TextMetrics] object describing the dimensions of a run of text *without* actually drawing it to the canvas. Skia Canvas adds an additional property to the metrics object called `.lines` which contains an array describing the geometry of each line individually.

Each element of the array contains an object of the form:
```
{x, y, width, height, baseline, startIndex, endIndex}
```
The `x`, `y`, `width`, and `height` values define a rectangle that fully encloses the text of a given line relative to the ‘origin’ point you would pass to `fillText()` or `strokeText()` (and reflecting the context’s current `.textBaseline` setting).

The `baseline` value is a y-axis offset from the text origin to that particular line’s baseline.

The `startIndex` and `endIndex` values are the indices into the string of the first and last character that were typeset on that line.

#### `outlineText(str)`

The `outlineText()` method typesets a string and returns a Path2D containing the shapes of its character glyphs. It will use the context’s current [`.font`][font], [`.textAlign`][textAlign], and [`.textBaseline`][textBaseline] settings to style the string and will anchor the text relative to the (0, 0) origin point. As a result, you’ll typically want to use the context’s transform-related methods or Path2D’s [`offset()`][p2d_offset] and [`transform()`][p2d_transform] to position the path before drawing it to the canvas.

Note that path-generation uses a more limited typesetting system than [`fillText()`][drawText] and [`strokeText()`][drawText]. As such, it ignores any settings made using the [`.fontVariant`](#fontvariant) or [`.textTracking`](#texttracking) properties and does not support multi-line text (regardless of the current [`.textWrap`](#textwrap) setting).

```js
ctx.textBaseline = 'top'
ctx.font = 'bold 140px Helvetica'
let ampersand = ctx.outlineText('&')

for (let i=0; i<8000; i++){
  let x = Math.random() * 100,
      y = Math.random() * 120;
  ctx.fillStyle = path.contains(x, y) ? 'lightblue' : '#eee'
  ctx.fillRect(x, y, 2, 2)
}
```
![text converted to a Path2D](/test/assets/path/outlineText@2x.png)

## Path2D

The `Path2D` class allows you to create paths independent of a given [Canvas](#canvas) or [graphics context](#canvasrenderingcontext2d). These paths can be modified over time and drawn repeatedly (potentially on multiple canvases).


| Line Segments                              | Shapes                   | Boolean Ops ⚡           | Filters ⚡                       | Geometry ⚡                  |
| --                                         | --                       | --                       | --                               | --                           |
| [moveTo()][p2d_moveTo]                     | [addPath()][p2d_addPath] | [complement()][bool-ops] | [interpolate()][p2d_interpolate] | [**bounds**](#bounds)        |
| [lineTo()][p2d_lineTo]                     | [arc()][p2d_arc]         | [difference()][bool-ops] | [jitter()][p2d_jitter]           | [**edges**](#edges)          |
| [bezierCurveTo()][p2d_bezierCurveTo]       | [arcTo()][p2d_arcTo]     | [intersect()][bool-ops]  | [round()][p2d_round]             | [contains()][p2d_contains]   |
| [conicCurveTo() ⚡][conicCurveTo]          | [ellipse()][p2d_ellipse] | [union()][bool-ops]      | [simplify()][p2d_simplify]       | [points()][p2d_points]       |
| [quadraticCurveTo()][p2d_quadraticCurveTo] | [rect()][p2d_rect]       | [xor()][bool-ops]        | [trim()][p2d_trim]               | [offset()][p2d_offset]       |
| [closePath()][p2d_closePath]               |                          |                          |                                  | [transform()][p2d_transform] |

#### Creating `Path2D` objects

Its constructor can be called without any arguments to create a new, empty path object. It can also accept a string  using [SVG syntax][SVG_path_commands] or a reference to an existing `Path2D` object (which it will return a clone of):
```js
// three identical (but independent) paths
let p1 = new Path2D("M 10,10 h 100 v 100 h -100 Z")
let p2 = new Path2D(p1)
let p3 = new Path2D()
p3.rect(10, 10, 100, 100)
```

#### Drawing paths

A canvas’s context always contains an implicit ‘current’ bézier path which is updated by commands like [lineTo()][lineTo()] and [arcTo()][arcTo()] and is drawn to the canvas by calling [fill()][fill()], [stroke()][stroke()], or [clip()][clip()] without any arguments (aside from an optional [winding][nonzero] [rule][evenodd]). If you start creating a second path by calling [beginPath()][beginPath()] the context discards the prior path, forcing you to recreate it by hand if you need it again later.

You can then use these objects by passing them as the first argument to the context’s `fill()`, `stroke()`, and `clip()` methods (along with an optional second argument specifying the winding rule).


##### PROPERTIES

#### `.bounds`

In the browser, Path2D objects offer very little in the way of introspection—they are mostly-opaque recorders of drawing commands that can be ‘played back’ later on. Skia Canvas offers some additional transparency by allowing you to measure the total amount of space the lines will occupy (though you’ll need to account for the current `lineWidth` if you plan to draw the path with `stroke()`).

The `.bounds` property returns an object defining the minimal rectangle containing the path:
```
{top, left, bottom, right, width, height}
```

#### `.edges`

Returns an array containing each path segment that has been added to the path so far. Each element of the list is an array of the form `["verb", ...points]`, mirroring the calling conventions of both Path2D and the rendering context. As a result, the `edges` may be used to ‘replay’ a sequence of commands such as:
```js
let original = new Path2D()
// ... add some contours to the path

// apply the original path’s edges to a new Path2D
let clone = new Path2D()
for (const [verb, ...pts] of original.edges){
  clone[verb](...pts)
}

// or use the original path’s edges to draw directly to the context
for (const [verb, ...pts] of original.edges){
  ctx[verb](...pts)
}
```

The array is not a verbtaim transcript of the drawing commands that have been called since some commands (e.g., `arc()`) will be converted into an equivalent sequence of bézier curves. The full range of verbs and numbers of point arguments is as follows:

```js
[
  ["moveTo", x, y],
  ["lineTo", x, y],
  ["quadraticCurveTo", cpx, cpy, x, y],
  ["bezierCurveTo", cp1x, cp1y, cp2x, cp2y, x, y],
  ["conicCurveTo", cpx, cpy, x, y, weight],
  ["closePath"]
]
```

##### METHODS

#### `contains(x, y)`

Returns true if the point (*x, y*) is either inside the path or intersects one of its contours.

#### `complement()`, `difference()`, `intersect()`, `union()`, and `xor()`
In addition to creating `Path2D` objects through the constructor, you can use pairs of existing paths *in combination* to generate new paths based on their degree of overlap. Based on the method you choose, a different boolean relationship will be used to construct the new path. In all the following examples we’ll be starting off with a pair of overlapping shapes:
```js
let oval = new Path2D()
oval.arc(100, 100, 100, 0, 2*Math.PI)

let rect = new Path2D()
rect.rect(0, 100, 100, 100)
```
![layered paths](/test/assets/path/operation-none.svg)

We can then create a new path by using one of the boolean operations such as:
```js
let knockout = rect.complement(oval),
    overlap = rect.intersect(oval),
    footprint = rect.union(oval),
    ...
```
![different combinations](/test/assets/path/operations@2x.png)

Note that the `xor` operator is liable to create a path with lines that cross over one another so you’ll get different results when filling it using the [`"evenodd"`][evenodd] winding rule (as shown above) than with [`"nonzero"`][nonzero] (the canvas default).

#### `interpolate(otherPath, weight)`

When two similar paths share the same sequence of ‘verbs’ and differ only in the point arguments passed to them, the `interpolate()` method can combine them in different proportions to create a new path. The `weight` argument controls whether the resulting path resembles the original (at `0.0`), the `otherPath` (at `1.0`), or something in between.

```js
let start = new Path2D()
start.moveTo(-200, 100)
start.bezierCurveTo(-300, 100, -200, 200, -300, 200)
start.bezierCurveTo(-200, 200, -300, 300, -200, 300)

let end = new Path2D()
end.moveTo(200, 100)
end.bezierCurveTo(300, 100, 200, 200, 300, 200)
end.bezierCurveTo(200, 200, 300, 300, 200, 300)

let left = start.interpolate(end, .25),
    mean = start.interpolate(end, .5),
    right = start.interpolate(end, .75)
```
![merging similar paths](/test/assets/path/effect-interpolate@2x.png)


#### `jitter(segmentLength, amount, seed=0)`

The `jitter()` method will return a new Path2D object obtained by breaking the original path into segments of a given length then applying random offsets to the resulting points. Though the modifications are random, they will be consistent between runs based on the specified `seed`. Try passing different integer values for the seed until you get results that you like.

```js
let cube = new Path2D()
cube.rect(100, 100, 100, 100)
cube.rect(150, 50, 100, 100)
cube.moveTo(100, 100)
cube.lineTo(150, 50)
cube.moveTo(200, 100)
cube.lineTo(250, 50)
cube.moveTo(200, 200)
cube.lineTo(250, 150)

let jagged = cube.jitter(1, 2),
    reseed = cube.jitter(1, 2, 1337),
    sketchy = cube.jitter(10, 1)
```
![xkcd-style](/test/assets/path/effect-jitter@2x.png)

#### `offset(dx, dy)`

Returns a copy of the path whose points have been shifted horizontally by `dx` and vertically by `dy`.

#### `points(step=1)`

The `points()` method breaks a path into evenly-sized steps and returns the (*x, y*) positions of the resulting vertices. The `step` argument determines the amount of distance between neighboring points and defaults to 1 px if omitted.


```js
let path = new Path2D()
path.arc(100, 100, 50, 0, 2*Math.PI)
path.rect(100, 50, 50, 50)
path = path.simplify()

for (const [x, y] of path.points(10)){
  ctx.fillRect(x, y, 3, 3)
}
```
![sampling points from a path](/test/assets/path/effect-points@2x.png)

#### `round(radius)`

Calling `round()` will return a new Path2D derived from the original path whose corners have been rounded off to the specified radius.

```js
let spikes = new Path2D()
spikes.moveTo(50, 225)
spikes.lineTo(100, 25)
spikes.lineTo(150, 225)
spikes.lineTo(200, 25)
spikes.lineTo(250, 225)
spikes.lineTo(300, 25)

let snake = spikes.round(80)
```
![no sharp edges](/test/assets/path/effect-round@2x.png)

#### `simplify()`

In cases where the contours of a single path overlap one another, it’s often useful to have a way of effectively applying a `union` operation *within* the path itself. The `simplify` method traces the path and returns a new copy that removes any overlapping segments:

```js
let cross = new Path2D(`
  M 10,50 h 100 v 20 h -100 Z
  M 50,10 h 20 v 100 h -20 Z
`)
let uncrossed = cross.simplify()
```
![different combinations](/test/assets/path/effect-simplify@2x.png)

#### `transform(matrix)` or `transform(a, b, c, d, e, f)`

Returns a new copy of the path whose points have been modified by the specified transform matrix. The matrix’s terms can be passed individually as 6 numbers or as a [DOMMatrix][DOMMatrix] object. The original path remains unmodified.

#### `trim(start, end, inverted)`

The `trim()` method returns a new Path2D which contains only a portion of the original path. The `start` and `end` arguments specify percentages of the original contour as numbers between `0` and `1.0`. If both arguments are provided, the new path will be a continuous contour connecting those endpoints. If the `inverted` argument is set to `true`, the new path will contain everything from the original **except** the region between the specified endpoints.

Passing a single positive number implicitly sets the starting point to `0.0` and uses the supplied argument as the `end`. Passing a negative value sets the ending point to `1.0` and uses the argument as the `start` value. In either case, you can include `inverted` as the second argument to flip the selected contour.

```js
let orig = new Path2D()
orig.arc(100, 100, 50, Math.PI, 0)

let middle = orig.trim(.25, .75),
    endpoints = orig.trim(.25, .75, true),
    left = orig.trim(.25),
    right = orig.trim(-.25)

```
![trimmed subpaths](/test/assets/path/effect-trim@2x.png)

## Utilities

### loadImage()

The included [Image][Image] object behaves just like the one in browsers, which is to say that loading images can be verbose, fiddly, and callback-heavy. The `loadImage()` utility method wraps image loading in a [Promise][Promise], allowing for more concise initialization. For instance the following snippets are equivalent:

```js
let img = new Image()
img.onload = function(){
  ctx.drawImage(img, 100, 100)
}
img.src = 'https://example.com/icon.png'
```

```js
let img = await loadImage('https://example.com/icon.png')
ctx.drawImage(img, 100, 100)
```

In addition to HTTP URLs, both `loadImage()` and the `Image.src` attribute will also accept [data URLs][DataURL], local file paths, and [Buffer][Buffer] objects.

### FontLibrary

The `FontLibrary` is a static class which does not need to be instantiated with `new`. Instead you can access the properties and methods on the global `FontLibrary` you import from the module and its contents will be shared across all canvases you create.

##### `.families`

The `.families` property contains a list of family names, merging together all the fonts installed on the system and any fonts that have been added manually through the `FontLibrary.use()` method. Any of these names can be passed to `FontLibrary.family()` for more information.

##### `family(name)`

If the `name` argument is the name of a known font family, this method will return an object with information about the available weights and styles. For instance, on my system `FontLibrary.family("Avenir Next")` returns:
```js
{
  family: 'Avenir Next',
  weights: [ 100, 400, 500, 600, 700, 800 ],
  widths: [ 'normal' ],
  styles: [ 'normal', 'italic' ]
}
```

Asking for details about an unknown family will return `undefined`.

##### `has(familyName)`

Returns `true` if the family is installed on the system or has been added via `FontLibrary.use()`.

##### `use(familyName, [...fontPaths])`

The `FontLibrary.use()` method allows you to dynamically load local font files and use them with your canvases. By default it will use whatever family name is in the font metadata, but this can be overridden by an alias you provide. Since font-wrangling can be messy, `use` can be called in a number of different ways:

###### with a list of file paths
```js
// with default family name
FontLibrary.use([
  "fonts/Oswald-Regular.ttf",
  "fonts/Oswald-SemiBold.ttf",
  "fonts/Oswald-Bold.ttf",
])

// with an alias
FontLibrary.use("Grizwald", [
  "fonts/Oswald-Regular.ttf",
  "fonts/Oswald-SemiBold.ttf",
  "fonts/Oswald-Bold.ttf",
])
```

###### with a list of ‘glob’ patterns

```js
// with default family name
FontLibrary.use(['fonts/Crimson_Pro/*.ttf'])

// with an alias
FontLibrary.use("Stinson", ['fonts/Crimson_Pro/*.ttf'])
```

###### multiple families with aliases
```js
FontLibrary.use({
  Nieuwveen: ['fonts/AmstelvarAlpha-VF.ttf', 'fonts/AmstelvarAlphaItalic-VF.ttf'],
  Fairway: 'fonts/Raleway/*.ttf'
})
```

The return value will be either a list or an object (matching the style in which it was called) with an entry describing each font file that was added. For instance, one of the entries from the first example could be:
```js
{
  family: 'Grizwald',
  weight: 600,
  style: 'normal',
  width: 'normal',
  file: 'fonts/Oswald-SemiBold.ttf'
}
```

## Acknowledgements

This project is deeply indebted to the work of the [Rust Skia project](https://github.com/rust-skia/rust-skia) whose Skia bindings provide a safe and idiomatic interface to the mess of C++ that lies underneath.

Many thanks to the [`node-canvas`](https://github.com/Automattic/node-canvas) developers for their terrific set of unit tests. In the absence of an [Acid Test](https://www.acidtests.org) for canvas, these routines were invaluable.

[SVG_path_commands]: https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/d#path_commands
[p2d_addPath]: https://developer.mozilla.org/en-US/docs/Web/API/Path2D/addPath
[p2d_closePath]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/closePath
[p2d_moveTo]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/moveTo
[p2d_lineTo]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/lineTo
[p2d_bezierCurveTo]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/bezierCurveTo
[p2d_quadraticCurveTo]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/quadraticCurveTo
[p2d_arc]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/arc
[p2d_arcTo]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/arcTo
[p2d_ellipse]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/ellipse
[p2d_rect]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/rect
[p2d_jitter]: #jittersegmentlength-amount-seed0
[p2d_round]: #roundradius
[p2d_trim]: #trimstart-end-inverted
[p2d_interpolate]: #interpolateotherpath-weight
[p2d_simplify]: #simplify
[p2d_points]: #pointsstep1
[p2d_contains]: #containsx-y
[p2d_offset]: #offsetdx-dy
[p2d_transform]: #transformmatrix-or-transforma-b-c-d-e-f
[bool-ops]: #complement-difference-intersect-union-and-xor

[drawText]: #filltextstr-x-y-width--stroketextstr-x-y-width
[conicCurveTo]: #coniccurvetocpx-cpy-x-y-weight
[outlineText()]: #outlinetextstr

[Buffer]: https://nodejs.org/api/buffer.html
[Canvas]: https://developer.mozilla.org/en-US/docs/Web/API/Canvas
[TextMetrics]: https://developer.mozilla.org/en-US/docs/Web/API/TextMetrics
[Promise]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Promise
[DataURL]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/Data_URIs
[VariableFonts]: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Fonts/Variable_Fonts_Guide

[CanvasGradient]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasGradient
[CanvasPattern]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasPattern
[CanvasRenderingContext2D]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D
[DOMMatrix]: https://developer.mozilla.org/en-US/docs/Web/API/DOMMatrix
[Image]: https://developer.mozilla.org/en-US/docs/Web/API/Image
[ImageData]: https://developer.mozilla.org/en-US/docs/Web/API/ImageData
[Path2D]: https://developer.mozilla.org/en-US/docs/Web/API/Path2D
[lineHeight]: https://developer.mozilla.org/en-US/docs/Web/CSS/line-height
[font-variant]: https://developer.mozilla.org/en-US/docs/Web/CSS/font-variant

[canvas_attr]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/canvas
[currentTransform]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/currentTransform
[direction]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/direction
[fillStyle]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/fillStyle
[filter]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/filter
[font]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/font
[globalAlpha]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/globalAlpha
[globalCompositeOperation]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/globalCompositeOperation
[imageSmoothingEnabled]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/imageSmoothingEnabled
[imageSmoothingQuality]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/imageSmoothingQuality
[lineCap]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/lineCap
[lineDashOffset]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/lineDashOffset
[lineJoin]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/lineJoin
[lineWidth]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/lineWidth
[miterLimit]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/miterLimit
[shadowBlur]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/shadowBlur
[shadowColor]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/shadowColor
[shadowOffsetX]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/shadowOffsetX
[shadowOffsetY]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/shadowOffsetY
[strokeStyle]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/strokeStyle
[textAlign]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/textAlign
[textBaseline]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/textBaseline
[arc()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/arc
[arcTo()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/arcTo
[beginPath()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/beginPath
[bezierCurveTo()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/bezierCurveTo
[clearRect()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/clearRect
[clip()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/clip
[closePath()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/closePath
[createConicGradient()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/createConicGradient
[createImageData()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/createImageData
[createLinearGradient()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/createLinearGradient
[createPattern()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/createPattern
[createRadialGradient()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/createRadialGradient
[drawFocusIfNeeded()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/drawFocusIfNeeded
[drawImage()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/drawImage
[ellipse()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/ellipse
[fill()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/fill
[fillRect()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/fillRect
[fillText()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/fillText
[getImageData()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/getImageData
[getLineDash()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/getLineDash
[getTransform()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/getTransform
[isPointInPath()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/isPointInPath
[isPointInStroke()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/isPointInStroke
[lineTo()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/lineTo
[measureText()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/measureText
[moveTo()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/moveTo
[putImageData()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/putImageData
[quadraticCurveTo()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/quadraticCurveTo
[rect()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/rect
[resetTransform()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/resetTransform
[restore()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/restore
[rotate()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/rotate
[save()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/save
[scale()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/scale
[setLineDash()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/setLineDash
[setTransform()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/setTransform
[stroke()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/stroke
[strokeRect()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/strokeRect
[strokeText()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/strokeText
[transform()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/transform
[translate()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/translate

[nonzero]: https://en.wikipedia.org/wiki/Nonzero-rule
[evenodd]: https://en.wikipedia.org/wiki/Even–odd_rule