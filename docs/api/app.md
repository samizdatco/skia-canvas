---
description: Global window manager and process controller
---

# App

> The `App` global variable is a static class which does not need to be instantiated with `new`. It allows you to access all the windows that are currently on screen, choose a frame rate for the `frame` and `draw` events, and control when the GUI event loop begins and terminates.

| App Lifecycle       | Runtime State           | Individual Windows      |
| --                  | --                      | --                      |
| [launch()](#launch) | [**running**](#running) | [**windows**](#windows) |
| [quit()](#quit)     | [**fps**](#fps)         |                         |

### Properties

####  ~~`.eventLoop`~~

**The eventLoop property has been deprecated** and setting it no longer has an effect. Node's event loop now continues to operate even while a GUI window is active and the OS is handling rendering and UI events. As a result, [the `launch()`][launch] method now runs asynchronously and returns a `Promise` that resolves when the last window is closed. In the meantime, Node-based events like timers and intervals will fire normally.

Keep in mind though that you'll still be better off using the [`frame`][frame] or [`draw`][draw] event for timing rather than setting up a timeout- or interval-based rendering callback.

####  `.fps`
By default, each window will attempt to re-render its content every time the display refreshes (typically 60 times per second). You can reduce this by setting `App.fps` to a smaller integer value. Raising it above the display's native refresh rate will have no effect.
> This setting is only relevant if you are listening for [`frame`][frame] or [`draw`][draw] events (or have a pending `requestAnimationFrame`) on your windows. Otherwise the canvas will only be updated when responding to UI interactions like keyboard and mouse events.

####  `.running`
A read-only boolean flagging whether any GUI windows are currently active. It is `true` while the Promise returned by [`launch()`][launch] is still pending.  

####  `.windows`
An array of references to all of the `Window` objects that have been created and not yet [closed][close].

### Methods

####  `launch()`

```js returns="Promise"
App.launch()
```
Any `Window` you create will schedule the `App` to begin running as soon as the current function returns. You can make this happen sooner by calling `App.launch()` within your code. The `launch()` method is asynchronous and returns a `Promise` that resolves when the last window is closed so you may find it handy to place ‘clean up’ code in a `.then()` callback or after `await`ing the `launch()` invocation.

####  `quit()`
```js
App.quit()
```

By default your process will terminate once the final window has closed (and any timers or intervals you've set up have been cleared). If you wish to bring things to a swifter conclusion from code, call the `App.quit()` method from one of your event handlers instead.

### Events

#### `idle`

Emitted when the final active window is closed via a user interface click on its close widget or by a programmatic call to the window's [`close()`][close] method.

<!-- references_begin -->
[close]: window.md#close
[draw]: window.md#draw
[frame]: window.md#frame
[launch]: #launch
<!-- references_end -->
