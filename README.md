<a href="https://skia-canvas.org">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/hero-dark@2x.png">
  <img alt="Skia Canvas" src="docs/assets/hero@2x.png">
</picture>

</a>

---

<div align="center">
  <a href="http://skia-canvas.org/getting-started">Getting Started</a> <span>&nbsp;&nbsp;·&nbsp;&nbsp;</span>
  <a href="http://skia-canvas.org/api">Documentation</a> <span>&nbsp;&nbsp;·&nbsp;&nbsp;</span>
  <a href="http://skia-canvas.org/releases">Release Notes</a>  <span>&nbsp;&nbsp;·&nbsp;&nbsp;</span>
  <a href="https://github.com/samizdatco/skia-canvas/discussions">Discussion Forum</a>
</div>

---

Skia Canvas is a Node.js implementation of the HTML Canvas drawing [API](https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API) for both on- and off-screen rendering. Since it uses Google’s [Skia](https://skia.org) graphics engine, its output is very similar to Chrome’s [`<canvas>`](https://html.spec.whatwg.org/multipage/canvas.html) element — though it's also capable of things the brower’s Canvas still can't achieve.

In particular, Skia Canvas:

  - generates images in both raster (JPEG, PNG, & WEBP) and vector (PDF & SVG) formats
  - can draw to interactive GUI [windows][window] and provides a browser-like [event][win_bind] framework
  - can save images to [files][saveAs], encode to [dataURL][toDataURL_ext] strings, and return [Buffers][toBuffer] or [Sharp][sharp] objects
  - uses native threads in a [user-configurable][multithreading] worker pool for asynchronous rendering and file I/O
  - can create [multiple ‘pages’][newPage] on a given canvas and then [output][saveAs] them as a single, multi-page PDF or an image-sequence saved to multiple files
  - can [simplify][p2d_simplify], [blunt][p2d_round], [combine][bool-ops], [excerpt][p2d_trim], and [atomize][p2d_points] bézier paths using [efficient](https://www.youtube.com/watch?v=OmfliNQsk88) boolean operations or point-by-point [interpolation][p2d_interpolate]
  - provides [3D perspective][createProjection()] transformations in addition to [scaling][scale()], [rotation][rotate()], and [translation][translate()]
  - can fill shapes with vector-based [Textures][createTexture()] in addition to bitmap-based [Patterns][createPattern()] and supports line-drawing with custom [markers][lineDashMarker]
  - supports the full set of [CSS filter][filter] image processing operators
  - offers rich typographic control including:
    - multi-line, [word-wrapped][textwrap] text
    - line-by-line [text metrics][c2d_measuretext]
    - small-caps, ligatures, and other opentype features accessible using standard [font-variant][fontvariant] syntax
    - proportional [letter-spacing][letterSpacing], [word-spacing][wordSpacing], and [leading][c2d_font]
    - support for [variable fonts][VariableFonts] and transparent mapping of weight values
    - use of non-system fonts [loaded][fontlibrary-use] from local files

## Installation

If you’re running on a supported platform, installation should be as simple as:
```bash
npm install skia-canvas
```

This will download a pre-compiled library from the project’s most recent [release](https://github.com/samizdatco/skia-canvas/releases).


### `pnpm`
If you use the `pnpm` package manager, it will not download `skia-canvas`'s platform-native binary unless you explicitly allow it. You can do this interactively via the ‘approve builds’ command (note that you need to press `<space>` to toggle the selection and then `<enter>` to proceed):

```bash
pnpm install skia-canvas
pnpm approve-builds
```
In non-interactive scenarios (like building via CI), you can approve the build step when you add `skia-canvas` to your project:

```bash
pnpm install skia-canvas --allow-build=skia-canvas
```

Alternatively, you can add a [`pnpm.onlyBuiltDependencies`](https://pnpm.io/9.x/package_json#pnpmonlybuiltdependencies) entry to your `package.json` file to mark the build-step as allowed:
```json
{
  "pnpm": {
    "onlyBuiltDependencies": ["skia-canvas"]
  }
}
```



## Platform Support

The underlying Rust library uses [N-API][node_napi] v8 which allows it to run on Node.js versions:
  - v12.22+
  - v14.17+
  - v15.12+
  - v16.0.0 and later

Pre-compiled binaries are available for:

  - Linux — x64 & arm64
  - macOS — x64 & Apple silicon
  - Windows — x64 & arm64
  - AWS Lambda — x64 & arm64

### Linux / Docker

The library is compatible with Linux systems using [glibc](https://www.gnu.org/software/libc/) 2.28 or later as well as Alpine Linux and the [musl](https://musl.libc.org) C library it favors. In both cases, the [Fontconfig](https://www.freedesktop.org/wiki/Software/fontconfig/) library must be installed on the system for `skia-canvas` to operate correctly.

If you are setting up a [Dockerfile](https://nodejs.org/en/docs/guides/nodejs-docker-webapp/) that uses [`node`](https://hub.docker.com/_/node) as its basis, the simplest approach is to set your `FROM` image to one of the (Debian-derived) defaults like `node:lts`, `node:18`, `node:16`, `node:14-buster`, `node:12-buster`, `node:bullseye`, `node:buster`, or simply:
```dockerfile
FROM node
```

You can also use the ‘slim’ image if you manually install fontconfig:

```dockerfile
FROM node:slim
RUN apt-get update && apt-get install -y -q --no-install-recommends libfontconfig1
```

If you wish to use Alpine as the underlying distribution, you can start with something along the lines of:

```dockerfile
FROM node:alpine
RUN apk update && apk add fontconfig
```

### AWS Lambda

Skia Canvas depends on libraries that aren't present in the standard Lambda [runtime](https://docs.aws.amazon.com/lambda/latest/dg/lambda-runtimes.html). You can add these to your function by uploading a ‘[layer](https://docs.aws.amazon.com/lambda/latest/dg/chapter-layers.html)’ (a zip file containing the required libraries and `node_modules` directory) and configuring your function to use it.

1. Look in the **Assets** section of Skia Canvas’s [current release](https://github.com/samizdatco/skia-canvas/releases/latest) and download the `aws-lambda-x64.zip` or `aws-lambda-arm64.zip` file (depending on your architecture) but don’t decompress it
2. Go to the AWS Lambda [Layers console](https://console.aws.amazon.com/lambda/home/#/layers) and click the **Create Layer** button, then fill in the fields:
  - **Name**: `skia-canvas` (or whatever you want)
  - **Description**: you might want to note the Skia Canvas version here
  - **Compatible architectures**: select **x86_64** or **arm64** depending on which zip you chose
  - **Compatible runtimes**: select **Node.js 22.x** (and/or 20.x & 18.x)
3. Click the **Choose file** button and select the zip file you downloaded in Step 1, then click **Create**

You can now use this layer in any function you create in the [Functions console](https://console.aws.amazon.com/lambda/home/#/functions). After creating a new function, click the **Add a Layer** button and you can select your newly created Skia Canvas layer from the **Custom Layers** layer source.

### Next.js / Webpack

If you are using a framework like Next.js that bundles your serverside with Webpack, you'll need to mark `skia-canvas` as an ‘external’, otherwise its platform-native binary file will be excluded from the final build. Try adding these options to your `next.config.ts` file:

```js
const nextConfig: NextConfig = {
  serverExternalPackages: ['skia-canvas'],
  webpack: (config, options) => {
    if (options.isServer){
      config.externals = [
        ...config.externals,
        {'skia-canvas': 'commonjs skia-canvas'},
      ]
    }
    return config
  }
};
```


## Compiling from Source

If prebuilt binaries aren’t available for your system you’ll need to compile the portions of this library that directly interface with Skia.

Start by installing:

  1. The [Rust compiler](https://www.rust-lang.org/tools/install) and cargo package manager using [`rustup`](https://rust-lang.github.io/rustup/)
  2. A C compiler toolchain (either LLVM/Clang or MSVC)
  4. Python 3 (used by Skia's [build process](https://skia.org/docs/user/build/))
  3. The [Ninja](https://ninja-build.org) build system
  5. On Linux: Fontconfig and OpenSSL

[Detailed instructions](https://github.com/rust-skia/rust-skia#building) for setting up these dependencies on different operating systems can be found in the ‘Building’ section of the Rust Skia documentation. Once all the necessary compilers and libraries are present, running `npm run build` will give you a usable library (after a fairly lengthy compilation process).

## Global Settings

> There are a handful of settings that can only be configured at launch and will apply to all the canvases you create in your script. The sections below describe the different [environment variables][node_env] you can set to make global changes. You can either set them as part of your command line invocation, or place them in a `.env` file in your project directory and use Node 20's [`--env-file` argument][node_env_arg] to load them all at once.

### Multithreading

When rendering canvases in the background (e.g., by using the asynchronous [saveAs][saveAs] or [toBuffer][toBuffer] methods), tasks are spawned in a thread pool managed by the [rayon][rayon] library. By default it will create up to as many threads as your CPU has cores. You can see this default value by inspecting any [Canvas][canvas] object's [`engine.threads`][engine] property. If you wish to override this default, you can set the `SKIA_CANVAS_THREADS` environment variable to your preferred value.

For example, you can limit your asynchronous processing to two simultaneous tasks by running your script with:
```bash
SKIA_CANVAS_THREADS=2 node my-canvas-script.js
```

### Argument Validation

There are a number of situations where the browser API will react to invalid arguments by silently ignoring the method call rather than throwing an error. For example, these lines will simply have no effect:

```js
ctx.fillRect(0, 0, 100, "october")
ctx.lineTo(NaN, 0)
```


Skia Canvas does its best to emulate these quirks, but allows you to opt into a stricter mode in which it will throw TypeErrors in these situations (which can be useful for debugging).

Set the `SKIA_CANVAS_STRICT` environment variable to `1` or `true` to enable this mode.

## Example Usage

### Generating image files

```js
import {Canvas} from 'skia-canvas'

let canvas = new Canvas(400, 400),
    ctx = canvas.getContext("2d"),
    {width, height} = canvas;

let sweep = ctx.createConicGradient(Math.PI * 1.2, width/2, height/2)
sweep.addColorStop(0, "red")
sweep.addColorStop(0.25, "orange")
sweep.addColorStop(0.5, "yellow")
sweep.addColorStop(0.75, "green")
sweep.addColorStop(1, "red")
ctx.strokeStyle = sweep
ctx.lineWidth = 100
ctx.strokeRect(100,100, 200,200)

// render to multiple destinations using a background thread
async function render(){
  // save a ‘retina’ image...
  await canvas.saveAs("rainbox.png", {density:2})
  // ...or use a shorthand for canvas.toBuffer("png")
  let pngData = await canvas.png
  // ...or embed it in a string
  let pngEmbed = `<img src="${await canvas.toDataURL("png")}">`
}
render()

// ...or save the file synchronously from the main thread
canvas.saveAsSync("rainbox.pdf")
```

### Multi-page sequences

```js
import {Canvas} from 'skia-canvas'

let canvas = new Canvas(400, 400),
    ctx = canvas.getContext("2d"),
    {width, height} = canvas

for (const color of ['orange', 'yellow', 'green', 'skyblue', 'purple']){
  ctx = canvas.newPage()
  ctx.fillStyle = color
  ctx.fillRect(0,0, width, height)
  ctx.fillStyle = 'white'
  ctx.arc(width/2, height/2, 40, 0, 2 * Math.PI)
  ctx.fill()
}

async function render(){
  // save to a multi-page PDF file
  await canvas.saveAs("all-pages.pdf")

  // save to files named `page-01.png`, `page-02.png`, etc.
  await canvas.saveAs("page-{2}.png")
}
render()
```

### Rendering to a window

```js
import {Window} from 'skia-canvas'

let win = new Window(300, 300)
win.title = "Canvas Window"
win.on("draw", e => {
  let ctx = e.target.canvas.getContext("2d")
  ctx.lineWidth = 25 + 25 * Math.cos(e.frame / 10)
  ctx.beginPath()
  ctx.arc(150, 150, 50, 0, 2 * Math.PI)
  ctx.stroke()

  ctx.beginPath()
  ctx.arc(150, 150, 10, 0, 2 * Math.PI)
  ctx.stroke()
  ctx.fill()
})
```

### Integrating with [Sharp.js][sharp]

```js
import sharp from 'sharp'
import {Canvas, loadImage} from 'skia-canvas'

let canvas = new Canvas(400, 400),
    ctx = canvas.getContext("2d"),
    {width, height} = canvas,
    [x, y] = [width/2, height/2]

ctx.fillStyle = 'red'
ctx.fillRect(0, 0, x, y)
ctx.fillStyle = 'orange'
ctx.fillRect(x, y, x, y)

// Render the canvas to a Sharp object on a background thread then desaturate
await canvas.toSharp().modulate({saturation:.25}).jpeg().toFile("faded.jpg")

// Convert an ImageData to a Sharp object and save a grayscale version
let imgData = ctx.getImageData(0, 0, width, height, {matte:'white', density:2})
await imgData.toSharp().grayscale().png().toFile("black-and-white.png")

// Create an image using Sharp then draw it to the canvas as an Image object
let sharpImage = sharp({create:{ width:x, height:y, channels:4, background:"skyblue" }})
let canvasImage = await loadImage(sharpImage)
ctx.drawImage(canvasImage, x, 0)
await canvas.saveAs('mosaic.png')
```

## Acknowledgements

This project is deeply indebted to the work of the [Rust Skia project](https://github.com/rust-skia/rust-skia) whose Skia bindings provide a safe and idiomatic interface to the mess of C++ that lies underneath. Many thanks to the developers of [node-canvas](https://github.com/Automattic/node-canvas) for their terrific set of unit tests. In the absence of an [Acid Test](https://www.acidtests.org) for canvas, these routines were invaluable.


### Notable contributors

- [@mpaparno](https://github.com/mpaparno) contributed support for SVG rendering, raw image-buffer handling, WEBP import/export and numerous bugfixes
- [@Salmondx](https://github.com/Salmondx) developed the initial Raw image loading & rendering routines
- [@lucasmerlin](https://github.com/lucasmerlin) helped get GPU rendering working on Vulkan
- [@cprecioso](https://github.com/cprecioso) & [@saantonandre](https://github.com/saantonandre) corrected and expanded upon the TypeScript type definitions
- [@meihuanyu](https://github.com/meihuanyu) contributed filter & path rendering fixes

## Copyright
© 2020–2025 [Samizdat Drafting Co.](https://samizdat.co)

[bool-ops]: https://skia-canvas.org/api/path2d#complement-difference-intersect-union-and-xor
[c2d_font]: https://skia-canvas.org/api/context#font
[c2d_measuretext]: https://skia-canvas.org/api/context#measuretext
[canvas]: https://skia-canvas.org/api/canvas
[createProjection()]: https://skia-canvas.org/api/context#createprojection
[createTexture()]: https://skia-canvas.org/api/context#createtexture
[engine]: https://skia-canvas.org/api/canvas#engine
[fontlibrary-use]: https://skia-canvas.org/api/font-library#use
[fontvariant]: https://skia-canvas.org/api/context#fontvariant
[lineDashMarker]: https://skia-canvas.org/api/context#linedashmarker
[newPage]: https://skia-canvas.org/api/canvas#newpage
[p2d_interpolate]: https://skia-canvas.org/api/path2d#interpolate
[p2d_points]: https://skia-canvas.org/api/path2d#points
[p2d_round]: https://skia-canvas.org/api/path2d#round
[p2d_simplify]: https://skia-canvas.org/api/path2d#simplify
[p2d_trim]: https://skia-canvas.org/api/path2d#trim
[saveAs]: https://skia-canvas.org/api/canvas#saveas
[textwrap]: https://skia-canvas.org/api/context#textwrap
[toBuffer]: https://skia-canvas.org/api/canvas#tobuffer
[toDataURL_ext]: https://skia-canvas.org/api/canvas#todataurl
[win_bind]: https://skia-canvas.org/api/window#on--off--once
[window]: https://skia-canvas.org/api/window
[multithreading]: https://skia-canvas.org/getting-started#multithreading
[node_napi]: https://nodejs.org/api/n-api.html#node-api-version-matrix
[node_env]: https://nodejs.org/en/learn/command-line/how-to-read-environment-variables-from-nodejs
[node_env_arg]: https://nodejs.org/dist/latest-v22.x/docs/api/cli.html#--env-fileconfig
[rayon]: https://crates.io/crates/rayon
[sharp]: https://sharp.pixelplumbing.com
[VariableFonts]: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Fonts/Variable_Fonts_Guide
[filter]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/filter
[letterSpacing]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/letterSpacing
[wordSpacing]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/wordSpacing
[createPattern()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/createPattern
[rotate()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/rotate
[scale()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/scale
[translate()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/translate
