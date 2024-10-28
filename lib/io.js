"use strict"

const {basename, extname} = require('path')

//
// Mime type <-> File extension mappings
//

class Format{
  constructor(){
    let isWeb = (() => typeof global=='undefined')(),
        png = "image/png",
        jpg = "image/jpeg",
        jpeg = "image/jpeg",
        webp = "image/webp",
        pdf = "application/pdf",
        svg = "image/svg+xml"

    Object.assign(this, {
      toMime: this.toMime.bind(this),
      fromMime: this.fromMime.bind(this),
      expected: isWeb ? `"png", "jpg", or "webp"`
                      : `"png", "jpg", "webp", "pdf", or "svg"`,
      formats: isWeb ? {png, jpg, jpeg, webp}
                     : {png, jpg, jpeg, webp, pdf, svg},
      mimes: isWeb ? {[png]: "png", [jpg]: "jpg", [webp]: "webp"}
                   : {[png]: "png", [jpg]: "jpg", [webp]: "webp", [pdf]: "pdf", [svg]: "svg"},
    })
  }

  toMime(ext){
    return this.formats[(ext||'').replace(/^\./, '').toLowerCase()]
  }

  fromMime(mime){
    return this.mimes[mime]
  }
}

//
// Validation of the options dict shared by the Canvas saveAs, toBuffer, and toDataURL methods
//

function options(pages, {filename='', extension='', format, page, quality, matte, density, outline, archive}={}){
  var {fromMime, toMime, expected} = new Format(),
      archive = archive || 'canvas',
      ext = format || extension.replace(/@\d+x$/i,'') || extname(filename),
      format = fromMime(toMime(ext) || ext),
      mime = toMime(format),
      pp = pages.length

  if(!ext) throw new Error(`Cannot determine image format (use a filename extension or 'format' argument)`)
  if (!format) throw new Error(`Unsupported file format "${ext}" (expected ${expected})`)
  if (!pp) throw new RangeError(`Canvas has no associated contexts (try calling getContext or newPage first)`)

  let padding, isSequence, pattern = filename.replace(/{(\d*)}/g, (_, width) => {
    isSequence = true
    width = parseInt(width, 10)
    padding = isFinite(width) ? width : isFinite(padding) ? padding : -1
    return "{}"
  })

  // allow negative indexing if a specific page is specified
  let idx = page > 0 ? page - 1
          : page < 0 ? pp + page
          : undefined;

  if (isFinite(idx) && idx < 0 || idx >= pp) throw new RangeError(
    pp == 1 ? `Canvas only has a ‘page 1’ (${idx} is out of bounds)`
            : `Canvas has pages 1–${pp} (${idx} is out of bounds)`
  )

  pages = isFinite(idx) ? [pages[idx]]
        : isSequence || format=='pdf' ? pages
        : pages.slice(-1) // default to the 'current' context

  if (quality===undefined){
    quality = 0.92
  }else{
    if (typeof quality!='number' || !isFinite(quality) || quality<0 || quality>1){
      throw new TypeError("The quality option must be an number in the 0.0–1.0 range")
    }
  }

  if (density===undefined){
    let m = (extension || basename(filename, ext)).match(/@(\d+)x$/i)
    density = m ? parseInt(m[1], 10) : 1
  }else if (typeof density!='number' || !Number.isInteger(density) || density<1){
    throw new TypeError("The density option must be a non-negative integer")
  }

  if (outline===undefined){
    outline = true
  }else if (format == 'svg'){
    outline = !!outline
  }

  return {filename, pattern, format, mime, pages, padding, quality, matte, density, outline, archive}
}

//
// Zip (pace Phil Katz & q.v. https://github.com/jimmywarting/StreamSaver.js)
//

class Crc32 {
  static for(data){
    return new Crc32().append(data).get()
  }

  constructor(){ this.crc = -1 }

  get(){ return ~this.crc }

  append(data){
    var crc = this.crc | 0,
        table = this.table
    for (var offset = 0, len = data.length | 0; offset < len; offset++) {
      crc = (crc >>> 8) ^ table[(crc ^ data[offset]) & 0xFF]
    }
    this.crc = crc
    return this
  }

}

Crc32.prototype.table = (() => {
  var i, j, t, table = []
  for (i = 0; i < 256; i++) {
    t = i
    for (j = 0; j < 8; j++) {
      t = (t & 1)
        ? (t >>> 1) ^ 0xEDB88320
        : t >>> 1
    }
    table[i] = t
  }
  return table
})()

function calloc(size){
  let array = new Uint8Array(size),
      view = new DataView(array.buffer),
      buf = {
        array, view, size,
        set8(at, to){ view.setUint8(at, to); return buf },
        set16(at, to){ view.setUint16(at, to, true); return buf },
        set32(at, to){ view.setUint32(at, to, true); return buf },
        bytes(at, to){ array.set(to, at); return buf },
      }
  return buf
}

class Zip{
  static encoder = new TextEncoder()

  constructor(directory){
    let now = new Date()
    Object.assign(this, {
      directory,
      offset: 0,
      files: [],
      time: (((now.getHours() << 6) | now.getMinutes()) << 5) | now.getSeconds() / 2,
      date: ((((now.getFullYear() - 1980) << 4) | (now.getMonth() + 1)) << 5) | now.getDate(),
    })
    this.add(directory)
  }

  async add(filename, blob){
    let folder = !blob,
        name = Zip.encoder.encode(`${this.directory}/${folder ? '' : filename}`),
        data = new Uint8Array(folder ? 0 : await blob.arrayBuffer()),
        preamble = 30 + name.length,
        descriptor = preamble + data.length,
        postamble = 16,
        {offset} = this

    let header = calloc(26)
      .set32(0, 0x08080014)       // zip version
      .set16(6, this.time)        // time
      .set16(8, this.date)        // date
      .set32(10, Crc32.for(data)) // checksum
      .set32(14, data.length)     // compressed size (w/ zero compression)
      .set32(18, data.length)     // un-compressed size
      .set16(22, name.length)     // filename length (utf8 bytes)
    offset += preamble

    let payload = calloc(preamble + data.length + postamble)
      .set32(0, 0x04034b50)   // local header signature
      .bytes(4, header.array) // ...header fields...
      .bytes(30, name)        // filename
      .bytes(preamble, data)  // blob bytes
    offset += data.length

    payload
      .set32(descriptor, 0x08074b50)                    // signature
      .bytes(descriptor + 4, header.array.slice(10,22)) // length & filemame
    offset += postamble

    this.files.push({offset, folder, name, header, payload})
    this.offset = offset
  }

  toBuffer(){
    // central directory record
    let length = this.files.reduce((len, {name}) => 46 + name.length + len, 0),
        cdr = calloc(length + 22),
        index = 0

    for (var {offset, name, header, folder} of this.files){
      cdr.set32(index, 0x02014b50)            // archive file signature
         .set16(index + 4, 0x0014)            // version
         .bytes(index + 6, header.array)      // ...header fields...
         .set8(index + 38, folder ? 0x10 : 0) // is_dir flag
         .set32(index + 42, offset)           // file offset
         .bytes(index + 46, name)             // filename
      index += 46 + name.length
    }
    cdr.set32(index, 0x06054b50)             // signature
       .set16(index + 8, this.files.length)  // № files per-segment
       .set16(index + 10, this.files.length) // № files this segment
       .set32(index + 12, length)            // central directory length
       .set32(index + 16, this.offset)       // file-offset of directory

    // concatenated zipfile data
    let output = new Uint8Array(this.offset + cdr.size),
        cursor = 0;

    for (var {payload} of this.files){
      output.set(payload.array, cursor)
      cursor += payload.size
    }
    output.set(cdr.array, cursor)

    return output
  }

  get blob(){
    return new Blob([this.toBuffer()], {type:"application/zip"})
  }
}


//
// Browser helpers for converting canvas elements to blobs/buffers/files/zips
//

const asBlob = (canvas, mime, quality, matte) => {
  if (matte){
    let {width, height} = canvas,
        comp = Object.assign(document.createElement('canvas'), {width, height}),
        ctx = comp.getContext("2d")
    ctx.fillStyle = matte
    ctx.fillRect(0, 0, width, height)
    ctx.drawImage(canvas, 0, 0)
    canvas = comp
  }

  return new Promise((res, rej) => canvas.toBlob(res, mime, quality))
}

const asBuffer = (...args) => asBlob(...args).then(b => b.arrayBuffer())

const asDownload = async (canvas, mime, quality, matte, filename) => {
  _download(filename, await asBlob(canvas, mime, quality, matte))
}

const asZipDownload = async (pages, mime, quality, matte, archive, pattern, padding) => {
  let filenames = i => pattern.replace('{}', String(i+1).padStart(padding, '0')),
      folder = basename(archive, '.zip') || 'archive',
      zip = new Zip(folder)

  await Promise.all(pages.map(async (page, i) => {
    let filename = filenames(i) // serialize filename(s) before awaiting
    await zip.add(filename, await asBlob(page, mime, quality, matte))
  }))

  _download(`${folder}.zip`, zip.blob)
}

const _download = (filename, blob) => {
  const href = window.URL.createObjectURL(blob),
        link = document.createElement('a')
  link.style.display = 'none'
  link.href = href
  link.setAttribute('download', filename)
  if (typeof link.download === 'undefined') {
    link.setAttribute('target', '_blank')
  }
  document.body.appendChild(link)
  link.click()
  document.body.removeChild(link)
  setTimeout(() => window.URL.revokeObjectURL(href), 100)
}

const atScale = (pages, density, matte) => pages.map(page => {
  if (density == 1 && !matte) return page.canvas

  let scaled = document.createElement('canvas'),
      ctx = scaled.getContext("2d"),
      src = page.canvas ? page.canvas : page
  scaled.width = src.width * density
  scaled.height = src.height * density
  if (matte){
    ctx.fillStyle = matte
    ctx.fillRect(0, 0, scaled.width, scaled.height)
  }
  ctx.scale(density, density)
  ctx.drawImage(src, 0, 0)
  return scaled
})

module.exports = {asBuffer, asDownload, asZipDownload, atScale, options}
