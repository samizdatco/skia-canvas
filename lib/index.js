const classes = require('./classes'),
      geometry = require('./geometry'),
      {loadImage} = require('./utils');

module.exports = Object.assign({loadImage}, classes, geometry)