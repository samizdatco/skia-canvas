// Adapted from @voxpelli/node-test-pretty-reporter@1.1.2 (MIT) for `make test`

import { basename } from 'node:path'
import chalk from 'chalk'
import { diff } from 'jest-diff'
import cleanStack from 'clean-stack'

const slowIfAboveMs = 75
const slowRunMs = 2000       // whole-run duration turns magenta above this…
const sluggishRunMs = 5000   // …and red above this
const stackRegex = /(?:\n {4}at .*)+/
const testRunnerStackRegex = /^ {4}at Test\.runInAsyncScope \(/m

// node reports skipped & todo tests as passing, so they need to be told apart by icon
const icons = { pass: chalk.green('√'), skip: chalk.yellow('○'), todo: chalk.magenta('✎') }

const pad = n => ' '.repeat(n)

// a count in black-on-bright + bold, then its label in bright-white-on-normal + italic
const block = (count, label, color) =>
  chalk.black[`bg${color}Bright`].bold(` ${count} `) +
  chalk.whiteBright[`bg${color}`].italic(` ${label} `)

const indent = (text, level = 1) => {
  const prefix = pad(level * 2)
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

function * printParentStack (parentStack, prefix = '', suffix = '') {
  for (const [i, test] of parentStack.entries()) {
    if (i) yield '\n'
    const name = test.nesting === 0 ? chalk.bold(test.name) : test.name
    yield pad((test.nesting + 1) * 2 + (i ? prefix.length : 0)) + (i ? '' : prefix) + name
  }
  yield suffix + '\n'
}

export default async function * prettyReporter (source) {
  const diagnostics = []
  const failures = []
  const parentStack = []
  const output = new Map()   // file -> [{stream, message}], replayed at the end
  let stack = []

  const collect = (file, stream, message) => {
    const key = file || '(unknown)'
    if (!output.has(key)) output.set(key, [])
    output.get(key).push({ stream, message })
  }

  for await (const { data, type } of source) {
    if (type === 'test:start') {
      stack.push(data)
      while (parentStack.length && parentStack.at(-1).nesting >= data.nesting) parentStack.pop()
      parentStack.push(data)
    } else if (type === 'test:diagnostic') {
      diagnostics.push(data)
    } else if (type === 'test:stdout' || type === 'test:stderr') {
      // these arrive unbuffered, ahead of the results they belong to, and carry no test
      // identity — only a file. so stash them and replay grouped by file once the run ends
      collect(data.file, type === 'test:stderr' ? 'err' : 'out', data.message)
    } else if (type === 'test:pass' || type === 'test:fail') {
      if (stack.length === 0) continue

      if (stack.length > 1) {
        stack.pop()
        if (stack[0]?.nesting === 0) yield '\n'
        yield * printParentStack(stack)
      }
      stack = []
      if (data.nesting === 0) continue

      let label
      if (type === 'test:pass') {
        const icon = data.todo ? icons.todo : data.skip ? icons.skip : icons.pass
        label = chalk.gray(`${icon} ${data.name}`)
      } else {
        failures.push({ data, parentStack: [...parentStack] })
        label = chalk.red(`${failures.length}) ${data.name}`)
      }
      yield pad((data.nesting + 1) * 2) + label

      if (data.details.duration_ms > slowIfAboveMs) {
        yield ' ' + chalk.red(chalk.italic(`(${Math.floor(data.details.duration_ms)}ms)`))
      }
      yield '\n'
    }
  }

  for (const [i, { data, parentStack }] of failures.entries()) {
    yield * printParentStack(parentStack, `${i + 1}) `, ':')
    yield '\n' + indent(formatErrorAndCauses(data.details.error), 3) + '\n\n'
  }

  for (const [file, lines] of output) {
    yield '\n' + indent(chalk.bold(basename(file)), 1) + '\n'
    for (const { stream, message } of lines) {
      yield indent((stream === 'err' ? chalk.yellow : chalk.dim)(message.trimEnd()), 2) + '\n'
    }
  }

  // node only emits `test:summary` (with its ready-made counts) on v24+, so tally up the
  // end-of-run diagnostics instead — those are identical all the way back to v18
  const { pass, fail, skipped, todo, duration_ms } =
    Object.fromEntries(diagnostics.map(d => d.message.split(' ')))

  const ms = Number(duration_ms)
  const [count, unit] = ms >= 1000 ? [(ms / 1000).toFixed(1), 's'] : [Math.round(ms), 'ms']
  const hue = ms >= sluggishRunMs ? 'red' : ms >= slowRunMs ? 'magenta' : 'white'
  const elapsed = chalk[`${hue}Bright`](count) + chalk[hue].italic(unit)

  const tally = [[pass, 'passed', 'Blue'], [fail, 'failed', 'Red'],
                 [skipped, 'skipped', 'Magenta'], [todo, 'todo', 'Cyan']]
    .filter(([count]) => Number(count) > 0)
    .map(([count, label, color]) => block(count, label, color))

  yield '\n' + [...tally, elapsed].join(' ') + '\n\n'
}
