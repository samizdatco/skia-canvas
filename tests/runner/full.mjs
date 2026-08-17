// Adapted from @voxpelli/node-test-pretty-reporter@1.1.2 (MIT) for `make test`

import chalk from 'chalk'
import { diff } from 'jest-diff'
import cleanStack from 'clean-stack'

const slowIfAboveMs = 75
const stackRegex = /(?:\n {4}at .*)+/
const testRunnerStackRegex = /^ {4}at Test\.runInAsyncScope \(/m

const logSymbols = { success: chalk.green('✔'), info: chalk.blue('ℹ') }

const pad = n => ' '.repeat(n)

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
    yield pad((test.nesting + 1) * 2 + (i ? prefix.length : 0)) + (i ? '' : prefix) + test.name
  }
  yield suffix + '\n'
}

export default async function * prettyReporter (source) {
  const diagnostics = []
  const failures = []
  const parentStack = []
  let stack = []

  for await (const { data, type } of source) {
    if (type === 'test:start') {
      stack.push(data)
      while (parentStack.length && parentStack.at(-1).nesting >= data.nesting) parentStack.pop()
      parentStack.push(data)
    } else if (type === 'test:diagnostic') {
      diagnostics.push(data)
    } else if (type === 'test:stdout') {
      process.stdout.write(data.message)
    } else if (type === 'test:stderr') {
      process.stderr.write(data.message)
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
        label = chalk.gray(`${logSymbols.success} ${data.name}`)
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

  const info = diagnostics.map(d => `${logSymbols.info} ${d.message}`).join('\n')
  yield '\n' + chalk.gray(info) + '\n\n'

  for (const [i, { data, parentStack }] of failures.entries()) {
    yield * printParentStack(parentStack, `${i + 1}) `, ':')
    yield '\n' + indent(formatErrorAndCauses(data.details.error), 3) + '\n\n'
  }
}
