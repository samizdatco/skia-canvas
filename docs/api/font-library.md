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
```js returns="{family, weights, widths, styles, variable, variations, features}"
FontLibrary.family(name)
```

If `name` matches the name of an installed font family, this method will return an object with information about the available weights and styles (aggregated across all the individual fonts in the family). If no matches are found, it will return `undefined`.

For instance, on my system `FontLibrary.family("Raleway")` returns:
```js
{
  family: 'Raleway',
  weights: [
    100, 200, 300,
    400, 500, 600,
    700, 800, 900
  ],
  widths: [ 'normal' ],
  styles: [ 'normal' ],
  variable: true,
  variations: { wght: { min: 100, max: 900, default: 100, label: 'Weight' } },
  features: {
    aalt: { label: 'Access All Alternates', type: 'indexed' },
    c2sc: { label: 'Small Capitals From Capitals', type: 'on/off' },
    ccmp: { label: 'Glyph Composition / Decomposition', type: 'on/off' },
    dlig: { label: 'Discretionary Ligatures', type: 'on/off' },
    dnom: { label: 'Denominators', type: 'on/off' },
    frac: { label: 'Fractions', type: 'on/off' },
    kern: { label: 'Kerning', type: 'on/off' },
    liga: { label: 'Standard Ligatures', type: 'on/off' },
    lnum: { label: 'Lining Figures', type: 'on/off' },
    locl: { label: 'Localized Forms', type: 'on/off' },
    mark: { label: 'Mark Positioning', type: 'on/off' },
    mkmk: { label: 'Mark to Mark Positioning', type: 'on/off' },
    numr: { label: 'Numerators', type: 'on/off' },
    ordn: { label: 'Ordinals', type: 'on/off' },
    salt: { label: 'Stylistic Alternates', type: 'indexed' },
    sinf: { label: 'Scientific Inferiors', type: 'on/off' },
    smcp: { label: 'Small Capitals', type: 'on/off' },
    ss01: { label: 'Stylistic Set 1', type: 'on/off' },
    ss02: { label: 'Stylistic Set 2', type: 'on/off' },
    ss03: { label: 'Stylistic Set 3', type: 'on/off' },
    ss04: { label: 'Stylistic Set 4', type: 'on/off' },
    ss05: { label: 'Stylistic Set 5', type: 'on/off' },
    ss06: { label: 'Stylistic Set 6', type: 'on/off' },
    ss07: { label: 'Stylistic Set 7', type: 'on/off' },
    ss08: { label: 'Stylistic Set 8', type: 'on/off' },
    ss09: { label: 'Stylistic Set 9', type: 'on/off' },
    ss10: { label: 'Stylistic Set 10', type: 'on/off' },
    ss11: { label: 'Stylistic Set 11', type: 'on/off' },
    subs: { label: 'Subscript', type: 'on/off' },
    sups: { label: 'Superscript', type: 'on/off' }
  }
}
```

Because Raleway is a [variable font][VariableFonts], the `variable` flag is set to `true`, and its variation axes are itemized in the `variations` object. Each key in `variations` is a four-character code that can be used with the Context's  [`fontVariationSettings`][fontvariations] property and points to an object with the font's `min`, `max`, and `default` values for that axis (plus a human-readable `label`).

The `features` object itemizes all the OpenType features the font supports, identifying them using four-character codes that can be used with the [`fontFeatureSettings`][fontfeatures] property. Each feature's `type` can either be `"on/off"` (meaning it can be set to `1` to enable it or `0` to disable it) or `indexed` (meaning it allows you to select among different alternates via a 1-based index, or `0` to use the default form).


### `has()`
```js
FontLibrary.has(familyName)
```

Returns `true` if the family is installed on the system or has been added via `FontLibrary.use()`.

### `reset()`

Uninstalls any dynamically loaded fonts that had been added via `FontLibrary.use()`.

### `use()`
```js returns="{family, weight, style, width, variable, variations, features, file}[]"
FontLibrary.use([...fontPaths])
FontLibrary.use(familyName, [...fontPaths])
FontLibrary.use({familyName:[...fontPaths], ...})
```

The `FontLibrary.use()` method allows you to dynamically load local font files and use them with your canvases. It can read fonts in the OpenType (`.otf`), TrueType (`.ttf`), and web-font (`.woff` & `.woff2`) file formats.

By default the family name will be taken from the font metadata, but this can be overridden by an alias you provide. Since font-wrangling can be messy, `use` can be called in a number of different ways:

#### with a list of file paths
```js
import {FontLibrary} from 'skia-canvas'

// use the default family name
FontLibrary.use([
  "fonts/Oswald-Regular.ttf",
  "fonts/Oswald-SemiBold.ttf",
  "fonts/Oswald-Bold.ttf",
])
```

#### with a custom family name
```js
// override the default name (possibly to avoid conflicts with system fonts)
FontLibrary.use("Grizwald", [
  "fonts/Oswald-Regular.ttf",
  "fonts/Oswald-SemiBold.ttf",
  "fonts/Oswald-Bold.ttf",
])
```

#### multiple families with aliases
```js
// each key is the custom family name, values are the fonts in that family
FontLibrary.use({
  Nieuwveen: ['fonts/AmstelvarAlpha-VF.ttf', 'fonts/AmstelvarAlphaItalic-VF.ttf'],
  Fairway: 'fonts/Raleway/*.ttf'
})
```

#### with a list of ‘glob’ patterns

```js
import {globSync as glob} from 'fast-glob'

// with default family name
FontLibrary.use(glob('fonts/Crimson_Pro/*.ttf'))

// with an alias
FontLibrary.use("Stinson", glob('fonts/Crimson_Pro/*.ttf'))
```

---

The return value will be either a list or an object (matching the style in which it was called) with an entry describing each font file that was added. For instance, the Semibold from the "Grizwald" alias example above could be:
```js
{
  family: 'Grizwald',
  weight: 600,
  style: 'normal',
  width: 'normal',
  variable: false,
  variations: {},
  features: {
    aalt: { label: 'Access All Alternates', type: 'indexed' },
    case: { label: 'Case-Sensitive Forms', type: 'on/off' },
    ccmp: { label: 'Glyph Composition / Decomposition', type: 'on/off' },
    dlig: { label: 'Discretionary Ligatures', type: 'on/off' },
    frac: { label: 'Fractions', type: 'on/off' },
    kern: { label: 'Kerning', type: 'on/off' },
    liga: { label: 'Standard Ligatures', type: 'on/off' },
    locl: { label: 'Localized Forms', type: 'on/off' },
    mark: { label: 'Mark Positioning', type: 'on/off' },
    mkmk: { label: 'Mark to Mark Positioning', type: 'on/off' },
    ordn: { label: 'Ordinals', type: 'on/off' },
    sups: { label: 'Superscript', type: 'on/off' }
  },
  file: 'fonts/Oswald-SemiBold.ttf'
}
```



<!-- references_begin -->
[fontvariations]: context.md#fontvariationsettings
[fontfeatures]: context.md#fontfeaturesettings
[VariableFonts]: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Fonts/Variable_Fonts_Guide
<!-- references_end -->
