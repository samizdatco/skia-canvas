import assert from "node:assert/strict"
import { readFile } from "node:fs/promises"
import { join, resolve } from "node:path"
import { isDeepStrictEqual } from "node:util"

const [leftArgument, rightArgument, control] = process.argv.slice(2)
assert(
  leftArgument && rightArgument,
  "Usage: compare.mjs <left-output> <right-output> [--expect-pixel-difference]",
)
assert(control === undefined || control === "--expect-pixel-difference", "Unknown comparison mode")
const roots = [leftArgument, rightArgument].map((value) => resolve(value))
const [left, right] = await Promise.all(
  roots.map(async (root) => JSON.parse(await readFile(join(root, "report.json"), "utf8"))),
)
for (const key of [
  "version",
  "captureSource",
  "settings",
  "fonts",
  "width",
  "height",
  "pdfWidth",
  "pdfHeight",
]) {
  assert.deepEqual(left[key], right[key], `Different capture input: ${key}`)
}
const metricsMatch = isDeepStrictEqual(left.cases, right.cases)
const [differentPixels, pdfDifferentPixels] = await Promise.all(
  ["canvas.rgba", "pdf.rgba"].map(async (filename) => {
    const [expected, actual] = await Promise.all(
      roots.map((root) => readFile(join(root, filename))),
    )
    return countDifferentPixels(expected, actual)
  }),
)
const changedCases = left.cases.filter(
  (value, index) => !isDeepStrictEqual(value, right.cases[index]),
)
console.log(
  JSON.stringify(
    {
      left: `${left.platform}/${left.arch}`,
      right: `${right.platform}/${right.arch}`,
      metricsMatch,
      changedCases: changedCases.length,
      differentPixels,
      pdfDifferentPixels,
      examples: changedCases.slice(0, 3).map(({ family, style, weight, size, sample }) => ({
        family,
        style,
        weight,
        size,
        sample,
      })),
    },
    null,
    2,
  ),
)
assert(metricsMatch, "Font metrics differ")
if (control === "--expect-pixel-difference") {
  assert(differentPixels > 0, "Deliberate visible defect was not detected")
  assert(pdfDifferentPixels > 0, "Deliberate visible defect was not detected in the PDF")
} else {
  assert.equal(differentPixels, 0, "Decoded pixels differ")
  assert.equal(pdfDifferentPixels, 0, "PDF pixels differ")
}

function countDifferentPixels(expected, actual) {
  assert.equal(expected.length, actual.length, "Different pixel buffer sizes")
  let count = 0
  for (let offset = 0; offset < expected.length; offset += 4) {
    if (!expected.subarray(offset, offset + 4).equals(actual.subarray(offset, offset + 4))) count++
  }
  return count
}
