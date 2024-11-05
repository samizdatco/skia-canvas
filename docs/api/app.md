---
description: Global window manager and process controller
---

# App

The `App` global variable is a static class which does not need to be instantiated with `new`. Instead you can directly access its properties and methods on the `App` you import from the module. It allows you to access all the windows that are currently on screen, choose a frame rate for the `frame` and `draw` events, and control when the GUI event loop begins and terminates.

### Properties

####  `.fps`
By default, each window will attempt to update its display 60 times per second. You can reduce this by setting `App.fps` to a smaller integer value. You can raise it as well but on the majority of LCD monitors you won't see any benefit and are likely to get worse performance as you begin to swamp the CPU with your rendering code.
> This setting is only relevant if you are listening for `frame` or `draw` events on your windows. Otherwise the canvas will only be updated when responding to UI interactions like keyboard and mouse events.

####  `.running`
A read-only boolean flagging whether the GUI event loop has taken control away from Node in order to display your windows.

####  `.windows`
An array of references to all of the `Window` objects that have been created and not yet [closed][close].

### Methods

####  `launch()`
Any `Window` you create will schedule the `App` to begin running as soon as the current function returns. You can make this happen sooner by calling `App.launch` within your code. The `launch()` method will not return until the last window is closed so you may find it handy to place ‘clean up’ code after the `launch()` invocation.
>Note, however, that the `App` **cannot be launched a second time** once it terminates due to limitiations in the underlying platform libraries.

####  `quit()`
By default your process will terminate once the final window has closed. If you wish to bring things to a swifter conclusion from code, call the `App.quit()` method from one of your event handlers instead.

<!-- references_begin -->
[close]: window.md#close
<!-- references_end -->
