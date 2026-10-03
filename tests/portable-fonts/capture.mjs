import assert from "node:assert/strict"
import { createHash } from "node:crypto"
import { readFile, mkdir, writeFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { fileURLToPath } from "node:url"

import { Canvas, FontLibrary, loadCanvas } from "../../lib/index.mjs"

const [outputArgument, ...flags] = process.argv.slice(2)
assert(outputArgument, "Usage: capture.mjs <output-dir> [--changed-label|--removed-line]")
assert(flags.every(flag => ["--changed-label", "--removed-line"].includes(flag)), "Unknown flag")
const outputRoot = resolve(outputArgument)
const sourceRoot = dirname(fileURLToPath(import.meta.url))
const packageRoot = resolve(sourceRoot, "../..")
// The custom empty manager has one unnamed empty family.
const systemFamilies = FontLibrary.families.filter(family => family !== "")
assert.equal(systemFamilies.length, 0, "Portable build exposes system fonts")
const faces = [
  ["MontserratRegular", "normal", 400, "montserrat-latin/montserrat-v30-latin-regular.woff2"],
  ["MontserratBold", "normal", 700, "montserrat-latin/montserrat-v30-latin-700.woff2"],
  ["MontserratItalic", "italic", 400, "montserrat-latin/montserrat-v30-latin-italic.woff2"],
  ["MontserratBoldItalic", "italic", 700, "montserrat-latin/montserrat-v30-latin-700italic.woff2"],
  ["MontserratLight", "normal", 200, "montserrat-latin/montserrat-v30-latin-200.woff2"],
  ["MonotonWoff", "normal", 400, "Monoton-Regular.woff"],
  ["MonotonWoff2", "normal", 400, "Monoton-Regular.woff2"],
  ["Amstelvar", "normal", 400, "AmstelvarAlpha-VF.ttf"],
  ["Oswald", "normal", 500, "Oswald-Medium.ttf"],
]
const aliases = faces.map(([alias]) => alias)
const fonts = []
for (const [alias, , , file] of faces) {
  const filename = join(packageRoot, "tests/assets/fonts", file)
  FontLibrary.use(alias, filename)
  assert(FontLibrary.has(alias), `Font was not loaded: ${alias}`)
  fonts.push({ alias, file, sha256: hash(await readFile(filename)) })
}

const samples = [
  "Snapshot 0123456789 MWil.,-()",
  "AVATAR office ffi fi A\u0308 é € —",
  "The quick brown fox jumps over the lazy dog",
  "Bold italic text and dimensions 0123456789",
  "Display dimensions and connection details for wrapped text",
]
const settings = { gpu: false, textContrast: 0, textGamma: 1.4 }
const canvas = new Canvas(1000, 1900, settings)
const ctx = canvas.getContext("2d", { colorSpace: "srgb" })
ctx.fontHinting = false
ctx.fontSmoothing = true
ctx.fontSynthesis = false
ctx.fillStyle = "white"
ctx.fillRect(0, 0, canvas.width, canvas.height)
ctx.fillStyle = "black"
const cases = []
let y = 30.25
for (const [faceIndex, [family, style, weight]] of faces.entries()) {
  const alias = aliases[faceIndex]
  for (const size of [12, 18.5, 24]) {
    ctx.font = `${style} ${weight} ${size}px "${alias}"`
    ctx.letterSpacing = size === 18.5 ? "0.25px" : "0px"
    for (const sample of samples) {
      const metrics = ctx.measureText(sample)
      const values = Object.fromEntries(Object.entries(metrics))
      assert(metrics.width > 0, `No text width: ${family}`)
      assert(
        Object.values(values)
          .filter((value) => typeof value === "number")
          .every(Number.isFinite),
      )
      ctx.textWrap = true
      const wrapped = Object.fromEntries(Object.entries(ctx.measureText(sample, 160.5)))
      ctx.textWrap = false
      cases.push({ family, style, weight, size, sample, metrics: values, wrapped })
    }
    // Render representative mixed content; measurements include all samples above.
    const visible = `${samples[0]} ${samples[2]} ${samples[3]}`
    ctx.fillText(visible, 18.25, y)
    const glyphPixels = ctx.getImageData(0, Math.floor(y - size * 1.5), canvas.width, Math.ceil(size * 2)).data
    assert(glyphPixels.some((value, index) => index % 4 !== 3 && value < 240), `No visible glyphs: ${alias}`)
    y += 42.5
  }
}
ctx.font = `18px "${aliases[0]}", "${aliases[5]}"`
ctx.textWrap = true
ctx.fillText(samples[4].repeat(4), 18.25, y, 160.5)
ctx.textWrap = false
ctx.strokeStyle = "#1565c0"
ctx.lineWidth = 0.5
if (!flags.includes("--removed-line")) {
  ctx.beginPath()
  ctx.moveTo(18.25, 1750.25)
  ctx.lineTo(940.75, 1750.25)
  ctx.stroke()
}
ctx.fillText(flags.includes("--changed-label") ? "Connection B" : "Connection A", 18.25, 1800.5)
const pixels = ctx.getImageData(0, 0, canvas.width, canvas.height).data
assert(
  pixels.some((value, index) => index % 4 !== 3 && value < 240),
  "Blank render",
)
const packageInfo = JSON.parse(await readFile(join(packageRoot, "package.json"), "utf8"))
const pdf = await canvas.toBuffer("pdf")
const pdfCanvas = await loadCanvas(pdf, { ...settings, colorSpace: "srgb" })
const pdfPixels = pdfCanvas
  .getContext("2d")
  .getImageData(0, 0, pdfCanvas.width, pdfCanvas.height).data
assert(
  pdfPixels.some((value, index) => index % 4 !== 3 && value < 240),
  "Blank PDF render",
)
const report = {
  captureSource: hash(await readFile(fileURLToPath(import.meta.url))),
  version: packageInfo.version,
  platform: process.platform,
  arch: process.arch,
  node: process.version,
  locale: process.env.LANG ?? null,
  systemFamilyCount: systemFamilies.length,
  settings: {
    ...settings,
    fontHinting: false,
    fontSmoothing: true,
    fontSynthesis: false,
    colorSpace: "srgb",
  },
  fonts,
  width: canvas.width,
  height: canvas.height,
  pdfWidth: pdfCanvas.width,
  pdfHeight: pdfCanvas.height,
  cases,
  pixelHash: hash(pixels),
  pdfPixelHash: hash(pdfPixels),
}
await mkdir(outputRoot, { recursive: true })
await Promise.all([
  writeFile(join(outputRoot, "report.json"), JSON.stringify(report, null, 2) + "\n"),
  writeFile(join(outputRoot, "canvas.rgba"), pixels),
  writeFile(join(outputRoot, "canvas.png"), await canvas.toBuffer("png")),
  writeFile(join(outputRoot, "canvas.pdf"), pdf),
  writeFile(join(outputRoot, "pdf.rgba"), pdfPixels),
  writeFile(join(outputRoot, "pdf.png"), await pdfCanvas.toBuffer("png")),
])
console.log(
  `${report.version}: ${cases.length} metric cases; ${systemFamilies.length} system families; pixels ${report.pixelHash}`,
)

function hash(data) {
  return createHash("sha256").update(data).digest("hex")
}
