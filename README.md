# Skia Canvas

Skia Canvas is a browser-less implementation of the HTML Canvas drawing API for Node.js. It is based on Google’s [Skia](https://skia.org) graphics engine and as a result produces very similar results to Chrome’s `<canvas>` element.

While the primary goal of this project is to provide a reliable emulation of the [standard API](https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API) according to the [spec](https://html.spec.whatwg.org/multipage/canvas.html), it also extends it in a number of areas that are relevant to the generation of static graphics file rather that ‘live’ display in a browser.

In particular, Skia Canvas:

  - can generate output in both raster (JPEG & PNG) and vector (PDF & SVG) image formats
  - can create multiple ‘pages’ on a given canvas and then output them as a single, multi-page PDF or an image-sequence saved to multiple files
  - fully supports the new [CSS filter effects](https://drafts.fxtf.org/filter-effects/#FilterProperty) image processing operators
  - offers rich typographic control including:

    - multi-line, word-wrapped text
    - line-by-line text metrics
    - small-caps, ligatures, and other opentype features accessible using standard [font-variant][font-variant] syntax
    - proportional letter-spacing (a.k.a. ‘tracking’) and leading
    - support for [variable fonts][VariableFonts] and transparent mapping of weight values
    - use of non-system fonts loaded from local files


## Roadmap
This project is newly-hatched and still has some obvious gaps to fill (feel free to pitch in!).

On the agenda for subsequent updates are:
  - Prebuilt binaries (coming soon)
  - Use neon [Tasks](https://neon-bindings.com/docs/async) to provide asynchronous file i/o
  - Add SVG image loading using the [µsvg](https://crates.io/crates/usvg) parser
  - Add a `density` argument to Canvas and/or the output methods to allow for scaling to other device-pixel-ratios

## Installation

Until prebuilt binaries can be provided you’ll need to compile the portions of this library that directly interface with Skia. For this you’ll need to install:

  1. The [Rust compiler](https://www.rust-lang.org/tools/install) and cargo package manager using `rustup`
  2. Python 2.7 (Python 3 is not supported by [neon](https://neon-bindings.com/docs/getting-started#install-node-build-tools))
  3. The GNU `make` tool
  4. A C compiler toolchain like LLVM/Clang, GCC, or MSVC

Once these are all in place, installation *should* be as simple as:
```console
$ npm install skia-canvas
```

> Development of this library has taken place entirely on macOS, so reports from users of other platforms on the specifics of getting everything to compile properly would be appreciated.

## Module Contents

The library exports a number of classes emulating familiar browser objects including:

 - [Canvas][Canvas]
 - [CanvasGradient][CanvasGradient]
 - [CanvasPattern][CanvasPattern]
 - [CanvasRenderingContext2D][CanvasRenderingContext2D]
 - [DOMMatrix][DOMMatrix]
 - [Image][Image]
 - [ImageData][ImageData]
 - [Path2D][Path2D]

In addition, the module contains:

- [loadImage()](#loadimage) a utility function for loading `Image` objects asynchronously
- [FontLibrary](#fontlibrary) a class allowing you to inspect the system’s installed fonts and load additional ones




### Basic Usage
```js
const {Canvas, loadImage} = require('skia-canvas')

let canvas = new Canvas(512, 512),
    ctx = canvas.getContext("2d");

ctx.fillStyle = 'red'
ctx.fillRect(100,100, 200,200)
// ...
canvas.saveAs("foo.pdf")
```

## API Documentation

Most of your interaction with the canvas will actually be directed toward its ‘rendering context’, a supporting object you can acquire by calling the canvas’s [getContext()](https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/getContext) method. Documentation for each of the context’s attributes is linked below—properties are printed in italics and methods have parentheses attached to the name. The instances where Skia Canvas’s behavior goes beyond the standard are marked by a ⚡ symbol (see the next section for details).

| Canvas State                           | Drawing Primitives                          | Stroke & Fill                      | Effects                                                |
|----------------------------------------|---------------------------------------------|------------------------------------|--------------------------------------------------------|
| [*canvas*](#canvas) [⚡](#canvas)      | [clearRect()][clearRect()]                  | [*fillStyle*][fillStyle]           | [*filter*][filter]                                     |
| [beginPath()][beginPath()]             | [drawImage()][drawImage()]                  | [*lineCap*][lineCap]               | [*globalAlpha*][globalAlpha]                           |
| [clip()][clip()]                       | [fill()][fill()]                            | [*lineDashOffset*][lineDashOffset] | [*globalCompositeOperation*][globalCompositeOperation] |
| [isPointInPath()][isPointInPath()]     | [fillRect()][fillRect()]                    | [*lineJoin*][lineJoin]             | [*shadowBlur*][shadowBlur]                             |
| [isPointInStroke()][isPointInStroke()] | [fillText()][fillText()] [⚡][drawText]     | [*lineWidth*][lineWidth]           | [*shadowColor*][shadowColor]                           |
| [restore()][restore()]                 | [stroke()][stroke()]                        | [*miterLimit*][miterLimit]         | [*shadowOffsetX*][shadowOffsetX]                       |
| [save()][save()]                       | [strokeRect()][strokeRect()]                | [*strokeStyle*][strokeStyle]       | [*shadowOffsetY*][shadowOffsetY]                       |
|                                        | [strokeText()][strokeText()] [⚡][drawText] | [getLineDash()][getLineDash()]     |                                                        |
|                                        |                                             | [setLineDash()][setLineDash()]     |                                                        |


| Bezier Paths                             | Typography                                                  | Pattern & Image                                    | Transform                              |
|------------------------------------------|-------------------------------------------------------------|----------------------------------------------------|----------------------------------------|
| [arc()][arc()]                           | [*direction*][direction]                                    | [*imageSmoothingEnabled*][imageSmoothingEnabled]   | [*currentTransform*][currentTransform] |
| [arcTo()][arcTo()]                       | [*font*][font]                                              | [*imageSmoothingQuality*][imageSmoothingQuality]   | [getTransform()][getTransform()]       |
| [bezierCurveTo()][bezierCurveTo()]       | [*fontVariant* ⚡](#fontvariant)                            | [createImageData()][createImageData()]             | [resetTransform()][resetTransform()]   |
| [closePath()][closePath()]               | [*textAlign*][textAlign]                                    | [createLinearGradient()][createLinearGradient()]   | [rotate()][rotate()]                   |
| [ellipse()][ellipse()]                   | [*textBaseline*][textBaseline]                              | [createPattern()][createPattern()]                 | [scale()][scale()]                     |
| [lineTo()][lineTo()]                     | [*textTracking* ⚡](#texttracking)                          | [createRadialGradient()][createRadialGradient()]   | [setTransform()][setTransform()]       |
| [moveTo()][moveTo()]                     | [*textWrap* ⚡](#textwrap)                                  | [getImageData()][getImageData()]                   | [transform()][transform()]             |
| [quadraticCurveTo()][quadraticCurveTo()] | [measureText()][measureText()] [⚡](#measuretextstr-width)  | [putImageData()][putImageData()]                   | [translate()][translate()]             |
| [rect()][rect()]                         |                                                             |                                                    |                                        |




## Non-standard extensions

### Canvas

##### `.pages`

The canvas’s `.pages` attribute is an array of [`CanvasRenderingContext2D`][CanvasRenderingContext2D] objects corresponding to each ‘page’ that has been created. The first page is added when the canvas is initialized and additional ones can be added by calling the `newPage()` method. Note that all the pages remain drawable persistently, so you don’t have to constrain yourself to modifying the ‘current’ page as you render your document or image sequence.

##### `.pdf`, `.svg`, `.jpg`, and `.png`

These properties are syntactic sugar for calling the `toBuffer()` method. Each returns a Node [`Buffer`][Buffer] object with the contents of the canvas in the given format. If more than one page has been added to the canvas, only the most recent one will be included unless you’ve accessed the `.pdf` property in which case the buffer will contain a multi-page PDF.

##### `newPage(width, height)`

This method allows for the creation of additional drawing contexts that are fully independent of one another but will be part of the same output batch. It is primarily useful in the context of creating a multi-page PDF but can be used to create multi-file image-sequences in other formats as well. Creating a new page with a different size than the previous one will update the parent Canvas object’s `.width` and `.height` attributes but will not affect any other pages that have been created previously.

The method’s return value is a `CanvasRenderingContext2D` object which you can either save a reference to or recover later from the `.pages` array.

##### `saveAs(filename, {format, quality})`

The `saveAs` method takes a file path and writes the canvas’s current contents to disk. If the filename ends with an extension that makes its format clear, the second argument is optional. If the filename is ambiguous, you can pass an options object with a `format` string using names like `"png"` and `"jpeg"` or a full mime type like `"application/pdf"`.

The `quality` option is a number between 0 and 100 that controls the level of JPEG compression both when making JPEG files directly and when embedding them in a PDF. If omitted, quality will default to 100 (lossless).

The way multi-page documents are handled depends on the filename argument. If the filename contains the string `{}`, it will be used as template for generating a numbered sequence of files—one per page. If no curly braces are found in the filename, only a single file will be saved. That single file will be multi-page in the case of PDF output but for other formats it will contain only the most recently added page.

An integer can optionally be placed between the braces to indicate the number of padding characters to use for numbering. For instance `page-{}.png` will generate files of the form `page-1.svg` whereas `frame-{4}.png` will generate files like `frame-0001.png`.

##### `toBuffer(format, {quality, page})`

Node [`Buffer`][Buffer] objects containing various image formats can be created by passing either a format string like `"svg"` or a mime-type like `"image/svg+xml"`. The optional `quality` argument behaves the same as in the `saveAs` method.

The optional `page` argument accepts an integer that allows for the individual selection of pages in a multi-page canvas. Note that page indexing starts with page 1 **not** 0. The page value can also be negative, counting from the end of the canvas’s `.pages` array. For instance, `.toBuffer("png", {page:-1})` is equivalent to omitting `page` since they both yield the canvas’s most recently added page.

##### `toDataURL(format, {quality, page})`

This method accepts the same arguments and behaves similarly to `.toBuffer`. However instead of returning a Buffer, it returns a string of the form `"data:<mime-type>;base64,<image-data>"` which can be used as a `src` attribute in `<img>` tags, embedded into CSS, etc.

### CanvasRenderingContext2D

##### `.fontVariant`

The context’s [`.font`][font] property follows the CSS 2.1 standard and allows the selection of only a single font-variant type: `normal` vs `small-caps`. The full range of CSS 3 [font-variant][font-variant] values can be used if assigned to the context’s `.fontVariant` property (presuming the currently selected font supports them). Note that setting `.font` will also update the current `.fontVariant` value, so be sure to set the variant *after* selecting a typeface.

##### `.textTracking`

To loosen or tighten letter-spacing, set the `.textTracking` property to an integer representing the amount of space to add/remove in terms of 1/1000’s of an ‘em’ (a.k.a. the current font size). Positive numbers will space out the text (e.g., `100` is a good value for setting all-caps) while negative values will pull the letters closer together (this is only rarely a good idea).

The tracking value defaults to `0` and settings will persist across changes to the `.font` property.

##### `.textWrap`

The standard canvas has a rather impoverished typesetting system, allowing for only a single line of text and an approach to width-management that horizontally scales the letterforms (a type-crime if ever there was one). Skia Canvas allows you to opt-out of this single-line world by setting the `.textWrap` property to `true`. Doing so affects the behavior of the `drawText()` and `measureText()` methods as described below.

##### `fillText(str, x, y, [width])` & `strokeText(str, x, y, [width])`

The text-drawing methods’ behavior is mostly standard unless `.textWrap` has been set to `true`, in which case there are 3 main effects:

  1. Manual line breaking via `"\n"` escapes will be honored rather than converted to spaces
  2. The optional `width` argument accepted by `drawText` and `measureText` will be interpreted as a ‘column width’ and used to word-wrap long lines
  3. The line-height setting in the `.font` value will be used to set the inter-line leading rather than simply being ignored.

Even when `.textWrap` is `false`, the text-drawing methods will never choose a more-condensed weight or otherwise attempt to squeeze your entire string into the measure specified by `width`. Instead the text will be typeset up through the last word that fits and the rest will be omitted. This can be used in conjunction with the `.lines` property of the object returned by `measureText()` to incrementally lay out a long string into, for example, a multi-column layout with an even number of lines in each.

##### `measureText(str, [width])`

The `measureText()` method returns a [TextMetrics][TextMetrics] object describing the dimensions of a run of text *without* actually drawing it to the canvas. Skia Canvas adds an additional property to the metrics object called `.lines` which contains an array describing the geometry of each line individually.

Each element of the array contains an object of the form:
```
{x, y, width, height, baseline, startIndex, endIndex}
```
The `x`, `y`, `width`, and `height` values define a rectangle that fully encloses the text of a given line relative to the ‘origin’ point you would pass to `fillText()` or `strokeText()` (and reflecting the context’s current `.textBaseline` setting).

The `baseline` value is a y-axis offset from the text origin to that particular line’s baseline.

The `startIndex` and `endIndex` values are the indices into the string of the first and last character that were typeset on that line.


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

// with default family name
FontLibrary.use("Stinson", ['fonts/Crimson_Pro/*.ttf'])
```

###### multiple families with aliases
```js
FontLibrary.use({
  Nieuwveen: 'fonts/AmstelvarAlpha-VF.ttf',
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

[drawText]: #filltextstr-x-y-width--stroketextstr-x-y-width

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

[canvas]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/canvas
[currentTransform]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/currentTransform
[direction]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/direction
[fillStyle]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/fillStyle
[filter]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/filter
[font]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/font
[font-variant]: https://developer.mozilla.org/en-US/docs/Web/CSS/font-CanvasRenderingContext2D/variant
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