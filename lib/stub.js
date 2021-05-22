// browser equivalents of the skia-canvas convenience initializers

const loadImage = src => new Promise((onload, onerror) =>
  Object.assign(new classes.Image(), {onload, onerror, src})
)

class Canvas{
  constructor(width, height){
    return Object.assign(document.createElement('canvas'), {width, height})
  }
}

module.exports = {Canvas, loadImage}