---
description: Image loading helper
---
# Utilities

## `loadImage()`

The included [Image][Image] object behaves just like the one in browsers, which is to say that loading images can be verbose, fiddly, and callback-heavy. The `loadImage()` utility method wraps image loading in a [Promise][Promise], allowing for more concise initialization. For instance the following snippets are equivalent:

```js
// this is a comment string
let img = new Image()
img.onload = function(){
  ctx.drawImage(img, 100, 100)
}
img.src = 'https://example.com/icon.png'
```

```js
let img = new Image()
img.src = 'https://example.com/icon.png'
await img.decode()
ctx.drawImage(img, 100, 100)
```

```js
let img = await loadImage('https://example.com/icon.png')
ctx.drawImage(img, 100, 100)
```

In addition to HTTP URLs, both `loadImage()` and the `Image.src` attribute will also accept [data URLs][DataURL], local file paths, and [Buffer][Buffer] objects.

<!-- references_begin -->
[Buffer]: https://nodejs.org/api/buffer.html
[Promise]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Promise
[DataURL]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/Data_URIs
[Image]: https://developer.mozilla.org/en-US/docs/Web/API/Image
<!-- references_end -->
