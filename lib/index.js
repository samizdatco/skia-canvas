const classes = require('./classes'),
      geometry = require('./geometry');

const loadImage = src => new Promise((onload, onerror) =>
  Object.assign(new classes.Image(), {onload, onerror, src})
)

module.exports = Object.assign({loadImage}, classes, geometry)