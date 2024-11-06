---
sidebar_position: 1
---

# Getting Started

## Installation

If you’re running on a supported platform, installation should be as simple as:
```bash
npm install skia-canvas
```

This will download a pre-compiled library from the project’s most recent [release](https://github.com/samizdatco/skia-canvas/releases).

### Platform Support

The underlying Rust library uses [N-API](https://nodejs.org/api/n-api.html) v6 which allows it to run on Node.js versions:
  - 10.20+
  - 12.17+
  - 14.0, 15.0, and later

Pre-compiled binaries are available for:

  - Linux (x64, arm64, & armhf)
  - macOS (x64 & Apple silicon)
  - Windows (x64)

Nearly everything you need is statically linked into the library. A notable exception is the [Fontconfig](https://www.freedesktop.org/wiki/Software/fontconfig/) library which must be installed separately if you’re running on Linux.


### Running in Docker

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

### Compiling from Source

If prebuilt binaries aren’t available for your system you’ll need to compile the portions of this library that directly interface with Skia.

Start by installing:

  1. The [Rust compiler](https://www.rust-lang.org/tools/install) and cargo package manager using [`rustup`](https://rust-lang.github.io/rustup/)
  2. A C compiler toolchain like LLVM/Clang or MSVC
  3. Python 2.7 (used by Skia's [build process](https://skia.org/docs/user/build/))
  4. On Linux: Fontconfig and OpenSSL

[Detailed instructions](https://github.com/rust-skia/rust-skia#building) for setting up these dependencies on different operating systems can be found in the ‘Building’ section of the Rust Skia documentation. Once all the necessary compilers and libraries are present, running `npm run build` will give you a usable library (after a fairly lengthy compilation process).
