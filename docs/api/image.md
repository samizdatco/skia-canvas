---
description: Bitmap & vector image container
---

# Image

> Skia Canvas's `Image` object is a stripped-down version of the [standard **Image**][img_element] used in browser environments. Since the Canvas API ignores most of its properties, only the relevant ones have been recreated here. Use the asynchronous [`loadImage()`][loadimage] helper function to create an `Image` from any supported data source.

| Content                                        | Loading                      | Event Handlers                                            | Memory |
| --                                             | --                           | --                                                        | --     |
| [**src**][img_src]                             | [**complete**][img_complete] | [**onload**][img_onload] / [**onerror**][img_onerror]     | [**disposed**](#disposed) |
| [**width**][img_size] / [**height**][img_size] | [decode()][img_decode]       | [on()][img_bind] / [off()][img_bind] / [once()][img_bind] | [dispose()](#dispose) |
|                                                |                              |                                                           | [release()](#release) |

## Loading `Image` objects

Before an image file can be drawn to the canvas a number of behind-the-scenes steps have to take place: its data has to be loaded, potentially from a remote system, its format needs to be determined, and its data must be decompressed. As a result, newly created Image objects are not ready for use; instead you must asynchronously wait for them to complete their loading & decoding process before making use of them.

### Callbacks
The traditional way to do this is to set up an event listener waiting for the `load` event. You can do this either by assigning a callback function to the image's [`onload`][img_onload] property, or by using the [on()][img_bind] or [once()][img_bind] methods to set up the listener by name. Once the event handler has been set up, you can then kick off the load process by setting a `src` value for the image:

```js
let img = new Image()
img.onload = function(theImage){
  // for non-arrow functions, the image object is also passed as `this`
  ctx.drawImage(this, 100, 100)
}
img.src = 'https://skia-canvas.org/icon.png'
```

Or, equivalently:

```js
img.on("load", (theImage) => {
  // arrow functions can use the image reference passed as an argument
  ctx.drawImage(theImage, 100, 100)
})
```

### Promises

If you're setting up an Image within an asynchronous function, you can avoid some of this ‘callback hell’ by using the `await` keyword in combination with the Image's [decode()][img_decode] method. It returns a [Promise][Promise] which resolves only once the load process is complete and the image is ready for use, making it convenient for pausing execution before drawing the image:

```js
let img = new Image()
img.src = 'https://skia-canvas.org/icon.png'
await img.decode()
ctx.drawImage(img, 100, 100)
```

To cut down on this repetitive boilerplate, you can also use the [loadImage()][loadimage] utility function which wraps both image creation and loading, allowing for even more concise initialization. For instance, the previous example could be rewritten as:

```js
let img = await loadImage('https://skia-canvas.org/icon.png')
ctx.drawImage(img, 100, 100)
```

### Cleaning Up

Each `Image` objects holds references to native resources (raw image bytes, cache buffers, etc.) that won't be released until the object is reclaimed by the garbage collector. Since the collector only runs in between ticks of the Node event loop this means that collection will be deferred even if your code is asychronous, and will not happen *at all* if you're loading and rendering images within a sychronous loop. The best way to handle these situations is to use explicit resource management (see also the [Canvas object's support][canvas_cleanup] for this).

On Node versions 24 and later, the `using` and `await using` keywords will allow you to mark an Image object for disposal as soon as it goes out of the scope in which it was created. On Node 22 and earlier you can achieve the same effect by using the [`dispose()`](#dispose) and [`release()`](#release) methods.

In a synchronous loop, you can use the `using` keyword:

```js
let canvas = new Canvas(150, 150),
    ctx = canvas.getContext('2d')

for (let path of imagePaths){
  using img = new Image(fs.readFileSync(path))
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
  canvas.toFileSync(path.replace(/\.\w+$/, '-thumb.png'))
} // ← calls img.dispose() at the end of each loop iteration
```

With asynchronous code, use the `await using` keyword:

```js
for (let url of imageURLs){
  await using img = await loadImage(url)
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
  await canvas.toFile(path.replace(/\.\w+$/, '-thumb.png'))
} // ← calls img.release() at the end of each loop iteration
```

## Constructor

```js returns="Image"
new Image(buffer)    // a Buffer or ArrayBuffer object
new Image(dataURL)   // a String with a valid `data:` url
new Image(data, src) // optionally include a `src` string
```

While loading images from remote sources is inherently asynchronous, if you've already fetched a file yourself you can create an Image synchronously by passing its data to the Image constructor. The data can be in any of the *encoded* formats Skia Canvas supports (`png`, `jpeg`, `webp`, `svg`, or `pdf`) but can't be raw pixel data—for that you should use [ImageData][imgdata_new] instead.

For example, you can synchronously create an image from a local file via:
```js prints="Image { width:100, height:100, complete:true, src:'::Buffer::' }"
import {readFileSync} from 'fs'

let data = readFileSync("my-image.svg")
let img = new Image(data)

console.log(img)
```

Note that if you just provide the data, the `src` property on the resulting `Image` will be a placeholder. If you'd like to be able to read from the `src` later (perhaps to help identify the image), you can optionally pass a string as the second argument. Note that it isn't actually used for loading the image so it needn't be a valid URL:

```js prints="Image { width:100, height:100, complete:true, src:'this string can be anything' }"
let img = new Image(data, 'this string can be anything')
console.log(img)
```

## Properties

### `.src`

Setting the `src` property will kick off the loading process. While the browser version of this property requires a string containing a URL, here the `src` can be any of the following:
- an HTTP URL to asynchronously retrieve the image from
- an absolute or relative path pointing to a file on the local system
- a [Data URL][DataURL] with the image data base64-encoded into the string (or [url-encoded][url_encode] in the case of SVG images)
- a [Buffer][Buffer] containing the pre-loaded bytes of a supported image file format
- a [Sharp][sharp] bitmap image object (if the library has been [installed separately][sharp_npm])

The images you load can be from a variety of formats:
- Bitmap: `png`, `jpeg`, or `webp`
- Vector: `svg` or `pdf`

Note that the image will be [`complete`][img_complete] immediately if a Buffer or Data URL was used, but otherwise you'll need to [wait for it to load](#loading-image-objects).

### `.width` & `.height`

In the browser these are writable properties that can control the display size of the image within the HTML page. But the context's [`drawImage`][drawImage()] method ignores them in favor of the image's intrinsic size. As a result, Skia Canvas doesn't let you overwrite the `width` and `height` properties (since it would have no effect anyway) and provides them as read-only values derived from the image data.

:::info[Note]
When loading an image from an SVG file, the intrinsic size may not be defined since the root `<svg>` element is not required to have a defined `width` and `height`. In these cases, the Image will use a default size of 300×100. If the SVG lacks an intrinsic size but *does* contain a `viewBox` attribute, its aspect ratio will be preserved and its size will be scaled to fit within the default 300×100 frame. Note that in *any* case where the default size is used, the image will be re-scaled when passed to [`drawImage`][drawImage()] so that it is contained by the canvas's dimensions (mimicking Chrome's behavior).
:::


### `.complete`

A boolean that is `true` once the `src` data has been fetched and parsed. It does **not** necessarily mean the **Image** is ready to be drawn since the data retrieved may not have been a valid image. In addition you should confirm that the `width` and `height` are non-zero to be sure that loading was successful.

### `.onload` & `.onerror`

For compatibility with browser conventions, event handlers can be set up by assigning functions to the **Image**'s `.onload` and `.onerror` properties. For a more modern-feeling approach, try using [`.on("load", …)`][img_bind] and [`.on("error", …)`][img_bind] instead

The `.onload` function will be passed a reference to the **Image** as its argument, and the `this` of its function context will also refer to the same **Image** object (presuming it is not defined as an arrow function).

The `.onerror` function will be called with a reference to the [Error][js_error] that occurred as its sole argument.

### `.disposed`

The `.disposed` flag identifies when the image's encoded bytes have been freed and it is no longer valid as a drawing source. It will be `false` until the image's [`dispose()`](#dispose) or [`release()`](#release) method is called.


## Methods

### `decode()`

```js returns="Promise<Image>"
img.decode()
```

Since image loading frequently occurs asynchronously, it can be convenient to use the `await` keyword to pause execution of your function until the **Image** is ready to be worked with:
```js
async function main(){
    let img = new Image()
    img.src = 'http://example.com/a-very-large-file.jpg'
    await img.decode()
    // …then do something with `img` now that it's ready
}
```

The browser version of this method returns a Promise that resolves to `undefined` once decoding is complete, but for convenience the Skia Canvas version resolves to a reference to the **Image** object itself:

```js
let img = new Image()
img.src = 'http://example.com/a-very-large-file.jpg'
img.decode().then(({width, height}) =>
    console.log(`dimensions: ${width}×${height}`)
)
```

### `on()` / `off()` / `once()`

```js returns="Image"
on(eventType, handlerFunction)
off(eventType, handlerFunction)
once(eventType, handlerFunction)
```

The **Image** object is an [Event Emitter][event_emitter] subclass and supports all the standard methods for adding and removing event listeners. The event handlers you create will be able to reference the target image through their `this` variable.


### `dispose()`
```js returns="void"
dispose()
```

Synchronously frees all resources associated with the Image and marks it as [`disposed`](#disposed) (preventing it from being used in future drawing operations). This is the method called behind the scenes by the [`using`][using] keyword when the Image reference goes out of scope.

```js
let canvas = new Canvas(150, 150),
    ctx = canvas.getContext('2d')

for (let path of imagePaths){
  let img = new Image(readFileSync(path))
  try{
    ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
    canvas.toFileSync(path.replace(/\.\w+$/, '-thumb.png'))
  }finally{
    img.dispose() // synchronously free the decoded pixels
  }
}
```


### `release()`
```js returns="Promise<void>"
release()
```

Synchronously frees all resources associated with the Image and marks it as [`disposed`](#disposed), then asychronously yields to the event loop (which has the side effect of running any deferred finalizers for *other* objects as well). This is the method called by the [`await using`][await_using] keyword when the Image reference goes out of scope.

```js
for (let path of imagePaths){
  let img = await loadImage(url)
  try{
    ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
    await canvas.toFile(path.replace(/\.\w+$/, '-thumb.png'))
  }finally{
    await img.release() // dispose, then run deferred finalizers
  }
}
```


## Events

The events emitted by the **Image** object both relate to image-loading and can be listened for using the `on()` and `once()` methods.

### `load`

Emitted once data has been retrieved and successfully decoded into one of the supported image file formats. The image will be passed to your callback as the first argument.

### `error`

Emitted if loading was unsuccessful for any reason. An **Error** object with details is passed to your callback as the first argument.


## Helpers

### `loadImage()`

```js returns="Promise<Image>"
loadImage(src)
loadImage(src, {
  page, // PDF-only: specify which (1-based) page to load
  method, headers, body, auth, signal, ... // http request options
})
```

The `loadImage` utility function is included to avoid the fiddly, callback-heavy verbosity of the normal Image-loading dance. It combines image creation, loading, and decoding and gives you a single call to `await` before making use of an image:

```js
import {loadImage} from 'skia-canvas'

let img = await loadImage('some-image-file.png')
```
#### Image Sources

Note that you can pass a wide variety of image sources to the `loadImage` helper: its `src` argument can be any of the types supported by the **Image** class:
- an HTTP URL to asynchronously retrieve the image from
- an absolute or relative path pointing to a file on the local system
- a [Data URL][DataURL] with the image data base64-encoded into the string (or [url-encoded][url_encode] in the case of SVG images)
- a [Buffer][Buffer] containing the pre-loaded bytes of a supported image file format (`png`, `jpg`, `webp`, `svg`, or `pdf`)
- a [Sharp][sharp] bitmap image object (if the library has been [installed separately][sharp_npm])

#### Loading PDFs

When loading a PDF file containing multiple pages, the first page’s content will be used by default. If you'd like to specify a different page number pass a positive integer as the optional `page` argument:

```js
let firstPage = await loadImage("book.pdf") // defaults to page 1
let secondPage = await loadImage("book.pdf", {page:2})
```

#### Loading URLs

When loading a URL over HTTP(S) you may also include a second argument containing options to be used when the library makes its web request behind the scenes.

The supported options include:
- `method`: the http ‘verb’ to use (defaults to `"GET"`)
- `headers`: an object mapping request header names to values
- `body`: a string or buffer to be sent to the server if the method is set to `POST` or `PUT`
- `auth`: a string with credentials for Basic Auth in the format `"user:password"` which will be used to construct the `Authorization` header (if not supplied)
- `timeout`: the amount of socket inactivity to tolerate before cancelling the request (note that this is *not* total request time: it resets on every redirect and whenever a new byte arrives)
- `signal`: an [AbortSignal][AbortSignal] to allow the request to be manually cancelled or [timed out](https://developer.mozilla.org/en-US/docs/Web/API/AbortSignal/timeout_static) after total request time exceeds the threshhold
- `agent`: an [http.Agent][http_agent] used to customize the connection or `false` to disable the [default agent](https://www.npmjs.com/package/https-proxy-agent) which uses the url in the `HTTP_PROXY` environment variable (if defined) as a proxy server
- any other option supported by Node’s [http.request][http_request] call


These options can be useful for accessing resources requiring authentication:

```js
let basicImg = await loadImage('https://example.com/lightly-protected.png', {
  auth: "username:password" // credentials for http-basic-auth
})

let tokenImg = await loadImage('https://example.com/protected.png', {
  headers: {"Authorization": 'Bearer <secret-token-value>'} // token-based auth
})
```

You can also provide additional data for the backend server to use when handling the request:

```js
let img = await loadImage('https://example.com/customized.svg', {
  method: 'POST',
  headers: {
    "Content-Type": 'application/json'
  },
  body: JSON.stringify({
    additionalInfo: "data-used-by-the-backend"
  })
})
```

You can monitor the connection's health in two ways (and potentially catch the promise rejection to retry if it fails):

```js
let img = await loadImage('https://example.com/potentially-slow.jpg', {
  timeout: 2000, // bail if the connection ever goes quiet for >2s…
  signal: AbortSignal.timeout(10000), // …or if it takes >10s total
})
```


<!-- references_begin -->
[loadimage]: #loadimage
[img_bind]: #on--off--once
[img_src]: #src
[img_complete]: #complete
[img_onload]: #onload--onerror
[img_onerror]: #onload--onerror
[img_size]: #width--height
[img_decode]: #decode
[imgdata_new]: imagedata.md#constructor
[canvas_cleanup]: canvas.md#cleaning-up
[event_emitter]: https://nodejs.org/api/events.html#class-eventemitter
[Buffer]: https://nodejs.org/api/buffer.html
[sharp]: https://sharp.pixelplumbing.com
[sharp_npm]: https://www.npmjs.com/package/sharp
[http_agent]: https://nodejs.org/api/http.html#class-httpagent
[http_request]: https://nodejs.org/api/http.html#httprequestoptions-callback
[Promise]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Promise
[DataURL]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/Data_URIs
[img_element]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLImageElement
[drawImage()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/drawImage
[js_error]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error
[url_encode]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/encodeURIComponent
[await_using]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/await_using
[using]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/using
[AbortSignal]: https://developer.mozilla.org/en-US/docs/Web/API/AbortSignal
<!-- references_end -->
