---
description: An emulation of the HTML <canvas> element
---
# Canvas

> The Canvas object is a stand-in for the HTML `<canvas>` element. It defines image dimensions and provides a [rendering context][context] to draw to it. Once you’re ready to save or display what you’ve drawn, the canvas can [save][saveAs] it to a file, or hand it off to you as a [data buffer][toBuffer] or [string][toDataURL_ext] to process manually.


| Rendering Contexts            | Output                                                              | Image Dimensions               |
| --                            | --                                                                  | --                             |
| [**gpu**][canvas_gpu] 🧪      | [**pdf**, **png**, **svg**, **jpg**, **webp**][shorthands] 🧪       | [**width**][canvas_width]      |
| [**engine**][engine] 🧪       | [saveAs()][saveAs] / [saveAsSync()][saveAs] 🧪                      | [**height**][canvas_height]    |
| [**pages**][canvas_pages] 🧪  | [toBuffer()][toBuffer] / [toBufferSync()][toBuffer] 🧪              |                                |
| [getContext()][getContext]    | [toDataURL()][toDataURL_ext] / [toDataURLSync()][toDataURL_ext] 🧪  |                                |
| [newPage()][newPage] 🧪       |                                                                     |                                |

## Creating new `Canvas` objects

Rather than calling a DOM method to create a new canvas, you can simply call the `Canvas` constructor with the width and height (in pixels) of the image you’d like to begin drawing.

```js
let defaultCanvas = new Canvas() // without arguments, defaults to 300 × 150 px
let squareCanvas = new Canvas(512, 512) // creates a 512 px square
```

## Saving graphics to files, buffers, and strings

When the canvas renders images and writes them to disk, it does so in a background thread so as not to block execution within your script. As a result you’ll generally want to deal with the canvas from within an `async` function and be sure to use the `await` keyword when accessing any of its output methods or shorthand properties (all of which return Promises):
  - [`saveAs()`][saveAs]
  - [`toBuffer()`][toBuffer]
  - [`toDataURL()`][toDataURL_ext]
  - [`.pdf`, `.svg`, `.jpg`, `.webp`, and `.png`][shorthands]


In cases where this is not the desired behavior, you can use the synchronous equivalents for the primary export functions. They accept identical arguments to their async versions but block execution and return their values synchronously rather than wrapped in Promises. Also note that the [shorthand properties][shorthands] do not have synchronous versions:
- [`saveAsSync()`][saveAs]
- [`toBufferSync()`][toBuffer]
- [`toDataURLSync()`][toDataURL_ext]

For instance, both of the example functions below will generate PNG & PDF from the canvas, though the first will be more efficient (particularly for parallel contexts like request-handlers in an HTTP server or batch exports):

```js
let canvas = new Canvas()

async function normal(){
  let pngURL = await canvas.toDataURL("png")
  let pdfBuffer = await canvas.pdf
}

function synchronous(){
  let pngURL = canvas.toDataURLSync("png")
  let pdfBuffer = canvas.toBufferSync("pdf")
}
```

--------

## Properties

### `.gpu`

The `.gpu` attribute allows you to control whether rendering occurs on the graphics card or uses the CPU. Rendering is hardware accelerated by default, using [Metal](https://developer.apple.com/metal/) on macOS and [Vulkan](https://www.vulkan.org) on Linux and Windows. To use software-based rendering, set the `.gpu` property to `false`. If the current platform doesn't support GPU-based rendering, the property will be `false` by default (see [this article](https://linuxconfig.org/install-and-test-vulkan-on-linux) for some tips on getting Vulkan working on Linux).

### `.engine`

The `.engine` property is a read-only object that provides you with a status report on how this Canvas's images will be rendered. It contains the following fields:
  - `renderer`: describes whether the `CPU` or `GPU` is currently being used to generate output images. If GPU initialization failed, the renderer will report being the `CPU`, even if you set the Canvas's [`.gpu`][canvas_gpu] property to `true`
  - `api`: either `Metal` or `Vulkan` depending on your platform
  - `device`: the identity of the ‘video card’ that was found during start-up
  - `driver`: the name of the OS's device driver *‹vulkan-only›*
  - `threads`: the number of threads in the worker pool that will be used for asynchronous [`saveAs`][saveAs] & [`toBuffer`][toBuffer] exports. By default this is the same as the number of CPU cores found, but can be overridden by setting the [`SKIA_CANVAS_THREADS`][multithreading] environment variable.
  - `error`: if GPU initialization failed, this property will contain a description of what went wrong. Otherwise it will be undefined.

### `.pages`

The canvas’s `.pages` attribute is an array of [`CanvasRenderingContext2D`][CanvasRenderingContext2D] objects corresponding to each ‘page’ that has been created. The first page is added when the canvas is initialized and additional ones can be added by calling the `newPage()` method. Note that all the pages remain drawable persistently, so you don’t have to constrain yourself to modifying the ‘current’ page as you render your document or image sequence.

### `.pdf`, `.png`, `.svg`, `.jpg`, & `.webp`

These properties are syntactic sugar for calling the `toBuffer()` method. Each returns a [Promise][Promise] that resolves to a Node [`Buffer`][Buffer] object with the contents of the canvas in the given format. If more than one page has been added to the canvas, only the most recent one will be included unless you’ve accessed the `.pdf` property in which case the buffer will contain a multi-page PDF.

--------

## Methods

### `newPage()`
```js returns="CanvasRenderingContext2D"
newPage(width, height)
```

This method allows for the creation of additional drawing contexts that are fully independent of one another but will be part of the same output batch. It is primarily useful in the context of creating a multi-page PDF but can be used to create multi-file image-sequences in other formats as well. Creating a new page with a different size than the previous one will update the parent Canvas object’s `.width` and `.height` attributes but will not affect any other pages that have been created previously.

The method’s return value is a `CanvasRenderingContext2D` object which you can either save a reference to or recover later from the `.pages` array.

### `saveAs()`
```js returns="Promise<void>"
saveAs(filename, {page, format, matte, density=1, msaa=4, quality=0.92, outline=false, colorType='rgba'})
```
```js returns="void"
saveAsSync(filename, {page, format, matte, density=1, msaa=4, quality=0.92, outline=false, colorType='rgba'})
```

The `saveAs` method takes a file path and writes the canvas’s current contents to disk. If the filename ends with an extension that makes its format clear, the second argument is optional. If the filename is ambiguous, you can pass an options object with a `format` string using names like `"png"` and `"jpeg"` or a full mime type like `"application/pdf"`.

The way multi-page documents are handled depends on the `filename` argument. If the filename contains the string `"{}"`, it will be used as template for generating a numbered sequence of files—one per page. If no curly braces are found in the filename, only a single file will be saved. That single file will be multi-page in the case of PDF output but for other formats it will contain only the most recently added page.

An integer can optionally be placed between the braces to indicate the number of padding characters to use for numbering. For instance `"page-{}.svg"` will generate files of the form `page-1.svg` whereas `"frame-{4}.png"` will generate files like `frame-0001.png`.

#### page
The optional `page` argument accepts an integer that allows for the individual selection of pages in a multi-page canvas. Note that page indexing starts with page 1 **not** 0. The page value can also be negative, counting from the end of the canvas’s `.pages` array. For instance, `.saveAs("currentPage.png", {page:-1})` is equivalent to omitting `page` since they both yield the canvas’s most recently added page.

#### format

The image format to generate, specified either as a mime-type string or file extension. The `format` argument will take precedence over the type specified through the `filename` argument’s extension, but is primarily useful when generating a file whose name cannot end with an extension for other reasons.

Supported formats include:
- Bitmap: `png`, `jpeg`, `webp`, `raw`
- Vector: `svg`, `pdf`

#### matte
The optional `matte` argument accepts a color-string specifying the background that should be drawn *behind* the canvas in the exported image. Any transparent portions of the image will be filled with the matte color.

#### density
By default, the images will be at a 1:1 ratio with the canvas's `width` and `height` dimensions (i.e., a 72 × 72 canvas will yield a 72 pixel × 72 pixel bitmap). But with screens increasingly operating at higher densities, you’ll frequently want to generate images where an on-canvas 'point' may occupy multiple pixels. The optional `density` argument allows you to specify this magnification factor using an integer ≥1. As a shorthand, you can also select a density by choosing a filename using the `@nx` naming convention:

```js
canvas.saveAs('image.png', {density:2}) // choose the density explicitly
canvas.saveAs('image@3x.png') // equivalent to setting the density to 3
```

#### msaa
The `msaa` argument allows you to control the number of samples used for each pixel by the GPU's multi-scale antialiasing (common values are `2`, `4`, & `8`, corresponding to 2𝗑, 4𝗑, or 8𝗑 sampling). Higher values will produce smoother-looking images but also increase resource usage. Setting the value to `false` will disable MSAA and use (slower but potentially higher-quality) shader-based AA routines instead. If omitted, the renderer defaults to 4x MSAA as it produces good results with relatively low overhead.

#### quality
The `quality` option is a number between 0 and 1.0 that controls the level of JPEG compression both when making JPEG files directly and when embedding them in a PDF. If omitted, quality will default to 0.92.

#### outline
When generating SVG output containing text, you have two options for how to handle the fonts that were used. By default, SVG files will contain `<text>` elements that refer to the fonts by name in the embedded stylesheet. This requires that viewers of the SVG have the same fonts available on their system (or accessible as webfonts). Setting the optional `outline` argument to `true` will trace all the letterforms and ‘burn’ them into the file as bézier paths. This will result in a much larger file (and one in which the original text strings will be unrecoverable), but it will be viewable regardless of the specifics of the system it’s displayed on.

#### colorType

Specifies the color type to use when exporting pixel data in `"raw"` format (for other formats this setting has no effect). If omitted, defaults to `"rgba"`. See the ImageData documentation for a [list of supported `colorType` formats][imgdata_colortype]


### `toBuffer()`
```js returns="Promise<Buffer>"
toBuffer(format, {page, format, matte, density, msaa, quality, outline, colorType})
```
```js returns="Buffer"
toBufferSync(format, {page, format, matte, density, msaa, quality, outline, colorType})
```

Node [`Buffer`][Buffer] objects containing various image formats can be created by passing either a format string like `"svg"` or a mime-type like `"image/svg+xml"`. An ‘@’ suffix can be added to the format string to specify a pixel-density (for instance, `"jpg@2x"`). The optional arguments behave the same as in the [`saveAs`][saveAs] method.

### `toDataURL()`
```js returns="Promise<String>"
toDataURL(format, {page, format, matte, density, msaa, quality, outline, colorType})
```
```js returns="String"
toDataURLSync(format, {page, format, matte, density, msaa, quality, outline, colorType})
```

This method accepts the same arguments and behaves similarly to [.toBuffer()][toBuffer]. However instead of returning a Buffer, it returns a string of the form `"data:<mime-type>;base64,<image-data>"` which can be used as a `src` attribute in `<img>` tags, embedded into CSS, etc.

<!-- references_begin -->
[canvas_gpu]: #gpu
[canvas_pages]: #pages
[context]: context.md
[engine]: #engine
[newPage]: #newpage
[imgdata_colortype]: imagedata.md#colortype
[saveAs]: #saveas
[shorthands]: #pdf-png-svg-jpg--webp
[toBuffer]: #tobuffer
[toDataURL_ext]: #todataurl
[multithreading]: ../getting-started.md#multithreading
[Buffer]: https://nodejs.org/api/buffer.html
[canvas_width]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/width
[canvas_height]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/height
[getContext]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/getContext
[Promise]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Promise
[CanvasRenderingContext2D]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D
<!-- references_end -->
