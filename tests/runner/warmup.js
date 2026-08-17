//
// Preloaded (via --require) by the `full` & `debug` recipes so per-test timings are accurate.
//
// Whichever test first touches the native layer otherwise absorbs that file's entire one-time
// setup — the font collection built over every installed family (~35ms here, 418 families) plus
// the rendering engine's device init — and gets flagged as slow for work it didn't do. Since
// node runs each test file in its own process, every one of them pays it independently.
//

'use strict'

const {Canvas, FontLibrary} = require('../../lib')

// build the font collection
FontLibrary.families

// run an initial rasterization (init any GPUs)
const ctx = new Canvas(2, 2).getContext('2d')
ctx.fillRect(0, 0, 1, 1)
ctx.getImageData(0, 0, 1, 1)
