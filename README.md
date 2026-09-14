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

<div align="center">

<!--### [Version 3.0 now available](https://github.com/samizdatco/skia-canvas/discussions/255)-->

</div>

---

Skia Canvas is an implementation of the [HTML Canvas](https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API) drawing API that runs in [Node.js](https://nodejs.org/en) on Mac, Linux, and Windows systems. Depending on your needs, you can use it as:
  1. **A spec-compliant offscreen canvas:** it accepts the same drawing code you'd write for a browser but can run on servers and in other ‘headless’ contexts to generate image files and buffers.
  2. **A windowing toolkit:** it can open native [windows][window] on macOS, Windows, and Linux with [display-synced][win_animation] drawing and browser-inspired [event handling][win_events].
  3. **A JavaScript interface for the [Skia](https://skia.org) graphics library:** it uses familiar web APIs as a front-end to Google’s sophisticated imaging engine, rendering with high-performance native code (and optional GPU acceleration).



### A more capable canvas

In addition to being a faithful emulation of the [canvas standard](https://html.spec.whatwg.org/multipage/canvas.html), Skia Canvas includes a raft of extensions adding 2D capabilities that reach well beyond what the browser’s `<canvas>` can do.

In particular, Skia Canvas can:

  - generate images in vector (PDF & SVG) as well as bitmap (JPEG, PNG, & WEBP) formats
  - save images to [files][toFile], encode to [dataURL][toURL] strings, and return [Buffers][toBuffer] or [Sharp][sharp] objects
  - create [multiple ‘pages’][newPage] on a given canvas and [output][toFile] them as a multi-page PDF or an image-sequence saved to multiple files
  - load [PDFs & SVGs][loadimage] as scalable vector images or open a [multi-page PDF][loadcanvas] as an editable canvas
  - render in wide-gamut Display P3 color with [CSS Color 4][ctx_colors] syntax support
  - [slice][p2d_slice] & [sample][p2d_points] Path2D objects, combine them with [boolean operators][bool-ops], and decompose them into [contours][p2d_contours], [verbs][edges], or [points][p2d_positionAt]
  - transform coordinates using [3D perspective][createProjection()] in addition to [scaling][scale()], [rotation][rotate()], and [translation][translate()]
  - fill paths with vector-based [Textures][createTexture()] or bitmap [Patterns][createPattern()] and draw strokes with custom [markers][lineDashMarker]
  - apply the full set of [CSS filter][filter] image processing operators
  - provide rich typographic control including:
    - multi-line, [word-wrapped][textwrap] text
    - line-by-line [text metrics][c2d_measuretext]
    - small-caps, ligatures, and other [opentype features][fontfeatures] accessible using standard [font-variant][fontvariant] syntax
    - proportional [letter-spacing][letterSpacing], [word-spacing][wordSpacing], and [leading][c2d_font]
    - support for [variable fonts][VariableFonts] and automatic use of weight, width, and optical-sizing axes
    - use of non-system fonts [loaded][fontlibrary-use] from local files
  - use native threads in a [user-configurable][multithreading] worker pool for asynchronous rendering and file I/O
  - render images server-side on standard Linux hosts and ‘serverless’ platforms like Vercel, Cloudflare Containers, and AWS Lambda

## Installation

If you’re running on a supported platform, installation should be as simple as:
```bash
npm install skia-canvas
```

The precompiled binary for your platform will be downloaded automatically as a `@skia-canvas/*` optional dependency, so installation should succeed even if `--ignore-scripts` is enabled or you are using a proxied npm registry mirror. On the other hand, installation will now fail if you have disabled optional dependencies via `--no-optional`, `NPM_CONFIG_OMIT`, etc.

Because optional dependencies are platform-specific, your `node_modules` directory only includes the binary for the machine you originally ran `install` on. Syncing that directory to a server that runs a different OS or architecture may result in a ‘native binary missing’ error when you attempt to run your script remotely.

Working around this requires different approaches based on your package manager of choice:

### `npm`

The default `npm` package manager installs whichever optional depenency matches the *current* machine's OS & architecture by default. But you can also create a `node_modules` folder that's directly shippable to your target platform by specifying its configuration explicitly:

```bash
npm ci --os=linux --cpu=x64 --libc=glibc # (or --libc=musl)
```

> Note that the `--libc` flag is only supported on npm 10.4+, so you may need to upgrade `npm` first if you want to select a `musl` target.

### `pnpm` or `yarn`

You can add a `supportedArchitectures` snippet to your `pnpm-workspace.yaml` or `.yarnrc.yml` configuration that supports both your development machine (`current`) and the system you plan to deploy to (e.g., x64 Linux for both glibc and musl-based distributions):

```yml
supportedArchitectures:
  os: [current, linux]
  cpu: [current, x64]
  libc: [current, glibc, musl]
```

Once this is in place, you can run `pnpm install` or `yarn install` and the generated lockfile will contain all the specified optional dependencies. If you're adding this to an existing project, you may want to delete the existing lockfile before running `install` to ensure it picks up the change.

## Platform Support

Skia Canvas runs on Linux, macOS, or Windows as well as serverless platforms like Vercel, Cloudflare Containers, and AWS Lambda. Precompiled versions of the library’s native code will be automatically downloaded in the appropriate architecture (`arm64` or `x64`) when you install it via npm.

The underlying Rust library uses [N-API][node_napi] v8 which allows it to run on all [currently supported](https://nodejs.org/en/about/previous-releases) Node.js releases, and it is backward compatible with versions going back to Node 18.0.

### Linux

The library is compatible with Linux systems using [glibc](https://www.gnu.org/software/libc/) 2.28 or later as well as Alpine Linux and the [musl](https://musl.libc.org) C library it favors. It will make use of the system’s `fontconfig` settings in `/etc/fonts` if they exist but will otherwise fall back to using a [placeholder configuration](https://github.com/samizdatco/skia-canvas/blob/main/lib/fonts/fonts.conf), looking for installed fonts at commonly used Linux paths.

### Docker

If you are setting up a [Dockerfile](https://nodejs.org/en/docs/guides/nodejs-docker-webapp/) that uses [`node`](https://hub.docker.com/_/node) as its basis, the simplest approach is to set your `FROM` image to one of the (Debian-derived) defaults like `node:lts`, `node:22`, `node:24-bookworm`, or simply:
```dockerfile
FROM node
```

If you wish to use Alpine as the underlying distribution, you can start with something along the lines of:

```dockerfile
FROM node:alpine
```

Whichever distribution you choose, you'll probably want to bundle the `node_modules` folder into the container image itself so you can ensure it includes the correct optional dependencies for the target architecture:

```dockerfile
FROM node:lts-slim

WORKDIR /app
COPY package*.json ./
RUN npm ci --omit=dev
COPY . .

USER node
CMD ["node", "your-script-name.js"]
```

You'll also want to make sure that your project's `.dockerignore` contains `node_modules`, so your local copy doesn't partially overwrite and corrupt the version that's created by `npm ci`.

### Cloudflare

To use Skia Canvas as part of a Cloudflare Worker process, you must first create a docker container with its dependencies and [deploy](https://developers.cloudflare.com/containers/guides/deploy/) it to [Cloudflare Containers](https://developers.cloudflare.com/containers/). Start with a Dockerfile like those described in the previous section, but ensure that it's building for the `linux/amd64` configuration that Cloudflare supports:

```dockerfile
FROM node --platform=linux/amd64
```

### AWS Lambda

Skia Canvas depends on libraries that aren't present in the standard Lambda [runtime](https://docs.aws.amazon.com/lambda/latest/dg/lambda-runtimes.html). You can add these to your function by uploading a ‘[layer](https://docs.aws.amazon.com/lambda/latest/dg/chapter-layers.html)’ (a zip file containing the required libraries and `node_modules` directory) and configuring your function to use it.

<details>

<summary><b>Detailed AWS instructions</b> (click to expand)</summary>

#### Adding the Skia Canvas layer to your AWS account

1. Look in the **Assets** section of Skia Canvas’s [current release](https://github.com/samizdatco/skia-canvas/releases/latest) and download the `aws-lambda-x64.zip` or `aws-lambda-arm64.zip` file (depending on your architecture) but don’t decompress it
2. Go to the AWS Lambda [Layers console](https://console.aws.amazon.com/lambda/home/#/layers) and click the **Create Layer** button, then fill in the fields:
  - **Name**: `skia-canvas` (or whatever you want)
  - **Description**: you might want to note the Skia Canvas version here
  - **Compatible architectures**: select **x86_64** or **arm64** depending on which zip you chose
  - **Compatible runtimes**: select **Node.js 22.x** (and/or 20.x)
3. Click the **Choose file** button and select the zip file you downloaded in Step 1, then click **Create**

Alternatively, you can use the [`aws` command line tool](https://github.com/aws/aws-cli) to create the layer. This bash script will fetch the skia-canvas version of your choice and make it available to your Lambda functions.
```sh
#!/usr/bin/env bash
VERSION=4.0.0 # the skia-canvas version to include
PLATFORM=arm64 # arm64 or x64

curl -sLO https://github.com/samizdatco/skia-canvas/releases/download/v${VERSION}/aws-lambda-${PLATFORM}.zip
aws lambda publish-layer-version \
    --layer-name "skia-canvas" \
    --description "Skia Canvas ${VERSION} layer" \
    --zip-file "fileb://aws-lambda-${PLATFORM}.zip" \
    --compatible-runtimes "nodejs22.x" "nodejs24.x" \
    --compatible-architectures "${X/#x/x86_}"
```

#### Using the layer in a Lambda function

You can now use this layer in any function you create in the [Functions console](https://console.aws.amazon.com/lambda/home/#/functions). After creating a new function, click the **Add a Layer** button and you can select your newly created Skia Canvas layer from the **Custom Layers** layer source.

Note that the layer only includes Skia Canvas and its dependencies—any other npm modules you want to use will need to be bundled into your function. To prevent the `skia-canvas` module from being doubly-included, make sure you add it to the  `devDependencies` section (**not** the regular `dependencies` section) of your package.json file.

</details>


### Next.js / Webpack

If you are using a framework like Next.js that bundles your server-side code with Webpack, you'll need to mark `skia-canvas` as an ‘external’, otherwise its platform-native binary file will be excluded from the final build. Try adding these options to your `next.config.ts` file:

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

  1. A recent version of `git` (older versions have difficulties with Skia's submodules)
  2. The [Rust compiler](https://www.rust-lang.org/tools/install) and cargo package manager using [`rustup`](https://rust-lang.github.io/rustup/)
  3. A C compiler toolchain (either LLVM/Clang or MSVC)
  4. Python 3 (used by Skia's [build process](https://skia.org/docs/user/build/))
  5. The [Ninja](https://ninja-build.org) build system
  6. On Linux: Fontconfig and OpenSSL

[Detailed instructions](https://github.com/rust-skia/rust-skia#building) for setting up these dependencies on different operating systems can be found in the ‘Building’ section of the Rust Skia documentation. The Dockerfiles in the [containers](https://github.com/samizdatco/skia-canvas/tree/main/containers) directory may also be useful for identifying needed dependencies. Once all the necessary compilers and libraries are present, running `npm install` followed by `npm run build` will give you a usable library (after a fairly lengthy compilation process).

## Global Settings

> There are a handful of settings that can only be configured at launch and will apply to all the canvases you create in your script. The sections below describe the different [environment variables][node_env] you can set to make global changes. You can either set them as part of your command line invocation, or place them in a `.env` file in your project directory and use Node 20's [`--env-file` argument][node_env_arg] to load them all at once.

### Multithreading

When rendering canvases in the background (e.g., by using the asynchronous [toFile][toFile] or [toBuffer][toBuffer] methods), tasks are spawned in a thread pool managed by the [rayon][rayon] library. By default it will create up to as many threads as your CPU has cores. You can see this default value by inspecting any [Canvas][canvas] object's [`engine.threads`][engine] property. If you wish to override this default, you can set the `SKIA_CANVAS_THREADS` environment variable to your preferred value.

For example, you can limit your asynchronous processing to two simultaneous tasks by running your script with:
```bash
SKIA_CANVAS_THREADS=2 node my-canvas-script.js
```

### Render Cache

Skia Canvas defers rendering until the canvas is exported in order allow a single canvas to be rendered as both a vector and a bitmap. As a result, it needs to re-execute all the canvas's drawing commands every time a bitmap is requested—even if nothing has changed since it was last rasterized. To avoid this wasted work, rendered bitmaps are cached internally and reused if possible, improving execution speed at the cost of some memory.

By default, the cache is set to a maximum of **128MB** (enough to contain ~32 rasters at 720p resolution), but you can adjust this to fit your use case via the `SKIA_CANVAS_CACHE` environment variable. Caching can be disabled altogether by setting it to `0` or `off`, and a different maximum size can be set by passing a number (representing a number of megabytes):

```bash
SKIA_CANVAS_CACHE=0 node script.js   # `0`/`off` disable the render cache altogether
SKIA_CANVAS_CACHE=256 node script.js # set the maximum size to double the default
```

### Memory Fragmentation
> Note: this only applies to Linux systems using the `glibc` C Library

The memory allocator used by `glibc` does not return memory to the kernel the moment it's freed. Instead it maintains its own internal cache of reusable memory regions and only releases memory if a *contiguous* empty region sits at the very end of its arena. As a result, this can allow RSS to balloon if empty blocks of memory are punctuated by even a single small allocation.

Since that kind of fragmentation occurs quite frequently with canvas workflows (e.g., large export buffers interleaved with small Color and Path2D allocations), Skia Canvas calls `malloc_trim` intermittently to release unoccupied memory chunks even if they're not at the end of the arena. There is a marginal performance cost in exchange for the lower memory ceiling this maintains so you can tune the behavior to be more or less aggressive via the `SKIA_CANVAS_TRIM` environment variable:

```bash
SKIA_CANVAS_TRIM=0 node script.js     # `0`/`off` disable the `malloc_trim` calls altogether
SKIA_CANVAS_TRIM=eager node script.js # `eager` lowers the threshold and runs more frequently
```

### Argument Validation

There are a number of situations where the browser API will react to invalid arguments by silently ignoring the method call rather than throwing an error. For example, these lines will simply have no effect:

```js
ctx.fillRect(0, 0, 100, "october")
ctx.lineTo(NaN, 0)
```

Skia Canvas does its best to emulate these quirks, but allows you to opt into a stricter mode in which it will throw TypeErrors in these situations (which can be useful for debugging). Set `SKIA_CANVAS_STRICT` to `1` or `true` to enable strict mode:

```bash
SKIA_CANVAS_STRICT=1 node script.js
```

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
  await canvas.toFile("rainbox.png", {density:2})
  // ...or use a shorthand for canvas.toBuffer("png")
  let pngData = await canvas.png
  // ...or embed it in a string
  let pngEmbed = `<img src="${await canvas.toURL("png")}">`
}
render()

// ...or save the file synchronously from the main thread
canvas.toFileSync("rainbox.pdf")
```

### Multi-page sequences

```js
import {Canvas, loadCanvas} from 'skia-canvas'

let canvas = new Canvas(400, 400),
    ctx = canvas.getContext("2d"), // first page will be blank
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
  // save to files named `page-01.png`, `page-02.png`, etc.
  await canvas.toFile("page-{2}.png")

  // save to a multi-page PDF file
  await canvas.toFile("all-pages.pdf")

  // the multi-page PDF can be read back in and even drawn upon
  let multipage = await loadCanvas("all-pages.pdf")
  for (let [i, pg] of multipage.pages.entries()){
    pg.font = 'italic 12px serif'
    pg.textAlign = 'center'
    pg.textBaseline = 'middle'
    pg.fillText(`p. ${i+1}`, multipage.width/2, multipage.height/2)
  }
  await multipage.toFile("all-pages-labeled.pdf")
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

### Wide-gamut colors

```js
import {Canvas} from 'skia-canvas'

let pad = 16, size = 64, width = 4*size + 3*pad,
    canvas = new Canvas(336, 240),
    ctx = canvas.getContext("2d", {colorSpace:"display-p3"})

// CSS Color 4 syntax is supported everywhere (and colors can exceed the sRGB gamut)
for (let color of [
  "color(display-p3 1 0 0)", "oklch(65% 0.27 145)", "oklab(70% -0.11 0.16)", "lch(60% 76 300)"
]){
  ctx.fillStyle = color
  ctx.fillRect(pad, pad, size, size)
  ctx.translate(size + pad, 0)
}
ctx.translate(-width, pad + size)

// gradients can select the color space used for interpolation
for (let {space, from, to, hue} of [
  {space:"srgb",  from:"navy", to:"gold"}, // the sRGB default get washed out midway through
  {space:"oklab", from:"navy", to:"gold"}, // Oklab stays saturated and perceptually uniform
  {space:"oklch", from:"red", to:"red", hue:"longer"}, // full 360° from a single hue
]){
  let ramp = ctx.createLinearGradient(0, pad, width, pad)
  if (hue) ramp.hueInterpolationMethod = hue // only applies to angle-based spaces
  ramp.colorInterpolationMethod = space
  ramp.addColorStop(0, from)
  ramp.addColorStop(1, to)
  ctx.fillStyle = ramp
  ctx.fillRect(0, pad, width, size/2)
  ctx.translate(0, pad + size/2)
}
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
await canvas.toFile('mosaic.png')
```

## Benchmarks
In these benchmarks, Skia Canvas is tested running in two modes: serial and async. When running serially, each rendering operation is awaited before continuing to the next test iteration. When running asynchronously, all the test iterations are begun at once and are executed in parallel using the library’s multi-threading support.

[See full results here…](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/index.md)

### [Startup latency](https://github.com/samizdatco/canvas-benchmarks/tree/main/tests/cold-start.js)
| Library              | Per Run   | Total Time (100 iterations)                   |
| -------------------- | --------- | --------------------------------------------- |
| *canvaskit-wasm*     | `  25 ms` | ` 2.47 s` ![ ](./docs/assets/benchmarks.svg#cold-start_wasm)      |
| *canvas*             | `  88 ms` | ` 8.77 s` ![ ](./docs/assets/benchmarks.svg#cold-start_canvas)    |
| *@napi-rs/canvas*    | `  69 ms` | ` 6.87 s` ![ ](./docs/assets/benchmarks.svg#cold-start_napi)      |
| *skia-canvas*        | `  <1 ms` | `  33 ms` ![ ](./docs/assets/benchmarks.svg#cold-start_skia-sync) |

### [Bezier curves](https://github.com/samizdatco/canvas-benchmarks/tree/main/tests/beziers.js)
| Library                                                       | Per Run   | Total Time (20 iterations)                  |
| ------------------------------------------------------------- | --------- | ------------------------------------------- |
| *canvaskit-wasm* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/beziers_wasm.png)            | ` 790 ms` | `15.81 s` ![ ](./docs/assets/benchmarks.svg#beziers_wasm)       |
| *canvas* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/beziers_canvas.png)                  | ` 486 ms` | ` 9.72 s` ![ ](./docs/assets/benchmarks.svg#beziers_canvas)     |
| *@napi-rs/canvas* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/beziers_napi.png)           | ` 230 ms` | ` 4.60 s` ![ ](./docs/assets/benchmarks.svg#beziers_napi)       |
| *skia-canvas (serial)* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/beziers_skia-sync.png) | ` 137 ms` | ` 2.74 s` ![ ](./docs/assets/benchmarks.svg#beziers_skia-sync)  |
| *skia-canvas (async)* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/beziers_skia-async.png) | `  28 ms` | ` 558 ms` ![ ](./docs/assets/benchmarks.svg#beziers_skia-async) |

### [SVG to PNG](https://github.com/samizdatco/canvas-benchmarks/tree/main/tests/from-svg.js)
| Library                                                        | Per Run   | Total Time (100 iterations)                  |
| -------------------------------------------------------------- | --------- | -------------------------------------------- |
| canvaskit-wasm                                                 | ` ————— ` | ` ————— `   *not supported*                  |
| *canvas* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/from-svg_canvas.png)                  | ` 122 ms` | `12.16 s` ![ ](./docs/assets/benchmarks.svg#from-svg_canvas)     |
| *@napi-rs/canvas* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/from-svg_napi.png)           | `  84 ms` | ` 8.42 s` ![ ](./docs/assets/benchmarks.svg#from-svg_napi)       |
| *skia-canvas (serial)* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/from-svg_skia-sync.png) | `  58 ms` | ` 5.83 s` ![ ](./docs/assets/benchmarks.svg#from-svg_skia-sync)  |
| *skia-canvas (async)* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/from-svg_skia-async.png) | `  11 ms` | ` 1.08 s` ![ ](./docs/assets/benchmarks.svg#from-svg_skia-async) |

### [Scale/rotate images](https://github.com/samizdatco/canvas-benchmarks/tree/main/tests/image-blit.js)
| Library                                                          | Per Run   | Total Time (50 iterations)                     |
| ---------------------------------------------------------------- | --------- | ---------------------------------------------- |
| *canvaskit-wasm* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/image-blit_wasm.png)            | ` 274 ms` | `13.72 s` ![ ](./docs/assets/benchmarks.svg#image-blit_wasm)       |
| *canvas* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/image-blit_canvas.png)                  | ` 283 ms` | `14.13 s` ![ ](./docs/assets/benchmarks.svg#image-blit_canvas)     |
| *@napi-rs/canvas* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/image-blit_napi.png)           | ` 112 ms` | ` 5.60 s` ![ ](./docs/assets/benchmarks.svg#image-blit_napi)       |
| *skia-canvas (serial)* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/image-blit_skia-sync.png) | ` 100 ms` | ` 5.00 s` ![ ](./docs/assets/benchmarks.svg#image-blit_skia-sync)  |
| *skia-canvas (async)* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/image-blit_skia-async.png) | `  19 ms` | ` 935 ms` ![ ](./docs/assets/benchmarks.svg#image-blit_skia-async) |

### [Basic text](https://github.com/samizdatco/canvas-benchmarks/tree/main/tests/text.js)
| Library                                                    | Per Run   | Total Time (200 iterations)              |
| ---------------------------------------------------------- | --------- | ---------------------------------------- |
| *canvaskit-wasm* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/text_wasm.png)            | `  24 ms` | ` 4.75 s` ![ ](./docs/assets/benchmarks.svg#text_wasm)       |
| *canvas* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/text_canvas.png)                  | `  24 ms` | ` 4.88 s` ![ ](./docs/assets/benchmarks.svg#text_canvas)     |
| *@napi-rs/canvas* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/text_napi.png)           | `  19 ms` | ` 3.83 s` ![ ](./docs/assets/benchmarks.svg#text_napi)       |
| *skia-canvas (serial)* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/text_skia-sync.png) | `  21 ms` | ` 4.26 s` ![ ](./docs/assets/benchmarks.svg#text_skia-sync)  |
| *skia-canvas (async)* [👁️](https://github.com/samizdatco/canvas-benchmarks/blob/main/results/darwin-arm64/2025-09-26/snapshots/text_skia-async.png) | `   4 ms` | ` 819 ms` ![ ](./docs/assets/benchmarks.svg#text_skia-async) |

## Acknowledgements

This project is deeply indebted to the work of the [Rust Skia project](https://github.com/rust-skia/rust-skia) whose Skia bindings provide a safe and idiomatic interface to the mess of C++ that lies underneath. Many thanks to the developers of [node-canvas](https://github.com/Automattic/node-canvas) for their terrific set of unit tests. In the absence of an [Acid Test](https://www.acidtests.org) for canvas, these routines were invaluable.


### Notable contributors

- [@mpaparno](https://github.com/mpaparno) contributed support for SVG rendering, raw image-buffer handling, WEBP import/export and numerous bug fixes
- [@Salmondx](https://github.com/Salmondx) developed the initial Raw image loading & rendering routines
- [@lucasmerlin](https://github.com/lucasmerlin) helped get GPU rendering working on Vulkan
- [@cprecioso](https://github.com/cprecioso) & [@saantonandre](https://github.com/saantonandre) corrected and expanded upon the TypeScript type definitions
- [@meihuanyu](https://github.com/meihuanyu) contributed filter & path rendering fixes

## Copyright
© 2020–2026 [Samizdat Drafting Co.](https://samizdat.co)

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
[p2d_slice]: https://skia-canvas.org/api/path2d#slice
[p2d_trim]: https://skia-canvas.org/api/path2d#trim
[edges]: https://skia-canvas.org/api/path2d#edges
[p2d_contours]: https://skia-canvas.org/api/path2d#contours
[p2d_positionAt]: https://skia-canvas.org/api/path2d#positionat
[toFile]: https://skia-canvas.org/api/canvas#tofile
[textwrap]: https://skia-canvas.org/api/context#textwrap
[toBuffer]: https://skia-canvas.org/api/canvas#tobuffer
[toURL]: https://skia-canvas.org/api/canvas#tourl
[win_bind]: https://skia-canvas.org/api/window#on--off--once
[win_events]: https://skia-canvas.org/api/window#events
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
[Image]: https://skia-canvas.org/api/image
[loadimage]: https://skia-canvas.org/api/image#loadimage
[loadcanvas]: https://skia-canvas.org/api/canvas#loadcanvas
[css_color_4]: https://developer.mozilla.org/en-US/blog/css-color-module-level-4/
[fontfeatures]: https://skia-canvas.org/api/context#fontfeaturesettings
[win_animation]: https://skia-canvas.org/api/window#events-for-animation
[ctx_colors]: https://skia-canvas.org/api/context#choosing-colors
