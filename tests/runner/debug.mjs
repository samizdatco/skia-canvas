// Live per-file dot reporter for `make debug` (watch mode)

import { basename } from 'node:path'
import { dot } from 'node:test/reporters'
import chalk from 'chalk'
import { diff } from 'jest-diff'
import cleanStack from 'clean-stack'

const labelWidth = 12
const stackRegex = /(?:\n {4}at .*)+/
const testRunnerStackRegex = /^ {4}at Test\.runInAsyncScope \(/m

const indent = (text, level = 1) => {
  const prefix = ' '.repeat(level * 2)
  return prefix + text.split('\n').join('\n' + prefix)
}

function errDiff (err) {
  const diffable = 'expected' in err && 'actual' in err && err.expected !== undefined
  return diffable && err.showDiff !== false ? diff(err.expected, err.actual) || undefined : undefined
}

function causeChain (err) {
  const chain = [err]
  while (err.cause instanceof Error && !chain.includes(err.cause)) chain.push(err = err.cause)
  return chain
}

function formatError (err) {
  const firstStackLine = (err.stack || '').split('\n')[0]
  const extracted = (err.stack || '').match(stackRegex) || []
  const pruned = extracted[0]?.slice(1).split(testRunnerStackRegex)[0] || ''

  let message = err.message.split('\n')[0] || ''
  if (firstStackLine?.includes(message) && err.name !== 'AssertionError') message = firstStackLine

  const stack = cleanStack(pruned, { basePath: process.cwd() }).replaceAll(/^\s+/gm, '')
  const parts = [chalk.red(message), chalk.gray(stack)]
  const diffStr = errDiff(err)
  if (diffStr) parts.splice(1, 0, diffStr)
  return parts.join('\n\n')
}

function formatErrorAndCauses (err) {
  if (err.code === 'ERR_TEST_FAILURE' && err.cause instanceof Error) err = err.cause
  return causeChain(err)
    .map((e, i) => indent((i ? 'caused by:\n\n' : '') + formatError(e), i))
    .join('\n\n')
}

function renderFailures (failures) {
  return failures.map((d, i) =>
    chalk.red(`${i + 1}) ${basename(d.file)} › ${d.name}`) + '\n' +
    indent(formatErrorAndCauses(d.details.error), 3) + '\n'
  ).join('\n')
}

//
// reporter
//

export default async function * debugReporter (source) {
  // Non-interactive: defer to the built-in dot reporter.
  if (!process.stdout.isTTY) { yield * dot(source); return }

  const order = []            // files, in enqueue order
  const marks = new Map()     // file -> array of rendered marks
  const failed = new Set()    // files with >=1 failing test
  let failures = []
  let drawn = 0               // lines the live block currently occupies

  const track = file => {
    if (!marks.has(file)) { order.push(file); marks.set(file, []) }
  }

  const render = () => {
    const rewind = drawn ? `\x1b[${drawn}A\x1b[0J` : ''   // up N lines, clear to end
    const width = Math.max((process.stdout.columns || 80) - labelWidth, 0)
    const body = order.map(f => {
      const label = basename(f, '.test.js').padEnd(labelWidth)
      const styled = failed.has(f)
        ? chalk.red(label)      // failure: red
        : chalk.italic(label)   // normal: default color + italic
      return styled + marks.get(f).slice(0, width).join('')
    }).join('\n') + '\n'
    drawn = order.length
    return rewind + body
  }

  for await (const { type, data } of source) {
    if (type === 'test:enqueue' && data.file) {
      if (!marks.has(data.file)) { track(data.file); yield render() }
    } else if ((type === 'test:pass' || type === 'test:fail') && data.details?.type !== 'suite') {
      track(data.file)
      marks.get(data.file).push(type === 'test:pass' ? chalk.cyan('·') : chalk.red('X'))
      if (type === 'test:fail') { failures.push(data); failed.add(data.file) }
      yield render()
    } else if (type === 'test:summary' && !data.file) {
      const report = renderFailures(failures)
      yield (report ? '\n' + report : '') + '\n'
      order.length = 0
      marks.clear()
      failed.clear()
      failures = []
      drawn = 0
    }
  }
}
