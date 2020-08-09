const classes = require('./classes'),
      geometry = require('./geometry');

function loadImage(src) {
  return new Promise(function (res, rej) {
    Object.assign(new classes.Image(), {
      onload(img){ res(img) },
      onerror(err){ rej(err) },
      src
    })
  })
}

module.exports = Object.assign({loadImage}, classes, geometry)