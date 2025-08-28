/*
Usage: node test/visual/index.js [port #] [options]
   ex: node test/visual/index.js 8081 -g 1
   ex: node test/visual/index.js -p 8081 -w 300

options:
  [-p[port]] <number>  Set listening port number; default: 4000
  -g[pu]   (0|1)       Enable/disable GPU; default: default for skia-canvas build
  -w[idth] <number>    Set canvas width;  default: 200
  -h[eight] <number>   Set canvas height; default: 200
  -c[c] <css color>    Set default canvas background color; default: white
  -b[c] <css color>    Set the page background color; default: system preference (dark/light)

The leading '-' is optional.
All options besides 'port' can also be set via URL parameters using their full names (eg. '?width=250&gpu=1").
*/
"use strict";

const fs = require('fs/promises'),
      path = require('path'),
      {Hono} = require('hono'),
      {serve} = require('@hono/node-server'),
      {serveStatic} = require('@hono/node-server/serve-static'),
      {getCookie, setCookie} = require('hono/cookie'),
      {Canvas} = require('../../lib'),
      tests = require('./tests')


// Default options
const defaults = {
  width: 200,        // canvas dimensions
  height: 200,
  cc: '#FFFFFF',     // default canvas fill color
  bc: undefined,     // page bg color
  gpu: undefined,    // use gpu
}

const MIME = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  webp: "image/webp",
  pdf: "application/pdf",
  svg: "image/svg+xml",
}

var port = 4000  // server listening port

// Runtime options
const option = {}

function renderTest(canvas, name, option, format) {
  if (!tests[name]) {
    throw new Error('Unknown test: ' + name)
  }

  return new Promise((res, rej) => {
    let cb = async (err) => err ? rej(err) : res(await canvas.toBuffer(format))

    if (option.gpu != undefined)
      canvas.gpu = option.gpu

    try{
      var ctx = canvas.getContext('2d')
      var initialFillStyle = ctx.fillStyle
      ctx.fillStyle = option.cc
      ctx.fillRect(0, 0, canvas.width, canvas.height)
      ctx.fillStyle = initialFillStyle
      ctx.imageSmoothingEnabled = true
      if (tests[name].length === 2) {
        tests[name](ctx, cb)
      } else {
        tests[name](ctx)
        cb(null)
      }
    }catch(e){
      console.error(e)
      cb(e)
    }
  })

}

var app = new Hono()

// must go before static routes
app.get('/', async (c) => {
  // update renderOptions cookie based on presence/absence of query args
  let query = c.req.query()
  if (Object.keys(query).length == 0) {
    // full reset when no query string for initial request or 'reset' button
    setCookie(c, "renderOptions", JSON.stringify(defaults))
  }else{
    let opts = {}
    opts.width = parseInt(query.width) || defaults.width
    opts.height = parseInt(query.height) || defaults.height
    opts.gpu = query.gpu && query.gpu != 'null' ? !!parseInt(query.gpu) : defaults.gpu
    opts.cc = query.cc ? decodeURIComponent(query.cc) : defaults.cc
    if (query.alpha != null && opts.cc.length == 7 && opts.cc[0] == '#') {
      // add alpha component into overall color (because browser's color input doesn't do alpha)
      const a = Math.max(Math.min(Math.round(255 * parseFloat(query.alpha)), 255), 0)
      if (Number.isFinite(a))
        opts.cc += a.toString(16).padStart(2, '0')
    }
    opts.bc_default = query.bc_default != null
    if (query.bc) option.bc = decodeURIComponent(query.bc)
    setCookie(c, "renderOptions", JSON.stringify(opts))
  }

  return c.html(await fs.readFile(path.join(__dirname, 'index.html')))
})

// merge tests.js and assets dir contents into root
app.use('/tests.js', serveStatic({ root: __dirname }))
app.use('/*', serveStatic({ root: path.join(__dirname, '../assets') }))

app.get('/:format{(png|jpg|webp|pdf|svg)}', async (c) => {
  let cookie = getCookie(c, "renderOptions"),
      opts = cookie ? JSON.parse(cookie) : {...defaults},
      {format} = c.req.param(),
      test = c.req.query('name')

  let canvas = new Canvas(opts.width, opts.height),
      data = await renderTest(canvas, test, opts, format)
  return c.body(data, 200, {'Content-Type': MIME[format]})
})

// Handle CLI arguments; these set default options
for (let i=2; i < process.argv.length; ++i) {
  const arg = process.argv[i];
  if (i==2 && parseInt(arg))      port = parseInt(arg);
  else if ((/^-?p/i).test(arg))   port = parseInt(process.argv[++i]) || port;
  else if ((/^-?g/i).test(arg))   defaults.gpu = !!parseInt(process.argv[++i]);
  else if ((/^-?w/i).test(arg))   defaults.width = parseInt(process.argv[++i]) || defaults.width;
  else if ((/^-?h/i).test(arg))   defaults.height = parseInt(process.argv[++i]) || defaults.height;
  else if ((/^-?c/i).test(arg))   defaults.cc = process.argv[++i];
  else if ((/^-?b/i).test(arg)) { defaults.bc = process.argv[++i]; option.bc_default = false; }
  else console.log("Ignoring unknown argument:", arg)
}

console.log('=> http://localhost:%d/', port)
serve({
  fetch: app.fetch,
  port: port,
})
