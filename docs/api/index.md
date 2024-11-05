---
id: api-intro
---
# API Documentation

:::info[Note]
Documentation for the key classes and their attributes are listed below—properties are printed in **bold** and methods have parentheses attached to the name. The instances where Skia Canvas’s behavior goes beyond the standard are marked by a 🧪 symbol, linking to further details below. Links to documentation to the web standards Skia Canvas emulates are marked with a 📖.
:::

The library exports a number of classes emulating familiar browser objects including:

 - [Canvas][mdn_canvas] ⧸ [extensions][canvas] 🧪
 - [CanvasGradient][CanvasGradient]
 - [CanvasPattern][CanvasPattern]
 - [CanvasRenderingContext2D][CanvasRenderingContext2D] ⧸ [extensions][context] 🧪
 - [DOMMatrix][DOMMatrix]
 - [Image][Image]
 - [ImageData][ImageData]
 - [Path2D][p2d_mdn] ⧸ [extensions][path2d] 🧪

In addition, the module contains:

-[FontLibrary][fontlibrary] a global object for inspecting the system’s fonts and loading additional ones
-[Window][window] a class allowing you to display your canvas interactively in an on-screen window
-[App][app] a helper class for coordinating multiple windows in a single script
-[loadImage()][loadimage] a utility function for loading `Image` objects asynchronously

----

For detailed notes on the extensions Skia Canvas has made to standard object types, see the corresponding pages:

import DocCardList from '@theme/DocCardList';

<DocCardList />

<!-- references_begin -->
[app]: app.md#app
[canvas]: canvas.md
[context]: context.md
[fontlibrary]: font-library.md
[loadimage]: utilities.md#loadimage
[path2d]: path2d.md
[window]: window.md
[p2d_mdn]: https://developer.mozilla.org/en-US/docs/Web/API/Path2D
[mdn_canvas]: https://developer.mozilla.org/en-US/docs/Web/API/Canvas
[CanvasGradient]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasGradient
[CanvasPattern]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasPattern
[CanvasRenderingContext2D]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D
[DOMMatrix]: https://developer.mozilla.org/en-US/docs/Web/API/DOMMatrix
[Image]: https://developer.mozilla.org/en-US/docs/Web/API/Image
[ImageData]: https://developer.mozilla.org/en-US/docs/Web/API/ImageData
<!-- references_end -->
