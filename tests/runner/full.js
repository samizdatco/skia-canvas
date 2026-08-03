// Adapted from @voxpelli/node-test-pretty-reporter@1.1.2 (MIT) for `make test`

'use strict'

let MarkdownOrChalk, diff, cleanStack

const slowIfAboveMs = 75
const stackRegex = /(?:\n {4}at .*)+/
const testRunnerStackRegex = /^ {4}at Test\.runInAsyncScope \(/m

const paint = (format, style, str) => format.chalk ? format.chalk[style](str) : str
const pad = n => ' '.repeat(n)

function errDiff (err) {
  const diffable = 'expected' in err && 'actual' in err && err.expected !== undefined
  return diffable && err.showDiff !== false ? diff(err.expected, err.actual) || undefined : undefined
}

function causeChain (err) {
  const chain = [err]
  while (err.cause instanceof Error && !chain.includes(err.cause)) chain.push(err = err.cause)
  return chain
}

function formatError (format, err) {
  const firstStackLine = (err.stack || '').split('\n')[0]
  const extracted = (err.stack || '').match(stackRegex) || []
  const pruned = extracted[0]?.slice(1).split(testRunnerStackRegex)[0] || ''

  let message = err.message.split('\n')[0] || ''
  if (firstStackLine?.includes(message) && err.name !== 'AssertionError') message = firstStackLine

  const stack = cleanStack(pruned, { basePath: process.cwd() }).replaceAll(/^\s+/gm, '')
  const parts = [paint(format, 'red', message), paint(format, 'gray', stack)]
  const diffStr = errDiff(err)
  if (diffStr) parts.splice(1, 0, diffStr)
  return parts.join('\n\n')
}

function formatErrorAndCauses (format, err) {
  if (err.code === 'ERR_TEST_FAILURE' && err.cause instanceof Error) err = err.cause
  return causeChain(err)
    .map((e, i) => format.indent((i ? 'caused by:\n\n' : '') + formatError(format, e), i))
    .join('\n\n')
}

function * printParentStack (parentStack, prefix = '', suffix = '') {
  for (const [i, test] of parentStack.entries()) {
    if (i) yield '\n'
    yield pad((test.nesting + 1) * 2 + (i ? prefix.length : 0)) + (i ? '' : prefix) + test.name
  }
  yield suffix + '\n'
}

module.exports = async function * prettyReporter (source) {
  ({ MarkdownOrChalk } = await import('markdown-or-chalk'))
  ;({ diff } = await import('jest-diff'))
  cleanStack = (await import('clean-stack')).default

  const format = new MarkdownOrChalk(false)
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
        label = paint(format, 'gray', `${format.logSymbols.success} ${data.name}`)
      } else {
        failures.push({ data, parentStack: [...parentStack] })
        label = paint(format, 'red', `${failures.length}) ${data.name}`)
      }
      yield pad((data.nesting + 1) * 2) + label

      if (data.details.duration_ms > slowIfAboveMs) {
        yield ' ' + paint(format, 'red', format.italic(`(${Math.floor(data.details.duration_ms)}ms)`))
      }
      yield '\n'
    }
  }

  const info = diagnostics.map(d => `${format.logSymbols.info} ${d.message}`).join('\n')
  yield '\n' + paint(format, 'gray', info) + '\n\n'

  for (const [i, { data, parentStack }] of failures.entries()) {
    yield * printParentStack(parentStack, `${i + 1}) `, ':')
    yield '\n' + format.indent(formatErrorAndCauses(format, data.details.error), 3) + '\n\n'
  }
}
