---
description: Direct pixel access to image and canvas contents
---

# ImageData

> The `ImageData` object offers a convenient container that bundled raw pixel data with metadata helpful for working with it. Skia Canvas's implementation of the class mirrors the [standard **ImageData**][ImageData]'s structure and behavior, but extends it in a few ways.

| Dimensions                 | Format                                     | Pixel Data                    |
| --                         | --                                         | --                            |
| [**width**][imgdata_size]  | [**colorSpace**][mdn_ImageData_colorspace] | [**data**][imgdata_data]      |
| [**height**][imgdata_size] | [**colorType**][imgdata_colortype] 🧪      | [toSharp()][imgdata_tosharp] 🧪|
|                            | [**bytesPerPixel**][imgdata_bpp] 🧪        |                               |

## Working with `ImageData` objects

Empty ImageData objects can be created either by calling the context's `createImageData()` method or the `new ImageData()` constructor:

```js
let id = ctx.createImageData(800, 600)
```
or, equivalently:
```js
let id = new ImageData(800, 600)
```

### Choosing a `colorType`

By default ImageData represents bitmaps using an `rgba` ordering of color channels in subsequent bytes of the buffer, but there are quite a few other ways of arranging pixel data to choose from. You can specify one by passing an optional settings object with a `colorType` field when creating the object:

```js
let bgraData = new ImageData(128, 128, {colorType:'bgra'})
```

See below for a list of supported [`colorType` formats][imgdata_colortype].

### Manipulating pixels

Once you've created an ImageData you can access its buffer through its [`data`][imgdata_data] attribute (here using the default `rgba` color type):

```js
// you can read pixel values out…
let firstPixel = id.data.slice(0, 4)
let [r, g, b, a] = firstPixel

// …or write to them, here setting the entire buffer to #F00
for (let i=0; i<id.data.length; i+=id.bytesPerPixel) {
  imageData.data[i + 0] = 255 // red
  imageData.data[i + 1] = 0   // green
  imageData.data[i + 2] = 0   // blue
  imageData.data[i + 3] = 255 // alpha
}
```

### Drawing to the Canvas

According to the standard, the only way to draw ImageData to a canvas is through the [putImageData()][putImageData()] method, which copies either the entire ImageData or a rectangle within it to the canvas, pixel-for-pixel. Since this is *copying* rather than *drawing*, the operation ignores the current context state, including any transformations, filter, or global opacity options that have been set.

Skia Canvas, however, allows you to ImageData and Image objects interchangably when dealing with the canvas. If you pass an ImageData to the [drawImage()][drawImage()] method, its contents will be drawn to the canvas while honoring the context settings that are ignored by `putImageData()`. You may also pass ImageData objects to the [createPattern()][createPattern()] method.


## Constructor

```js returns="ImageData"
// create an empty buffer filled with transparent pixels
new ImageData(width, height)
new ImageData(width, height, {colorType='rgba', colorSpace='srgb'})

// copy an existing buffer into an ImageData container
new ImageData(buffer, width)
new ImageData(buffer, width, height)
new ImageData(buffer, width, height, {colorType='rgba', colorSpace='srgb'})

new ImageData(imageData) // create a copy from another ImageData
new ImageData(image, {colorType, colorSpace}) // decode the pixels from a bitmap Image
```
:::note
The optional `colorSpace` value can currently only be set to `"srgb"` (the default value), but this will hopefully change to include wider gamuts like `"display-p3"` in the future. For now you can simply omit it and use the default sRGB colorspace.
:::

When creating an empty ImageData you must fully specify the dimensions in order to determine the size of the resulting buffer (in conjunction with the `colorType`). In cases where you already have a Buffer object, you only need to provide the `width` so it knows where to ‘wrap’ the linear buffer. When passing an existing ImageData or Image object to the constructor the dimensions are known, but you can specify a non-default `colorType` you'd like to decode to in the Image case.




## Properties

### `.width` & `.height`

The dimensions of the ImageData cannot be changed after it is created. These properties give you read-only access to the 2D dimensions of the image.

### `.colorType`

The `colorType` value describes the layout of bytes within the buffer and how their values map onto the different components of each pixel. The most common formats devote 1 byte to each channel, with each pixel containing 4 channels in various orders. These formats have been given aliases for convenience:

```js
// aliases for the most common 4-byte formats
"rgb"  // for RGB888x
"rgba" // for BGRA8888
"bgra" // for BGRA8888
```

There are many other options with more verbose names (see [this listing][skia_colortype] for descriptions of each):
```js
// 1 byte per pixel
"Alpha8", "Gray8", "R8UNorm"

// 2 bytes per pixel
"A16Float", "A16UNorm", "ARGB4444", "R8G8UNorm", "RGB565"

// 4 bytes per pixel
"RGB888x", "RGBA8888", "BGRA8888", "BGR101010x", "BGRA1010102",
"R16G16Float", "R16G16UNorm", "RGB101010x", "RGBA1010102",
"RGBA8888", "SRGBA8888"

// 8 bytes per pixel
"R16G16B16A16UNorm", "RGBAF16", "RGBAF16Norm"

// 16 bytes per pixel
"RGBAF32"
```

Note that not all of them use 4-byte orderings like `rgba` does, so be sure to use the `.bytesPerPixel` field when stepping through them.

### `.bytesPerPixel`

An integer describing the byte-size of the **ImageData**'s pixels given its `colorType`.

```js
let id = new ImageData(10, 10)
console.log(id.data.length == id.width * id.height * id.bytesPerPixel) // → true
```


### `.data`
A writeable buffer with the pixel contents of the image presented as an [array of 8-bit bytes][u8_array]. See the [standard docs][mdn_ImageData_data] for more details.


## Methods

### `toSharp()`

```js returns="Sharp"
toSharp()
```

:::tip[Optional]
The Sharp library is an optional dependency that you must [install separately][sharp_npm]
:::

The contents of the canvas can be copied into a [Sharp][sharp] image object, allowing you to make use of the extensive image-processing and optimization features offered by the library. See the [`loadImageData()`](#loadimagedata) helper for details on converting the `Sharp` object back into an `ImageData`.



## Helpers
### `loadImageData()`

```js returns="Promise<ImageData>"
loadImageData(src, width)
loadImageData(src, width, height)
loadImageData(src, width, height, {colorType='rgba', …requestOptions})
loadImageData(sharpImage)
```

Similar to the [loadImage()][loadimage] utility, `loadImageData()` will asynchronously fetch a URL or local file path and package it into a usable object for you. In this case, you will need to provide slightly more information about the nature of the data since you will be loading ‘raw’ binary data lacking an internal representation of its dimensions or color type.


#### Loading files
If the file you are loading is stored in `rgba` format, you need only specify the row-width of the image. But if it uses a non-standard color type you'll need to fully specify the dimensions and include a `colorType`:

```js
import {loadImageData} from 'skia-canvas'

let id = await loadImageData('some-image-file.raw', 64, 64, {
  colorType: "bgra"
})
```

#### Loading URLs
If the `src` argument is a URL, you can optionally include any [request options][fetch_opts] supported by [`fetch`][fetch] in the final argument:
```js
let id = await loadImageData('https://skia-canvas.org/customized.raw', 64, 64, {
  colorType: "rgb",
  method: "POST",
  headers: {
    "Content-Type": "application/json"
  },
  body: JSON.stringify({
    additionalInfo: "data-used-by-the-backend"
  })
})
```

#### Loading Data URIs
Note that in addition to HTTP URLs you may also call `loadImageData()` using Data URLs. Just make sure you use the mime type `application/octet-stream` in the header:

```js
await loadImageData('data:application/octet-stream;base64,//8A////AP///...')
```
#### Loading Sharp images
[Sharp][sharp] images can be loaded without any additional arguments since they already contain their dimensions and encoding. The resulting `colorType` will always be converted to `rgba`, even if the Sharp object was initialized with 3-channel RGB:

```js
import sharp from 'sharp'

let sharpImage = sharp({
  create: {width:2, height:2, channels:3, background:"#f00"}
})
await loadImageData(sharpImage)
```





<!-- references_begin -->
[loadimage]: image.md#loadimage
[imgdata_data]: #data
[imgdata_size]: #width--height
[imgdata_colortype]: #colortype
[imgdata_bpp]: #bytesperpixel
[imgdata_tosharp]: #tosharp
[skia_colortype]: https://rust-skia.github.io/doc/skia_safe/enum.ColorType.html
[sharp]: https://sharp.pixelplumbing.com
[sharp_npm]: https://www.npmjs.com/package/sharp
[fetch]: https://developer.mozilla.org/en-US/docs/Web/API/Window/fetch
[fetch_opts]: https://developer.mozilla.org/en-US/docs/Web/API/RequestInit
[ImageData]: https://developer.mozilla.org/en-US/docs/Web/API/ImageData
[mdn_ImageData_data]: https://developer.mozilla.org/en-US/docs/Web/API/ImageData/data
[mdn_ImageData_colorspace]: https://developer.mozilla.org/en-US/docs/Web/API/ImageData/colorSpace
[createPattern()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/createPattern
[drawImage()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/drawImage
[putImageData()]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/putImageData
[u8_array]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Uint8ClampedArray
<!-- references_end -->
