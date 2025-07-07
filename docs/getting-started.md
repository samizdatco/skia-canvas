---
sidebar_position: 1
title: Getting Started
---
## Installation

If you’re running on a supported platform, installation should be as simple as:
```bash
npm install skia-canvas
```

This will download a pre-compiled library from the project’s most recent [release](https://github.com/samizdatco/skia-canvas/releases).

## Platform Support

The underlying Rust library uses [N-API][node_napi] v8 which allows it to run on Node.js versions:
  - v12.22+
  - v14.17+
  - v15.12+
  - v16.0.0 and later

Pre-compiled binaries are available for:

  - Linux (x64 & arm64)
  - macOS (x64 & Apple silicon)
  - Windows (x64 & arm64)

Nearly everything you need is statically linked into the library. A notable exception is the [Fontconfig](https://www.freedesktop.org/wiki/Software/fontconfig/) library which must be installed separately if you’re running on Linux.

## Running in Docker

The library is compatible with Linux systems using [glibc](https://www.gnu.org/software/libc/) 2.28 or later as well as Alpine Linux (x64 & arm64) and the [musl](https://musl.libc.org) C library it favors. In both cases, Fontconfig must be installed on the system for `skia-canvas` to operate correctly.

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

<!-- references_begin -->
[canvas]: api/canvas.md
[engine]: api/canvas.md#engine
[saveAs]: api/canvas.md#saveas
[toBuffer]: api/canvas.md#tobuffer
[node_napi]: https://nodejs.org/api/n-api.html#node-api-version-matrix
[node_env]: https://nodejs.org/en/learn/command-line/how-to-read-environment-variables-from-nodejs
[node_env_arg]: https://nodejs.org/dist/latest-v22.x/docs/api/cli.html#--env-fileconfig
[rayon]: https://crates.io/crates/rayon
<!-- references_end -->
