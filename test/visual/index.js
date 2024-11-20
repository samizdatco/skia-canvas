/*
Usage: node[mon] test/visual/index.js [port #] [options]
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

var path = require('path')
var express = require('express')
var {Canvas} = require('../../lib')
var tests = require('./tests')


// Default options
const defaults = {
  width: 200,        // canvas dimensions
  height: 200,
  cc: '#FFFFFF',     // default canvas fill color
  bc: undefined,     // page bg color
  gpu: undefined,    // use gpu
}

var port = 4000  // server listening port

// Runtime options
const option = {}

function renderTest (canvas, name, cb) {
  if (!tests[name]) {
    throw new Error('Unknown test: ' + name)
  }

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
}

function setOptionsFromQuery(query) {
  if (query.width == null) {
    // full reset when no query string for initial request or 'reset' button
    Object.assign(option, defaults)
    return
  }
  option.width = parseInt(query.width) || defaults.width
  option.height = parseInt(query.height) || defaults.height
  option.gpu = query.gpu && query.gpu != 'null' ? !!parseInt(query.gpu) : defaults.gpu
  option.cc = query.cc ? decodeURIComponent(query.cc) : defaults.cc
  if (query.alpha != null && option.cc.length == 7 && option.cc[0] == '#') {
    // add alpha component into overall color (because browser's color input doesn't do alpha)
    const a = Math.max(Math.min(Math.round(255 * parseFloat(query.alpha)), 255), 0)
    if (Number.isFinite(a))
      option.cc += a.toString(16).padStart(2, '0')
  }
  option.bc_default = query.bc_default != null
  if (query.bc)
    option.bc = decodeURIComponent(query.bc)
  // console.log(option)
}

var app = express()

// must go before static routes
app.get('/', function (req, res) {
  setOptionsFromQuery(req.query)
  res.cookie("renderOptions", JSON.stringify(option), { sameSite: 'Strict' })
  res.sendFile(path.join(__dirname, 'index.html'))
})

app.use(express.static(path.join(__dirname, '../assets')))
app.use(express.static(path.join(__dirname)))

app.get('/render', async function (req, res, next) {
  var canvas = new Canvas(option.width, option.height)
  if (option.gpu != undefined)
    canvas.gpu = option.gpu

  renderTest(canvas, req.query.name, async function (err) {
    if (err) return next(err)

    let data = await canvas.png
    res.contentType('image/png');
    res.send(data)
  })
})

app.get('/pdf', async function (req, res, next) {
  var canvas = new Canvas(option.width, option.height)

  renderTest(canvas, req.query.name, async function (err) {
    if (err) return next(err)

    let data = await canvas.pdf
    res.contentType('application/pdf');
    res.send(data)
  })
})

app.get('/svg', async function (req, res, next) {
  var canvas = new Canvas(option.width, option.height)

  renderTest(canvas, req.query.name, async function (err) {
    if (err) return next(err)

    let data = await canvas.svg
    res.contentType('image/svg+xml');
    res.send(data)
  })
})

// Handle CLI arguments; these set default options
for (let i=2; i < process.argv.length; ++i) {
  const arg = process.argv[i];
  if   (typeof arg == 'number')   port = arg;
  else if ((/^-?p/i).test(arg))   port = parseInt(process.argv[++i]) || port;
  else if ((/^-?g/i).test(arg))   defaults.gpu = !!parseInt(process.argv[++i]);
  else if ((/^-?w/i).test(arg))   defaults.width = parseInt(process.argv[++i]) || defaults.width;
  else if ((/^-?h/i).test(arg))   defaults.height = parseInt(process.argv[++i]) || defaults.height;
  else if ((/^-?c/i).test(arg))   defaults.cc = process.argv[++i];
  else if ((/^-?b/i).test(arg)) { defaults.bc = process.argv[++i]; option.bc_default = false; }
  else console.log("Ignoring unknown argument:", arg)
}

app.listen(port, function () {
  console.log('=> http://localhost:%d/', port)
  // console.log('   with options:', defaults)
})
