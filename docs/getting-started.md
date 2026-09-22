---
sidebar_position: 1
title: Getting Started
toc_max_heading_level: 3
---
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

<!-- references_begin -->
[canvas]: api/canvas.md
[engine]: api/canvas.md#engine
[toFile]: api/canvas.md#tofile
[toBuffer]: api/canvas.md#tobuffer
[node_napi]: https://nodejs.org/api/n-api.html#node-api-version-matrix
[node_env]: https://nodejs.org/en/learn/command-line/how-to-read-environment-variables-from-nodejs
[node_env_arg]: https://nodejs.org/dist/latest-v22.x/docs/api/cli.html#--env-fileconfig
[rayon]: https://crates.io/crates/rayon
<!-- references_end -->
