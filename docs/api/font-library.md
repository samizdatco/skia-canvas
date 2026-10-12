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
```js returns="{family, weights, widths, styles, variable, variations, instances, features}"
FontLibrary.family(name)
```

If `name` matches the name of an installed font family, this method will return an object with information about the available weights and styles (aggregated across all the individual fonts in the family). If no matches are found, it will return `undefined`.

For instance, on my system `FontLibrary.family("Lora")` returns:
```js
{
  family: 'Lora',
  weights: [ 400, 500, 600, 700 ],
  widths: [ 'normal' ],
  styles: [ 'normal', 'italic' ],
  variable: true,
  variations: { wght: { min: 400, max: 700, default: 400, label: 'Weight' } },
  instances: {
    Regular: { style: 'normal', settings: { wght: 400 } },
    Medium: { style: 'normal', settings: { wght: 500 } },
    SemiBold: { style: 'normal', settings: { wght: 600 } },
    Bold: { style: 'normal', settings: { wght: 700 } },
    Italic: { style: 'italic', settings: { wght: 400 } },
    'Medium Italic': { style: 'italic', settings: { wght: 500 } },
    'SemiBold Italic': { style: 'italic', settings: { wght: 600 } },
    'Bold Italic': { style: 'italic', settings: { wght: 700 } }
  },
  features: {
    aalt: { label: 'Access All Alternates', type: 'indexed' },
    calt: { label: 'Contextual Alternates', type: 'on/off' },
    case: { label: 'Case-Sensitive Forms', type: 'on/off' },
    ccmp: { label: 'Glyph Composition / Decomposition', type: 'on/off' },
    frac: { label: 'Fractions', type: 'on/off' },
    kern: { label: 'Kerning', type: 'on/off' },
    liga: { label: 'Standard Ligatures', type: 'on/off' },
    locl: { label: 'Localized Forms', type: 'on/off' },
    mark: { label: 'Mark Positioning', type: 'on/off' },
    mkmk: { label: 'Mark to Mark Positioning', type: 'on/off' },
    ordn: { label: 'Ordinals', type: 'on/off' },
    pnum: { label: 'Proportional Figures', type: 'on/off' },
    rvrn: { label: 'Required Variation Alternates', type: 'on/off' },
    sups: { label: 'Superscript', type: 'on/off' },
    tnum: { label: 'Tabular Figures', type: 'on/off' }
  }
}
```

Because Lora is a [variable font][VariableFonts], the `variable` flag is set to `true`, and its variation axes are itemized in the `variations` object. Each key in `variations` is a four-character code that can be used with the Context’s [`fontVariationSettings`][fontvariations] property and points to an object with the font’s `min`, `max`, and `default` values for that axis (plus a human-readable `label`). The `instances` object lists the different names you can pass to [`fontInstance`][fontinstance] along with the style and variation settings that will be enabled for each.

The `features` object itemizes all the OpenType features the font supports, identifying them using four-character codes that can be used with the [`fontFeatureSettings`][fontfeatures] property. Each feature’s `type` can either be `"on/off"` (meaning it can be set to `1` to enable it or `0` to disable it) or `indexed` (meaning it allows you to select among different alternates via a 1-based index, or `0` to use the default form).


### `has()`
```js returns="boolean"
FontLibrary.has(familyName)
```

Returns `true` if the family is installed on the system or has been added via `FontLibrary.use()`.

### `reset()`

Uninstalls any dynamically loaded fonts that had been added via `FontLibrary.use()`.

### `use()`
```js returns="{family, weight, style, width, variable, variations, instances, features, file, replaces, replacedBy}[]"
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

The return value will be either a list or an object (matching the style in which it was called) with an entry describing each font file that was added. For instance, the Semibold from the “Grizwald” alias example above could be:
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


#### diagnosing errors

```js
// register a static and variable font with the same family name and intrinsic style
FontLibrary.use("Inter", [
  "fonts/Inter/Inter[opsz,wght].ttf", // variable (defaults to "Regular" weight)
  "fonts/Inter/Inter-Regular.ttf",    // static (single weight)
])
```
---

Only one font can be added to the library for each unique combination of family name, weight, and style. As a result, not every font file you pass to `use()` will be successfully registered. In general, later fonts overwrite earlier ones, but within a single `use()` call a variable font will always replace a static one regardless of ordering. The `replaces` and `replacedBy` properties in the return value will let you see how naming collisions were resolved:
- `replaces` means the font was successfully registered; its value identifies the font that it replaced
- `replacedBy` means the font was skipped; its value identifies the font that was added instead

```js
[
  {
    family: 'Inter',
    weight: 400,
    style: 'normal',
    width: 'normal',
    variable: false,
    variations: {},
    instances: {},
    features: { … },
    file: 'fonts/Inter/Inter-Regular.ttf',
    replacedBy: 'fonts/Inter/Inter[opsz,wght].ttf'
  },
  {
    family: 'Inter',
    weight: 400,
    style: 'normal',
    width: 'normal',
    variable: true,
    variations: {
      opsz: { min: 14, max: 32, default: 14, label: 'Optical size' },
      wght: { min: 100, max: 900, default: 400, label: 'Weight' }
    },
    instances: { Thin, ExtraLight, Light, … },
    features: { … },
    file: 'fonts/Inter/Inter[opsz,wght].ttf',
    replaces: 'fonts/Inter/Inter-Regular.ttf'
  }
]
```

<!-- references_begin -->
[fontinstance]: context.md#fontinstance
[fontvariations]: context.md#fontvariationsettings
[fontfeatures]: context.md#fontfeaturesettings
[VariableFonts]: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_Fonts/Variable_Fonts_Guide
<!-- references_end -->
