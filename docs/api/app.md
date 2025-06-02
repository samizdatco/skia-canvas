---
description: Global window manager and process controller
---

# App

> The `App` global variable is a static class which does not need to be instantiated with `new`. It allows you to access all the windows that are currently on screen, choose a frame rate for the `frame` and `draw` events, and control when the GUI event loop begins and terminates.

| App Lifecycle               | Runtime State           | Individual Windows      |
| --                          | --                      | --                      |
| [**eventLoop**](#eventLoop) | [**running**](#running) | [**windows**](#windows) |
| [launch()](#launch)         | [**fps**](#fps)         |                         |
| [quit()](#quit)             |

### Properties

####  `.eventLoop`

When displaying GUI windows, the OS typically runs its own event loop to collect user input and orchestrate view updates. Node also runs its own event loop in order to deal with asynchronous events like `fetch`, `setTimeout`, and `setInterval` callbacks. This creates a conflict over who should ‘own’ the event loop: Node or the OS.

The `.eventLoop` property allows you to select between these two modes, each with different trade-offs. The proprerty can be set to:
  - `"native"` (the default) in which case the Node event loop is suspended while the OS handles displaying GUI windows. This is optimal in terms of rendering performance but means that [`launch()`][launch] will block async events until the last window has been closed (though note that [GUI event][win_events] handlers will still be triggered in the meantime).
  - `"node"` where the Node event loop maintains control and manually polls for GUI events every few milliseconds. In this case [`launch()`][launch] will run asynchronously and return a `Promise` that resolves when the last window is closed. In the meantime, Node-based events like timers and intervals will fire normally. Note that there are some platform-specific [caveats][winit_caveats] to be aware of that are associated with the Winit feature that allows for this mode.

When in doubt, stick with `"native"` mode unless you're sure you need to be running async code during your render loop. In particular, you'll be better off using the [`frame`][frame] or [`draw`][draw] event for timing rather than setting up a timeout- or interval-based rendering callback.

####  `.fps`
By default, each window will attempt to update its display 60 times per second. You can reduce this by setting `App.fps` to a smaller integer value. You can raise it as well but on the majority of LCD monitors you won't see any benefit and are likely to get worse performance as you begin to swamp the CPU with your rendering code.
> This setting is only relevant if you are listening for [`frame`][frame] or [`draw`][draw] events on your windows. Otherwise the canvas will only be updated when responding to UI interactions like keyboard and mouse events.

####  `.running`
A read-only boolean flagging whether the GUI event loop has begun running (after which the `.eventLoop` can no longer be modified).

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
[win_events]: window.md#events
[winit_caveats]: https://docs.rs/winit/0.30.11/winit/platform/pump_events/trait.EventLoopExtPumpEvents.html#platform-specific
<!-- references_end -->
