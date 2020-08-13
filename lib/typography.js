module.exports = {
  "weightMap":{
    "lighter":300, "normal":400, "bold":700, "bolder":800
  },

  "sizeMap":{
    "xx-small":3/5, "x-small":3/4, "small":8/9, "smaller":8/9,
    "large":6/5, "larger":6/5, "x-large":3/2, "xx-large":2/1,
    "normal": 1.2 // special case for lineHeight
  },

  "featureMap":{
    "normal": [],

    // font-variant-ligatures
    "common-ligatures": ["liga", "clig"],
    "no-common-ligatures": ["-liga", "-clig"],
    "discretionary-ligatures": ["dlig"],
    "no-discretionary-ligatures": ["-dlig"],
    "historical-ligatures": ["hlig"],
    "no-historical-ligatures": ["-hlig"],
    "contextual": ["calt"],
    "no-contextual": ["-calt"],

    // font-variant-position
    "super": ["sups"],
    "sub": ["subs"],

    // font-variant-caps
    "small-caps": ["smcp"],
    "all-small-caps": ["c2sc", "smcp"],
    "petite-caps": ["pcap"],
    "all-petite-caps": ["c2pc", "pcap"],
    "unicase": ["unic"],
    "titling-caps": ["titl"],

    // font-variant-numeric
    "lining-nums": ["lnum"],
    "oldstyle-nums": ["onum"],
    "proportional-nums": ["pnum"],
    "tabular-nums": ["tnum"],
    "diagonal-fractions": ["frac"],
    "stacked-fractions": ["afrc"],
    "ordinal": ["ordn"],
    "slashed-zero": ["zero"],

    // font-variant-east-asian
    "jis78": ["jp78"],
    "jis83": ["jp83"],
    "jis90": ["jp90"],
    "jis04": ["jp04"],
    "simplified": ["smpl"],
    "traditional": ["trad"],
    "full-width": ["fwid"],
    "proportional-width": ["pwid"],
    "ruby": ["ruby"],

    // font-variant-alternates (non-parameterized)
    "historical-forms": ["hist"],
  },

  "alternatesMap":{
      "stylistic": "salt #",
      "styleset": "ss##",
      "character-variant": "cv##",
      "swash": "swsh #",
      "ornaments": "ornm #",
      "annotation": "nalt #",
  }
}