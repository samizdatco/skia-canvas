"use strict"

//
// Mime type <-> File extension mappings
//

const isWeb = typeof module=='undefined'

let png = "image/png",
    jpg = "image/jpeg",
    jpeg = "image/jpeg",
    webp = "image/webp",
    pdf = "application/pdf",
    svg = "image/svg+xml",
    expected = isWeb ? `"png", "jpg", or "webp"`
                     : `"png", "jpg", "pdf", or "svg"`,
    formats = isWeb ? {png, jpg, jpeg, webp}
                    : {png, jpg, jpeg, pdf, svg},
    mimes = isWeb ? {[png]: "png", [jpg]: "jpg", [webp]: "webp"}
                  : {[png]: "png", [jpg]: "jpg", [pdf]: "pdf", [svg]: "svg"},
    toMime = ext => formats[(ext||'').replace(/^\./, '').toLowerCase()],
    fromMime = mime => mimes[mime];

//
// Path methods from https://github.com/browserify/path-browserify/blob/master/index.js
//

function assertPath(path) {
  if (typeof path !== 'string') {
    throw new TypeError('Path must be a string. Received ' + JSON.stringify(path));
  }
}

function basename(path, ext) {
  if (ext !== undefined && typeof ext !== 'string') throw new TypeError('"ext" argument must be a string');
  assertPath(path);

  var start = 0;
  var end = -1;
  var matchedSlash = true;
  var i;

  if (ext !== undefined && ext.length > 0 && ext.length <= path.length) {
    if (ext.length === path.length && ext === path) return '';
    var extIdx = ext.length - 1;
    var firstNonSlashEnd = -1;
    for (i = path.length - 1; i >= 0; --i) {
      var code = path.charCodeAt(i);
      if (code === 47 /*/*/) {
          // If we reached a path separator that was not part of a set of path
          // separators at the end of the string, stop now
          if (!matchedSlash) {
            start = i + 1;
            break;
          }
        } else {
        if (firstNonSlashEnd === -1) {
          // We saw the first non-path separator, remember this index in case
          // we need it if the extension ends up not matching
          matchedSlash = false;
          firstNonSlashEnd = i + 1;
        }
        if (extIdx >= 0) {
          // Try to match the explicit extension
          if (code === ext.charCodeAt(extIdx)) {
            if (--extIdx === -1) {
              // We matched the extension, so mark this as the end of our path
              // component
              end = i;
            }
          } else {
            // Extension does not match, so our result is the entire path
            // component
            extIdx = -1;
            end = firstNonSlashEnd;
          }
        }
      }
    }

    if (start === end) end = firstNonSlashEnd;else if (end === -1) end = path.length;
    return path.slice(start, end);
  } else {
    for (i = path.length - 1; i >= 0; --i) {
      if (path.charCodeAt(i) === 47 /*/*/) {
          // If we reached a path separator that was not part of a set of path
          // separators at the end of the string, stop now
          if (!matchedSlash) {
            start = i + 1;
            break;
          }
        } else if (end === -1) {
        // We saw the first non-path separator, mark this as the end of our
        // path component
        matchedSlash = false;
        end = i + 1;
      }
    }

    if (end === -1) return '';
    return path.slice(start, end);
  }
}

function extname(path) {
  assertPath(path);
  var startDot = -1;
  var startPart = 0;
  var end = -1;
  var matchedSlash = true;
  // Track the state of characters (if any) we see before our first dot and
  // after any path separator we find
  var preDotState = 0;
  for (var i = path.length - 1; i >= 0; --i) {
    var code = path.charCodeAt(i);
    if (code === 47 /*/*/) {
      // If we reached a path separator that was not part of a set of path
      // separators at the end of the string, stop now
      if (!matchedSlash) {
        startPart = i + 1;
        break;
      }
      continue;
    }
    if (end === -1) {
      // We saw the first non-path separator, mark this as the end of our
      // extension
      matchedSlash = false;
      end = i + 1;
    }
    if (code === 46 /*.*/) {
        // If this is our first dot, mark it as the start of our extension
        if (startDot === -1)
          startDot = i;
        else if (preDotState !== 1)
          preDotState = 1;
    } else if (startDot !== -1) {
      // We saw a non-dot and non-path separator before our dot, so we should
      // have a good chance at having a non-empty extension
      preDotState = -1;
    }
  }

  if (startDot === -1 || end === -1 ||
      // We saw a non-dot character immediately before the dot
      preDotState === 0 ||
      // The (right-most) trimmed path component is exactly '..'
      preDotState === 1 && startDot === end - 1 && startDot === startPart + 1) {
    return '';
  }
  return path.slice(startDot, end);
 }


//
// Validation of the options dict shared by the Canvas saveAs, toBuffer, and toDataURL methods
//

function options(pages, {filename='', extension='', format, page, quality, density, outline}={}){
  var ext = format || extension.replace(/@\d+x$/i,'') || extname(filename),
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

  return {filename, pattern, format, mime, pages, padding, quality, density, outline}
}

//
// Zip (pace Phil Katz & q.v. https://github.com/jimmywarting/StreamSaver.js)
//

class Crc32 {
  static for(data){
    return new Crc32().append(data).get()
  }

  constructor(){ this.crc = -1 }

  append(data){
    var crc = this.crc | 0,
        table = this.table
    for (var offset = 0, len = data.length | 0; offset < len; offset++) {
      crc = (crc >>> 8) ^ table[(crc ^ data[offset]) & 0xFF]
    }
    this.crc = crc
    return this
  }

  get(){ return ~this.crc }

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

const calloc = byteLength => {
  var uint8 = new Uint8Array(byteLength)
  return {
    array: uint8,
    view: new DataView(uint8.buffer),
    size: byteLength
  }
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
        bytes = new Uint8Array(folder ? 0 : await blob.arrayBuffer()),
        preamble = 30 + name.length,
        postamble = 16,
        {offset} = this

    let header = calloc(26);
    header.view.setUint32(0, 0x14000808)              // zip version
    header.view.setUint16(6, this.time, true)         // time
    header.view.setUint16(8, this.date, true)         // date
    header.view.setUint32(10, Crc32.for(bytes), true) // no compression
    header.view.setUint32(14, bytes.length, true)     // compressed size (w/ zero compression)
    header.view.setUint32(18, bytes.length, true)     // un-compressed size
    header.view.setUint16(22, name.length, true)      // filename length (utf8 bytes)
    offset += preamble

    let payload = calloc(preamble + bytes.length + postamble)
    payload.view.setUint32(0, 0x504b0304) // local header signature
    payload.array.set(header.array, 4)    // ...header fields...
    payload.array.set(name, 30)           // filename
    payload.array.set(bytes, preamble)    // blob bytes
    offset += bytes.length

    let descriptor = preamble + bytes.length
    payload.view.setUint32(descriptor, 0x504b0708)               // signature
    payload.array.set(header.array.slice(10,22), descriptor + 4) // length & filemame
    offset += postamble

    this.files.push({name, header, payload, folder, offset})
    this.offset = offset
  }

  toBuffer(){
    let length = this.files.reduce((len, {name}) => 46 + name.length + len, 0),
        cdr = calloc(length + 22),
        index = 0

    for (var {name, header, folder, offset} of this.files){
      cdr.view.setUint32(index, 0x504b0102)        // archive file signature
      cdr.view.setUint16(index + 4, 0x1400)        // version
      cdr.array.set(header.array, index + 6)
      if (folder){
        cdr.view.setUint8(index + 38, 0x10)        // is_dir flag
      }
      cdr.view.setUint32(index + 42, offset, true) // file offset
      cdr.array.set(name, index + 46)              // filename
      index += 46 + name.length
    }
    cdr.view.setUint32(index, 0x504b0506)                   // signature
    cdr.view.setUint16(index + 8, this.files.length, true)  // № files per-segment
    cdr.view.setUint16(index + 10, this.files.length, true) // № files this segment
    cdr.view.setUint32(index + 12, length, true)            // central directory length
    cdr.view.setUint32(index + 16, this.offset, true)       // file-offset of directory

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

const asBlob = (canvas, mime, quality) => new Promise((res, rej) =>
  canvas.toBlob(res, mime, quality)
)

const asBuffer = (...args) => asBlob(...args).then(b => b.arrayBuffer())

const asDownload = (canvas, mime, quality, filename) => new Promise(
  (res, rej) => canvas.toBlob(res, mime, quality)
).then(blob => _download(filename, blob))

const asZipDownload = async (contexts, mime, quality, pattern, padding) => {
  let archive = 'canvas',
      zip = new Zip(archive)

  await Promise.all(contexts.map(
    c2d => asBlob(c2d.canvas, mime, quality)
  )).then(blobs => Promise.all(blobs.map(async (blob, i) => {
    let filename = pattern.replace('{}', String(i+1).padStart(padding, '0'))
    await zip.add(filename, blob)
  })))

  _download(`${archive}.zip`, zip.blob)
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

if (isWeb){
  window.io = {asBuffer, asDownload, asZipDownload, options}
}else{
  module.exports = {fromMime, toMime, options}
}
