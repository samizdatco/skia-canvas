const assert = require('node:assert')

Object.assign(assert, {
  contains: (actual, expected) => assert((actual || []).includes(expected)),
  doesNotContain: (actual, expected) => assert(!((actual || [expected]).includes(expected))),
  matchesSubset: (actual, expected) => Object.entries(expected).forEach(([key, val]) => assert.deepEqual(actual[key], val)),
  nearEqual: (actual, expected, tolerance = Math.pow(10, -2) / 2) => assert.ok(
    Math.abs(expected - actual) < tolerance,
    new assert.AssertionError({actual, expected, operator:"≈"})
  )
})

module.exports = {assert}
