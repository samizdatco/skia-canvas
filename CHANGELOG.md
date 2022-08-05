# Changelog

<!-- ## 🥚 ⟩ [Unreleased] -->

## 📦 ⟩ [v1.0.0] ⟩ Aug 5, 2022

### New Features
- The new [Window][window] class can display a **Canvas** on screen, respond to mouse and keyboard input, and fluidly [animate][window_anim] by calling user-defined [event handlers][window_events].
- Bitmap rendering now occurs on the GPU by default and can be configured using the **Canvas**'s [`.gpu`][canvas_gpu] property. If the platform supports hardware-accelerated rendering (using Metal on macOS and Vulkan on Linux & Windows), the property will be `true` by default and can be set to `false` to use the software renderer.
- Added support for recent Chrome features:
  - the [`reset()`][chrome_reset] context method which erases the canvas, resets the transformation state, and clears the current path
  - the [`roundRect()`][chrome_rrect] method on contexts and **Path2D** objects which adds a rounded rectangle using 1–4 corner radii (provided as a single value or an array of numbers and/or **DOMPoint** objects)

### Bugfixes
- The `FontLibrary.reset()` method didn't actually remove previously installed fonts that had already been drawn with (and thus cached). It now clears those caches, which also means previously used fonts can now be replaced by calling `.use()` again with the same family name.
- The [`.drawCanvas()`][drawCanvas] routine now applies filter effects and shadows consistent with the current resolution and transformation state.

### Misc. Improvements
- The [`.filter`][filter] property's `"blur(…)"` and `"drop-shadow(…)"` effects now match browser behavior much more closely and scale appropriately with the `density` export option.
- Antialiasing is smoother, particularly when down-scaling images, thanks to the use of mipmaps rather than Skia's (apparently buggy?) implementation of bucubic interpolation.
- Calling `clearRect()` with dimensions that fully enclose the canvas will now discard all the vector objects that have been drawn so far (rather than simply covering them up).
- Upgraded Skia to milestone 103

[window]: https://github.com/samizdatco/skia-canvas#window
[window_anim]: https://github.com/samizdatco/skia-canvas#events--animation
[window_events]: https://github.com/samizdatco/skia-canvas#on--off--once
[canvas_gpu]: https://github.com/samizdatco/skia-canvas#gpu
[filter]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/filter
[chrome_reset]: https://developer.chrome.com/blog/canvas2d/#context-reset
[chrome_rrect]: https://developer.chrome.com/blog/canvas2d/#round-rect

## 📦 ⟩ [v0.9.30] ⟩ Jun 7, 2022

### New Features
- Enhacements to the shared **FontLibrary** object:
  - Added a [`reset()`][FontLibrary.reset] method to FontLibrary which uninstalls any fonts that had been dynamically installed via `FontLibrary.use()`
  - The [`use()`][FontLibrary.use] method now checks for previously installed fonts with the same family name (or alias) and will replace them with the newly added font
- Added pre-compiled binaries for Alpine Linux on arm64

### Bugfixes
- Calling `clip` with an empty path (or one that does not intersect the current clipping mask) will now prevent drawing altogether
- Transformation (`translate`, `rotate`, etc.) and line-drawing methods (`moveTo`, `lineTo`, `ellipse`, etc.) are now silently ignored if called with `NaN`, `Infinity`, or non-**Number** values in the arguments rather than throwing an error
  - applies to both the Context and Path2D versions of the drawing methods
  - a **TypeError** is thrown only if the number of arguments is too low (mirroring browser behavior)
- [`conicCurveTo()`][conicCurveTo] now correctly reflects the canvas's transform state
- The browser-based version of [`loadImage()`][loadImage] now returns a **Promise** that correctly resolves to an **Image** object
- SVG exports no longer have an invisible, canvas-sized `<rect/>` as their first element
- Fixed an incompatibility on Alpine between the version of libstdc++ present on the `node:alpine` docker images and the version used when building the precompiled binaries

### Misc. Improvements
- Upgraded Skia to milestone 101

[conicCurveTo]: https://github.com/samizdatco/skia-canvas#coniccurvetocpx-cpy-x-y-weight
[FontLibrary.reset]: https://github.com/samizdatco/skia-canvas#reset
[FontLibrary.use]: https://github.com/samizdatco/skia-canvas#usefamilyname-fontpaths
[loadImage]: https://github.com/samizdatco/skia-canvas/#loadimage

## 📦 ⟩ [v0.9.29] ⟩ Feb 7, 2022

### New Features
- PDF exports now support the optional [`matte`][matte] argument.

### Breaking Changes
- When the [`drawImage()`][mdn_drawImage] function is passed a **Canvas** object as its image source it will now rasterize the canvas before drawing. The prior behavior (in which it is drawn as a vector graphic) can now be accessed through the new [`drawCanvas()`][drawCanvas] method which supports the same numerical arguments as `drawImage` but requires that its first argument be a **Canvas**.

### Bugfixes
- Regions erased using [`clearRect()`][mdn_clearRect] are now properly antialiased
- The [`clip()`][mdn_clip] method now interprets the current translate/scale/rotate state correctly when combining clipping masks

### Misc. Improvements
- Upgraded Skia to milestone 97

[drawCanvas]: https://github.com/samizdatco/skia-canvas#drawcanvascanvas-x-y-
[mdn_clip]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/clip
[mdn_clearRect]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/clearRect

## 📦 ⟩ [v0.9.28] ⟩ Jan 12, 2022

### New Features
- Added TypeScript definitions for extensions to the DOM spec (contributed by [@cprecioso](https://github.com/cprecioso))
- Added 3D-perspective transformations via the new [createProjection()](https://github.com/samizdatco/skia-canvas#createprojectionquad-basis) context method
- Colors can now use the [hwb()](https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/hwb()) model

### Breaking Changes
- The **Canvas** [`.async`](https://github.com/samizdatco/skia-canvas#async) property has been **deprecated** and will be removed in a future release.
  - The `saveAs`, `toBuffer`, and `toDataURL` methods will now be async-only (likewise the [shorthand properties](https://github.com/samizdatco/skia-canvas#pdf-svg-jpg-and-png)).
  - Use their synchronous counterparts (`saveAsSync`, `toBufferSync`, and `toDataURLSync`) if you want to block execution while exporting images.
- The [ImageData](https://developer.mozilla.org/en-US/docs/Web/API/ImageData/ImageData) constructor now orders its arguments properly: the optional buffer/array argument now comes first

### Bugfixes
- Fixed a stack overflow that was occurring when images became too deeply nested for the default deallocator to handle (primarily due to many thousands of image exports from the same canvas)
- The `source-in`, `source-out`, `destination-atop`, and `copy` composite operations now work correctly for paths rather than rendering shapes without color (contributed by [@meihuanyu](https://github.com/meihuanyu))
- Shape primitives now behave consistently with browsers when being added to a non-empty path:
  - `rect()` now issues an initial `moveTo` rather than extending the path, then leaves the ‘current’ point in its upper left corner
  - `ellipse()` extends the current path rather than implicitly closing it (contributed by [@meihuanyu](https://github.com/meihuanyu))
  - `arc()` also extends the current path rather than closing it

### Misc. Improvements
- Upgraded Skia to milestone 96
- Added workflow for creating docker build environments


## 📦 ⟩ [v0.9.27] ⟩ Oct 23, 2021

### New Features
- Added pre-compiled binaries for Alpine Linux using the [musl](https://musl.libc.org) C library


## 📦 ⟩ [v0.9.26] ⟩ Oct 18, 2021

### New Features
- Added pre-compiled binaries for 32-bit and 64-bit ARM on Linux (a.k.a. Raspberry Pi)

### Bugfixes
- Windows text rendering has been restored after failing due to changes involving the `icudtl.dat` file
- `FontLibrary.use` now reports an error if the specified font file doesn't exist
- Fixed a crash that could result from calling `measureText` with various unicode escapes

### Misc. Improvements
- Upgraded Skia to milestone 94
- Now embedding a more recent version of the FreeType library on Linux with support for more font formats


## 📦 ⟩ [v0.9.25] ⟩ Aug 22, 2021

### Bugfixes
- Improved image scaling when a larger image is being shrunk down to a smaller size via [`drawImage()`][mdn_drawImage]
- modified [`imageSmoothingQuality`](https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/imageSmoothingQuality) settings to provide a more meaningful range across `low`, `medium`, and `high`
- [`measureText()`](https://github.com/samizdatco/skia-canvas#measuretextstr-width) now returns correct metrics regardless of current `textAlign` setting
- Rolled back `icudtl.dat` changes on Windows (which suppressed the misleading warning message but required running as Administrator)

### Misc. Improvements
- Now using [Neon](https://github.com/neon-bindings/neon) v0.9 (with enhanced async event scheduling)

[mdn_drawImage]: https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/drawImage

## 📦 ⟩ [v0.9.24] ⟩ Aug 18, 2021

### New Features
- **Path2D** objects now have a read/write [`d`](https://github.com/samizdatco/skia-canvas/#d) property with an [SVG representation](https://developer.mozilla.org/en-US/docs/Web/SVG/Attribute/d#path_commands) of the path’s contours and an [`unwind()`](https://github.com/samizdatco/skia-canvas/#unwind) method for converting from even-odd to non-zero winding rules
- The [`createTexture()`](https://github.com/samizdatco/skia-canvas#createtexturespacing-path-line-color-angle-offset0) context method returns **CanvasTexture** objects which can be assigned to `fillStyle` or `strokeStyle`
- Textures draw either a parallel-lines pattern or one derived from the provided **Path2D** object and positioning parameters
- The marker used when `setLineDash` is active can now be customized by assigning a **Path2D** to the context’s [`lineDashMarker`](https://github.com/samizdatco/skia-canvas#linedashmarker) property (default dashing can be restored by assigning `null`)
- The marker’s orientation & shape relative to the path being stroked can be controlled by the [`lineDashFit`](https://github.com/samizdatco/skia-canvas#linedashfit) property which defaults to `"turn"` but can be set to `"move"` (which preserves orientation) or `"follow"` (which distorts the marker’s shape to match the contour)

### Bugfixes

- Removed use of the `??` operator which is unavailable prior to Node 14
- Prevented a spurious warning on windows incorrectly claiming that the `icudtl.dat` file could not be found

### Misc. Improvements

- The **Path2D** [`simplify()`](https://github.com/samizdatco/skia-canvas/#simplifyrulenonzero) method now takes an optional fill-rule argument
- Added support for versions of macOS starting with 10.13 (High Sierra)


## 📦 ⟩ [v0.9.23] ⟩ Jul 12, 2021

### New Features

- [Conic béziers][conic_bezier] can now be drawn to the context or a Path2D with the [`conicCurveTo()`][conic_curveto] method
- Text can be converted to a Path2D using the context’s new [`outlineText()`][outline_text] method
- Path2D objects can now report back on their internal geometry with:
    - the [`edges`][edges] property which contains an array of line-drawing commands describing the path’s individual contours
    - the [`contains()`][contains] method which tests whether a given point is on/within the path
    - the [`points()`][points] method which returns an array of `[x, y]` pairs at the requested spacing along the curve’s periphery
- A modified copy of a source Path2D can now be created using:
    - [`offset()`][offset] or [`transform()`][transform] to shift position or apply a DOMMatrix respectively
    - [`jitter()`][jitter] to break the path into smaller sections and apply random noise to the segments’ positions
    - [`round()`][round] to round off every sharp corner in a path to a particular radius
    - [`trim()`][trim] to select a percentage-based subsection of the path
- Two similar paths can be ‘tweened’ into a proportional combination of their coordinates using the [`interpolate()`][interpolate] method

### Bugfixes

- Passing a Path2D argument to the `fill()` or `stroke()` method no longer disturbs the context’s ‘current’ path (if one has been created using `beginPath()`)
- The `filter` property will now accept percentage values greater than 999%

### Misc. Improvements

- The `newPage()` and `saveAs()` methods now work in the browser, including the ability to save image sequences to a zip archive. The browser’s canvas is still doing all the drawing however, so file export formats will be limited to PNG and JPEG and none of the other Skia-specific extensions will be available.
- The file-export methods now accept a [`matte`][matte] value in their options object which can be used to set the background color for any portions of the canvas that were left semi-transparent
- Canvas dimensions are no longer rounded-off to integer values (at least until a bitmap needs to be generated for export)
- Linux builds will now run on some older systems going back to glibc 2.24

[conic_bezier]: https://docs.microsoft.com/en-us/xamarin/xamarin-forms/user-interface/graphics/skiasharp/curves/beziers#the-conic-bézier-curve
[conic_curveto]: https://github.com/samizdatco/skia-canvas#coniccurvetocpx-cpy-x-y-weight
[outline_text]: https://github.com/samizdatco/skia-canvas#outlinetextstr
[matte]: https://github.com/samizdatco/skia-canvas#matte

[edges]: https://github.com/samizdatco/skia-canvas#edges
[contains]: https://github.com/samizdatco/skia-canvas#containsx-y
[points]: https://github.com/samizdatco/skia-canvas#pointsstep1
[offset]: https://github.com/samizdatco/skia-canvas#offsetdx-dy
[transform]: https://github.com/samizdatco/skia-canvas#transformmatrix-or-transforma-b-c-d-e-f

[interpolate]: https://github.com/samizdatco/skia-canvas#interpolateotherpath-weight
[jitter]: https://github.com/samizdatco/skia-canvas#jittersegmentlength-amount-seed0
[round]: https://github.com/samizdatco/skia-canvas#roundradius
[simplify]: https://github.com/samizdatco/skia-canvas#simplify
[trim]: https://github.com/samizdatco/skia-canvas#trimstart-end-inverted


## 📦 ⟩ [v0.9.22] ⟩ Jun 09, 2021

### New Features

- Rasterization and file i/o are now handled asynchronously in a background thread. See the discussion of Canvas’s new [`async`](https://github.com/samizdatco/skia-canvas#async) property for details.
- Output files can now be generated at pixel-ratios > 1 for High-DPI screens. `SaveAs` and the other canvas output functions all accept an optional [`density`](https://github.com/samizdatco/skia-canvas#density) argument which is an integer ≥1 and will upscale the image accordingly. The density can also be passed using the `filename` argument by ending the name with an ‘@’ suffix like `some-image@2x.png`.
- SVG exports can optionally convert text to paths by setting the [`outline`](https://github.com/samizdatco/skia-canvas#outline) argument to `true`.

### Breaking Changes

- The canvas functions dealing with rasterization (`toBuffer`, `toDataURL`, `png`, `jpg`, `pdf`, and `svg`) and file i/o (`saveAs`) are now asynchronous and return `Promise` objects. The old, synchronous behavior is still available on a canvas-by-canvas basis by setting its `async` property to `false`.
- The optional `quality` argument accepted by the output methods is now a float in the range 0–1 rather than an integer from 0–100. This is consistent with the [encoderOptions](https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/toDataURL) arg in the spec. Quality now defaults to 0.92 (again, as per the spec) rather than lossless.

### Bugfixes

- `measureText` was reporting zero when asked to measure a string that was entirely made of whitespace. This is still the case for ‘blank‘ lines when `textWrap` is set to `true` but in the default, single-line mode the metrics will now report the width of the whitespace.
-  Changed the way text rendering was staged so that SVG exports didn’t *entirely omit(!)* text from their output. As a result, `Context2D`s now use an external `Typesetter` struct to manage layout and rendering.


## 📦 ⟩ [v0.9.21] ⟩ May 22, 2021

### New Features
  - Now runs on Windows and Apple Silicon Macs.
  - Precompiled binaries support Node 10, 12, 14+.
  - Image objects can be initialized from PNG, JPEG, GIF, BMP, or ICO data.
  - Path2D objects can now be combined using [boolean operators](https://github.com/samizdatco/skia-canvas/#complement-difference-intersect-union-and-xor) and can measure their own [bounding boxes](https://github.com/samizdatco/skia-canvas/#bounds).
  - Context objects now support [`createConicGradient()`](https://developer.mozilla.org/en-US/docs/Web/API/CanvasRenderingContext2D/createConicGradient).
  - Image objects now return a promise from their [`decode()`](https://developer.mozilla.org/en-US/docs/Web/API/HTMLImageElement/decode) method allowing for async loading without the [`loadImage`](https://github.com/samizdatco/skia-canvas/#loadimage) helper.

### Bugfixes
  - Calling `drawImage` with a `Canvas` object as the argument now uses a Skia `Pict` rather than a  `Drawable` as the interchange format, meaning it can actually respect the canvas's current `globalAlpha` and `globalCompositeOperation` state (fixed #6).
  - Improved some spurious error messages when trying to generate a graphics file from a canvas whose width and/or height was set to zero (fixed #5).
  - `CanvasPattern`s now respect the `imageSmoothingEnabled` setting
  - The `counterclockwise` arg to `ellipse` and `arc` is now correctly treated as optional.

### Misc. Improvements
  - Made the `console.log` representations of the canvas-related objects friendlier.
  - Added new test suites for `Path2D`, `Image`, and `Canvas`’s format support.
  - Created [workflows](https://github.com/samizdatco/skia-canvas/tree/master/.github/workflows) to automate precompiled binary builds, testing, and npm package updating.


## 📦 ⟩ [v0.9.20] ⟩ Mar 27, 2021

### Bugfixes
  - The `loadImage` helper can now handle `Buffer` arguments

### Misc. Improvements
  - Improved documentation of compilation steps and use of line height with `ctx.font`


## 📦 ⟩ [v0.9.19] ⟩ Aug 30, 2020

**Initial public release** 🎉

[unreleased]: https://github.com/samizdatco/skia-canvas/compare/v1.0.0...HEAD
[v1.0.0]: https://github.com/samizdatco/skia-canvas/compare/v0.9.30...v1.0.0
[v0.9.30]: https://github.com/samizdatco/skia-canvas/compare/v0.9.29...v0.9.30
[v0.9.29]: https://github.com/samizdatco/skia-canvas/compare/v0.9.28...v0.9.29
[v0.9.28]: https://github.com/samizdatco/skia-canvas/compare/v0.9.27...v0.9.28
[v0.9.27]: https://github.com/samizdatco/skia-canvas/compare/v0.9.26...v0.9.27
[v0.9.26]: https://github.com/samizdatco/skia-canvas/compare/v0.9.25...v0.9.26
[v0.9.25]: https://github.com/samizdatco/skia-canvas/compare/v0.9.24...v0.9.25
[v0.9.24]: https://github.com/samizdatco/skia-canvas/compare/v0.9.23...v0.9.24
[v0.9.23]: https://github.com/samizdatco/skia-canvas/compare/v0.9.22...v0.9.23
[v0.9.22]: https://github.com/samizdatco/skia-canvas/compare/v0.9.21...v0.9.22
[v0.9.21]: https://github.com/samizdatco/skia-canvas/compare/v0.9.20...v0.9.21
[v0.9.20]: https://github.com/samizdatco/skia-canvas/compare/v0.9.19...v0.9.20
[v0.9.19]: https://github.com/samizdatco/skia-canvas/compare/v0.9.15...v0.9.19
