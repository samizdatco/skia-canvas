---
description: Inspect your system fonts or load new ones
---
# FontLibrary

> The `FontLibrary` global variable is a static class which does not need to be instantiated with `new`. Instead you can access the properties and methods on the global `FontLibrary` you import from the module and its contents will be shared across all canvases you create.

| Installed Fonts           | Loading New Fonts | Typeface Details  |
| --                        | --                | --                  |
| [**families**](#families) | [use()](#use)     | [family()](#family) |
| [has()](#has)             | [reset()](#reset) |                     |



## Properties

### `.families`

The `.families` property contains a list of family names, merging together all the fonts installed on the system and any fonts that have been added manually through the `FontLibrary.use()` method. Any of these names can be passed to `FontLibrary.family()` for more information.


## Methods

### `family()`
```js returns="{family, weights, widths, styles}"
FontLibrary.family(name)
```

If the `name` argument is the name of a known font family, this method will return an object with information about the available weights and styles. For instance, on my system `FontLibrary.family("Avenir Next")` returns:
```js
{
  family: 'Avenir Next',
  weights: [ 100, 400, 500, 600, 700, 800 ],
  widths: [ 'normal' ],
  styles: [ 'normal', 'italic' ]
}
```

Asking for details about an unknown family will return `undefined`.

### `has()`
```js
FontLibrary.has(familyName)
```

Returns `true` if the family is installed on the system or has been added via `FontLibrary.use()`.

### `reset()`

Uninstalls any dynamically loaded fonts that had been added via `FontLibrary.use()`.

### `use()`
```js returns="{family, weight, style, width, file}[]"
FontLibrary.use([...fontPaths])
FontLibrary.use(familyName, [...fontPaths])
FontLibrary.use({familyName:[...fontPaths], ...)
```

The `FontLibrary.use()` method allows you to dynamically load local font files and use them with your canvases. It can read fonts in the OpenType (`.otf`), TrueType (`.ttf`), and web-font (`.woff` & `.woff2`) file formats.


By default the family name will be take from the font metadata, but this can be overridden by an alias you provide. Since font-wrangling can be messy, `use` can be called in a number of different ways:

#### with a list of file paths
```js
import {FontLibrary} from 'skia-canvas'

// with default family name
FontLibrary.use([
  "fonts/Oswald-Regular.ttf",
  "fonts/Oswald-SemiBold.ttf",
  "fonts/Oswald-Bold.ttf",
])

// with an alias
FontLibrary.use("Grizwald", [
  "fonts/Oswald-Regular.ttf",
  "fonts/Oswald-SemiBold.ttf",
  "fonts/Oswald-Bold.ttf",
])
```

#### multiple families with aliases
```js
FontLibrary.use({
  Nieuwveen: ['fonts/AmstelvarAlpha-VF.ttf', 'fonts/AmstelvarAlphaItalic-VF.ttf'],
  Fairway: 'fonts/Raleway/*.ttf'
})
```

The return value will be either a list or an object (matching the style in which it was called) with an entry describing each font file that was added. For instance, one of the entries from the first example could be:
```js
{
  family: 'Grizwald',
  weight: 600,
  style: 'normal',
  width: 'normal',
  file: 'fonts/Oswald-SemiBold.ttf'
}
```

#### with a list of ‘glob’ patterns

:::warning
Glob support is no longer built-in as of v3.0: Try installing the [`glob`][glob] or [`fast-glob`][fastglob] module if you'd like to emulate the old behavior.
:::

> Note to Windows users: glob patterns require that you write paths using unix-style _forward_ slashes. Backslashes are used solely for escaping wildcard characters.

```js
import {globSync:glob} from 'fast-glob'

// with default family name
FontLibrary.use(glob('fonts/Crimson_Pro/*.ttf'))

// with an alias
FontLibrary.use("Stinson", glob('fonts/Crimson_Pro/*.ttf'))
```


<!-- references_begin -->
[glob]: https://www.npmjs.com/package/glob
[fastglob]: https://www.npmjs.com/package/fast-glob
<!-- references_end -->
