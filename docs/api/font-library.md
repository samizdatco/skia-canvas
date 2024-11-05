---
description: Inspect your system fonts or load new ones
---
# FontLibrary

The `FontLibrary` is a static class which does not need to be instantiated with `new`. Instead you can access the properties and methods on the global `FontLibrary` you import from the module and its contents will be shared across all canvases you create.


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

The `FontLibrary.use()` method allows you to dynamically load local font files and use them with your canvases. By default it will use whatever family name is in the font metadata, but this can be overridden by an alias you provide. Since font-wrangling can be messy, `use` can be called in a number of different ways:

#### with a list of file paths
```js
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

#### with a list of ‘glob’ patterns

> Note to Windows users: Due to recent changes to the [glob][glob] module, you must write paths using unix-style forward slashes. Backslashes are now used solely for escaping wildcard characters.

```js
// with default family name
FontLibrary.use(['fonts/Crimson_Pro/*.ttf'])

// with an alias
FontLibrary.use("Stinson", ['fonts/Crimson_Pro/*.ttf'])
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

<!-- references_begin -->
[glob]: https://github.com/isaacs/node-glob/blob/main/changelog.md#80
<!-- references_end -->
