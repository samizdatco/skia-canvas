---
description: Display a canvas in a window and handle UI events
---
# Window

The `Window` class allows you to open a native OS window and draw within its frame. You can create multiple windows (each with their own event-handling and rendering routines) and update them in response to user input.

Its attributes and methods include:

| Dimensions               | Content                          | Interface            | Mode                         | Methods            |
| --                       | --                               | --                   | --                           | --                 |
| [**left**][win_layout]   | [**background**][win_background] | [**title**][title]   | [**visible**][visible]       | [on()][win_bind] / [once()][win_bind]  |
| [**top**][win_layout]    | [**canvas**][win_canvas]         | [**cursor**][cursor] | [**resizable**][resizable]   | [off()][win_bind]  |
| [**width**][win_layout]  | [**ctx**][win_ctx]               | [**fit**][fit]       | [**fullscreen**][fullscreen] | [close()][close]   |
| [**height**][win_layout] | [**page**][win_page]             |                      |                              |                    |

##  Creating new `Window` objects

When called with no arguments, the `Window` constructor will return a 512 × 512 pt window with a white background and automatically create a `Canvas` of the same size that you can access through its `.canvas` property:

```js
let win = new Window()
console.log(win.canvas)
// Canvas {width:512, height:512, gpu:true, pages:[CanvasRenderingContext2D{}]}
```

You can specify a size (to be shared by the window and canvas) by passing width & height arguments:
```js
let smaller = new Window(256, 128)
````

All of the other window properties can be customized by passing an options object, either in addition to the width & height or all by itself:

```js
let orange = new Window(1024, 768, {background:"orange"})
let titled = new Window({title:"Canvas Window"}) // use default 512×512 size
```

After creating the window, you can modify these properties through simple assignment:

```js
let win = new Window(800, 600, {title="Multi-step Window"})
win.background = "skyblue"
win.top = 40
win.left = 40
```

The object accessible through the window’s `.canvas` attribute is no different than any other `Canvas` you create. You can even create a `Window` after setting up a canvas and tell the window to use it instead of automatically creating one. If you pass it to the constructor without specifying a window size, the window will match the dimensions of the canvas:

```js
let bigCanvas = new Canvas(1024, 1024)
let win = new Window({canvas:bigCanvas})
console.log([win.width, win.height])
// [1024, 1024]
```

Likewise, assigning a new `.canvas` will replace the contents of the window (though it won’t affect the window’s size):

```js
let win = new Window()
win.canvas = new Canvas(1024, 32)

console.log([win.width, win.height])
// [512, 512]
console.log([win.canvas.width, win.canvas.height])
// [1024, 32]
```

> When the window and canvas sizes don’t perfectly match, the canvas will be scaled using the approach selected via the window’s [`fit`][fit] property.

##  Drawing to a Window

To draw to the window’s canvas, you can either use the reference to its `.canvas` property to create a context, or use the shortcut `.ctx` property which skips that step:

```js
let win = new Window({background:"olive", fit:"contain-y"})
console.log(win.ctx === win.canvas.getContext("2d"))
// true

let {canvas, ctx} = win
ctx.fillStyle = 'lightskyblue'
ctx.fillRect(10, 10, canvas.width-20, canvas.height-20)
```

If you create multiple pages in your canvas using [newPage()][newPage], you can select which one is currently displayed by setting the window’s [`.page`][win_page]. By default, the most recently created page will be visible, but if you create a new page after the window is on screen, you’ll need to update the `.page` attribute manually to display it. The window’s `.ctx` shortcut will always point to the context for the currently visible page.

```js
let canvas = new Canvas(32, 32),
    colors = ['orange', 'yellow', 'green', 'skyblue', 'purple']

for (var c of colors){
  ctx = canvas.newPage(canvas.width * 2, canvas.height * 2)
  ctx.fillStyle = c
  ctx.fillRect(0,0, canvas.width, canvas.height)
  ctx.fillStyle = 'white'
  ctx.arc(canvas.width/2, canvas.height/2, 40, 0, 2 * Math.PI)
  ctx.fill()
}

let win = new Window({canvas, page:-2})
win.on('keydown', e => {
  if (e.key=='ArrowLeft') win.page--
  if (e.key=='ArrowRight') win.page++
  console.log(`page ${win.page}/${canvas.pages.length}: ${canvas.width} × ${canvas.height}`)
})
```

##  Responding to Events

Once you've created a `Window` object, Node will wait for your current function to end and then switch over to an OS-controlled event loop for the rest of your program’s runtime. This means it can actively redraw your canvas when you resize the window or update its contents, but also means the Node interpreter will be frozen for the duration.

As a result, you cannot rely upon Node's traditional asynchrononous behavior for structuring your program. In particular, the usual methods for scheduling callbacks like `setTimeout`, `setImmediate`, and `setInterval` **will not work**.

Instead, you must use event handlers attached to the `Window` object. By calling the window’s `.on()`, `.off()`, and `.once()` methods, you can respond to [user interface events][win_bind] like mouse and keyboard input, the window being dragged or resized, a new window becoming active, etc.

Any changes you make in an event handler (whether to the window's canvas or its attributes) will become visible in the next pass through the event loop. For example, you can let the user scribble to the canvas with the mouse and clear it via the escape key with:

```js
let win = new Window(400, 300, {background:'rgba(16, 16, 16, 0.35)'}),
    {canvas, ctx} = win // use the canvas & context created by the window

win.on('mousemove', ({button, x, y}) => {
  if (button == 0){ // a left click
    ctx.fillStyle = `rgb(${Math.floor(255 * Math.random())},0,0)`
    ctx.beginPath()
    ctx.arc(x, y, 10 + 30 * Math.random(), 0, 2 * Math.PI)
    ctx.fill()
  }

  win.cursor = button === 0 ? 'none' : 'crosshair'
})

win.on('keydown', ({key}) => {
  if (key == 'Escape'){
    ctx.clearRect(0, 0, canvas.width, canvas.height)
  }
})
```

In the previous example, we used references to the window’s `ctx` and `canvas` that were created outside the event handler, but this makes the function less general since it's tied to a single window. We can get a reference to the specific window associated with an event through its `.target` attribute, allowing us to write an event handler that doesn't contain a reference to the `win` variable it's attached to:
```js
const closeWindow = (e) => {
  console.log("now closing window:", e.target)
  e.target.close()
}

let win1 = new Window(),
    win2 = new Window();
win1.on('mousedown', closeWindow)
win2.on('mousedown', closeWindow)
```

Alternatively, we could have created our event handler using a `function(e){…}` defintion (rather than an `(e) => {…}` arrow expression) in which case the `this` variable will point to the window:
```js
function closeWindow(e){
  console.log("now closing window:", this)
  this.close()
}
```


##  Events for Animation

In the previous example you may have noticed that the canvas’s contents were preserved in between events and the screen was only being updated in response to user interaction. In general, this is the behavior you want for UI-driven graphics.

But another common case is creating animations in which you redraw the canvas at regular intervals (quite possibly from scratch rather than layering atop the previous contents). In these situations you’ll want to use a set of events that are driven by *timing* rather than interaction:
  - [`setup`][setup] fires once, just before your window is first drawn to the screen
  - [`frame`][frame] fires [60 times per second][fps] and provides a frame counter in its event object
  - [`draw`][draw] fires immediately after `frame` and **clears the canvas** of any window that has event handlers for it


To create a ‘flipbook’ animation (in which the screen is fully redrawn in each pass), your best choice is set up an event handler for the `draw` event. Since `draw` automatically erases the canvas before your code begins to run, you can presume a clean slate each time. The event object passed as an argument to your handler contains a propery called `frame` which will increment by one each time you draw (making it handy for advancing the ‘state’ of your animation):

```js
let win = new Window(300, 300, {background:'red'}),
    {ctx} = win

win.on("draw", e => {
  ctx.strokeStyle = 'white'
  ctx.lineWidth = 60 + 80 * Math.sin(e.frame/20)
  ctx.beginPath()
  ctx.moveTo(100,100)
  ctx.lineTo(200,200)
  ctx.moveTo(100,200)
  ctx.lineTo(200,100)
  ctx.stroke()
})
````

## Properties

###  `.background`
This specifies the color of the window's background which is drawn behind your canvas content. It supports all the same CSS color formats as the `fillStyle` and `strokeStyle` properties. Defaults to white.

###  `.canvas`
The `Canvas` object associated with the window. By default the window will create a canvas with the same size as the window dimensions, but the canvas can also be replaced at any time by assigning a new one to this property.

###  `.ctx`
The rendering context of the window's canvas. This is a shortcut to calling `win.canvas.getContext("2d")`. If the canvas has multiple pages, this will point to the most recent (i.e., the ‘topmost’ page in the stack).

###  `.page`
A 1-based index into the canvas's pages array. If the canvas has multiple pages, this property allows you to select which one to display (potentially allowing for pre-rendering a canvas then animating it as a flip-book). Page `1` is the earliest (or ‘bottommost’) page created. Negative page numbers also work, counting backward from `-1` (the ‘topmost’ page).

###  `.left` / `.top` / `.width` / `.height`
The current location and size of the window as specified in resolution-independent ‘points’. Defaults to a 512 × 512 pt window in the center of the screen. Note that the window and the canvas have independent sizes: the window will scale the canvas's content to fit its current dimensions (using the `fit` property to determine how to deal with differences in aspect ratio).

###  `.title`
The string that is displayed in the window's title bar.

###  `.cursor`
The icon used for the mouse pointer. By default an arrow cursor is used, but other styles can be selected by setting the property to one of the standard [CSS cursor][mdn_cursor] values.

###  `.fit`
When the window is resized, it is likely that it will not perfectly match the aspect ratio of the underlying canvas. This property selects how the layout should adapt—whether it should add margins, allow portions of the canvas to be cropped, or stretch the image to fit. It supports the standard [CSS modes][mdn_object_fit] (`"none"`, `"contain"`, `"cover"`, `"fill"`, and `"scale-down"`) plus some additions:
  - `contain-x` and `contain-y` extend the `contain` mode to choose which axis to use when fitting the canvas
  - `resize` will modify the window's canvas to match the new window size (you'll probably also want to define an `.on("resize")` handler to update the contents)


###  `.visible`
When set to `false`, the window will become invisible but will not be permanently ‘closed’. It can be made visible again by setting the property back to `true`.

###  `.resizable`
When set to `false`, the window’s size will become fixed and the zoom button in the title bar will be disabled. It can be made user-resizable again by setting the property back to `true`. Note that if the window is set to `fullscreen` its dimensions may still change. If you want to prevent that as well be sure to set up a `keydown` event listener that calls the event’s `preventDefault` on **⌘F** and **Alt-F4** presses so the user can’t switch to fullscreen mode.

###  `.fullscreen`
A boolean flag determining whether the window should expand to fill the screen.

---------

## Methods

###  `close()`
Removes the window from the screen permanently. Note that the `Window` object **will** remain valid after it is closed and its `.canvas` can still be used to export images to file, be inserted into other windows, etc.

###  `on()` / `off()` / `once()`
```js returns="Window"
on(eventType, handlerFunction)
off(eventType, handlerFunction)
once(eventType, handlerFunction)
```

The `Window` object is an [Event Emitter][event_emitter] subclass and supports all the standard methods for adding and removing event listeners.

## Events

The events emitted by the `Window` object are mostly consistent with browser-based DOM events, but include some non-standard additions (🧪) specific to Skia Canvas:

| Mouse                        | Keyboard                | Window                               | Focus                | Animation          |
| --                           | --                      | --                                   | --                   | --                 |
| [mousedown][mousedown]       | [keydown][keydown]      |[fullscreen][fullscreen-event] 🧪  | [blur][blur]         | [setup][setup] 🧪|
| [mouseup][mouseup]           | [keyup][keyup]          |[move][move-event] 🧪              | [focus][focus]       | [frame][frame] 🧪|
| [mousemove][mousemove]       | [input][input]          | [resize][resize]                    |                      | [draw][draw] 🧪  |
| [wheel][wheel]               |


### `fullscreen`
Emitted when the a window switches into or out of full-screen mode. The event object includes a boolean `enabled` property flagging the new state.

### `move`
Emitted when the user drags the window to a new position. The event object includes `top` and `left` properties expressed in resolution-independent points.

### `setup`
The `setup` event is emitted just before a newly created window is displayed on screen. This can be a good place to collect the data you'll need for an animation. Immediately after `setup`, the `frame` and `draw` events will fire.

### `frame`
Similar to the `requestAnimationFrame` callback system in browsers, the `frame` event allows you to schedule redrawing your canvas to maintain a constant frame rate. The event object provides a window-specific frame counter that begins ticking upward from zero as soon as the window appears.

### `draw`
The `draw` event fires immediately after `frame` and has the potentially convenient side effect of automatically erasing the window's canvas before calling your event handler.

> Note that this canvas-clearing behavior depends upon your having set up an event handler using `.on("draw", …)` and will continue until (and unless) you delete the window's `draw` event handlers using `.off()` or [`removeAllListeners()`][remove_all].

<!-- references_begin -->
[close]: #close
[cursor]: #cursor
[draw]: #draw
[fit]: #fit
[fps]: app.md#fps
[frame]: #frame
[fullscreen]: #fullscreen
[fullscreen-event]: #fullscreen-1
[move-event]: #move
[newPage]: canvas.md#newpage
[resizable]: #resizable
[setup]: #setup
[title]: #title
[visible]: #visible
[win_background]: #background
[win_bind]: #on--off--once
[win_canvas]: #canvas
[win_ctx]: #ctx
[win_layout]: #left--top--width--height
[win_page]: #page
[event_emitter]: https://nodejs.org/api/events.html#class-eventemitter
[remove_all]: https://nodejs.org/api/events.html#emitterremovealllistenerseventname
[mdn_cursor]: https://developer.mozilla.org/en-US/docs/Web/CSS/cursor
[mdn_object_fit]: https://developer.mozilla.org/en-US/docs/Web/CSS/object-fit
[mousedown]: https://developer.mozilla.org/en-US/docs/Web/API/Element/mousedown_event
[mouseup]: https://developer.mozilla.org/en-US/docs/Web/API/Element/mouseup_event
[mousemove]: https://developer.mozilla.org/en-US/docs/Web/API/Element/mousemove_event
[wheel]: https://developer.mozilla.org/en-US/docs/Web/API/Element/wheel_event
[keydown]: https://developer.mozilla.org/en-US/docs/Web/API/Element/keydown_event
[keyup]: https://developer.mozilla.org/en-US/docs/Web/API/Element/keyup_event
[input]: https://developer.mozilla.org/en-US/docs/Web/API/HTMLElement/input_event
[resize]: https://developer.mozilla.org/en-US/docs/Web/API/Window/resize_event
[focus]: https://developer.mozilla.org/en-US/docs/Web/API/Window/focus_event
[blur]: https://developer.mozilla.org/en-US/docs/Web/API/Window/blur_event
<!-- references_end -->
