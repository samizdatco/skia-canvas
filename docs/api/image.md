---
description: Bitmap & vector image container
---

# Image

> Skia Canvas's `Image` object is a stripped-down version of the [standard **Image**][img_element] used in browser environments. Since the Canvas API ignores most of its properties, only the relevant ones have been recreated here. Use the asynchronous [`loadImage()`][loadimage] helper function to create an `Image` from any supported data source.

| Content                                        | Loading                      | Event Handlers                                            |
| --                                             | --                           | --                                                        |
| [**src**][img_src]                             | [**complete**][img_complete] | [**onload**][img_onload] / [**onerror**][img_onerror]     |
| [**width**][img_size] / [**height**][img_size] | [decode()][img_decode]       | [on()][img_bind] / [off()][img_bind] / [once()][img_bind] |


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
## Constructor

```js returns="Image"
new Image(buffer)    // a Buffer or ArrayBuffer object
new Image(dataURL)   // a String with a valid `data:` url
new Image(data, src) // optionally include a `src` string
```

While loading images from remote sources is inherently asynchronous, if you've alread fetched a file yourself you can create an Image synchronously by passing its data to the Image constructor. The data can be in any of the *encoded* formats Skia Canvas supports (`png`, `jpeg`, `webp`, or `svg`) but can't be raw pixel data—for that you should use [ImageData][imgdata_new] instead.

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
- Vector: `svg` (but **not** `pdf`, sadly)

Note that the image will be [`complete`][img_complete] immediately if a Buffer or Data URL was used, but otherwise you'll need to [wait for it to load](#loading-image-objects).

### `.width` & `.height`

In the browser these are writable properties that can control the display size of the image within the HTML page. But the context's [`drawImage`][drawImage()] method ignores them in favor of the image's intrinsic size. As a result, Skia Canvas doesn't let you overwrite the `width` and `height` properties (since it would have no effect anyway) and provides them as read-only values derived from the image data.

:::info[Note]
When loading an image from an SVG file, the intrinsic size may not be defined since the root `<svg>` element is not required to have a defined `width` and `height`. In these cases, the Image will be set to a fixed height of `150` and its width will be scaled to a value that matches the aspect ratio of the SVG's `viewbox` size.
:::


### `.complete`

A boolean that is `true` once the `src` data has been fetched and parsed. It does **not** necessarily mean and the **Image** is ready to be drawn since the data retrieved may not have been a valid image. In addition your should confirm that the `width` and `height` are non-zero to be sure that loading was successful.

### `.onload` & `.onerror`

For compatibility with browser conventions, event handlers can be set up by assigning functions to the **Image**'s `.onload` and `.onerror` properties. For a more modern-feeling approach, try using [`.on("load", …)`][img_bind] and [`.on("error", …)`][img_bind] instead

The `.onload` function will be passed a reference to the **Image** as its argument, and the `this` of its function context will also refer to the same **Image** object (presuming it is not defined as an arrow function).

The `.onerror` function will be called with a reference to the [Error][js_error] that occurred as its sole argument.


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
loadImage(src, requestOptions)
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
- a [Buffer][Buffer] containing the pre-loaded bytes of a supported image file format (`png`, `jpg`, `webp`, or `svg`)
- a [Sharp][sharp] bitmap image object (if the library has been [installed separately][sharp_npm])

#### Loading URLs

When loading a URL over HTTP(S) you may also include a second argument containing [request options][fetch_opts] to be used when the library calls [`fetch`][fetch] behind the scenes. This can be useful for accessing resources requiring authentication…

```js
let img = await loadImage('https://example.com/protected.png', {
  headers: {"Authorization": 'Bearer <secret-token-value>'}
})
```

…or for cases where you must provide additional data for the backend server to use when handling the request:

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
[event_emitter]: https://nodejs.org/api/events.html#class-eventemitter
[Buffer]: https://nodejs.org/api/buffer.html
[sharp]: https://sharp.pixelplumbing.com
[sharp_npm]: https://www.npmjs.com/package/sharp
[fetch]: https://developer.mozilla.org/en-US/docs/Web/API/Window/fetch
[fetch_opts]: https://developer.mozilla.org/en-US/docs/Web/API/RequestInit
[Promise]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Promise
[DataURL]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/Data_URIs
[img_element]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLImageElement
[drawImage()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/drawImage
[js_error]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Error
[url_encode]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/encodeURIComponent
<!-- references_end -->
