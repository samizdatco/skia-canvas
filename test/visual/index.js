var path = require('path')
var express = require('express')
var {Canvas} = require('../../lib')
var tests = require('./tests')

function renderTest (canvas, name, cb) {
  if (!tests[name]) {
    throw new Error('Unknown test: ' + name)
  }

  try{
    var ctx = canvas.getContext('2d')
    var initialFillStyle = ctx.fillStyle
    ctx.fillStyle = 'white'
    ctx.fillRect(0, 0, 200, 200)
    ctx.fillStyle = initialFillStyle
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

var app = express()
app.use(express.static(path.join(__dirname, '../assets')))
app.use(express.static(path.join(__dirname)))

app.get('/', function (req, res) {
  res.sendFile(path.join(__dirname, 'inspect.html'))
})

app.get('/render', function (req, res, next) {
  var canvas = new Canvas(200, 200)

  renderTest(canvas, req.query.name, function (err) {
    if (err) return next(err)

    let data = canvas.png
    res.contentType('image/png');
    res.send(data)


  })
})

app.get('/pdf', function (req, res, next) {
  var canvas = new Canvas(200, 200)

  renderTest(canvas, req.query.name, function (err) {
    if (err) return next(err)

    let data = canvas.pdf
    res.contentType('application/pdf');
    res.send(data)


  })
})

var port = parseInt(process.argv[2] || '4000', 10)
app.listen(port, function () {
  console.log('=> http://localhost:%d/', port)
})
