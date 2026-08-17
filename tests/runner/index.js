//
// Run the test suite: `node tests/runner [mode] [flags…] [filters…]`
//
'use strict'

const {spawn} = require('node:child_process')
const {readdirSync} = require('node:fs')
const {resolve} = require('node:path')

const ROOT = resolve(__dirname, '../..')

const suite = () => readdirSync(`${ROOT}/tests/suite`)
  .filter(file => file.endsWith('.test.js'))
  .sort()
  .map(file => `tests/suite/${file}`)

const RECIPES = {
  test: () => ['--test', ...suite()], // npm test
  full: () => ['--test', '--test-reporter', './tests/runner/full.mjs', ...suite()], // make test
  debug: () => ['--test', '--test-reporter', './tests/runner/debug.mjs', '--watch', ...suite()], // make debug
}

const args = process.argv.slice(2)
const mode = RECIPES[args[0]] ? args.shift() : 'test'
const flags = args.flatMap(arg => arg.startsWith('-') ? [arg] : [`--test-name-pattern=${arg}`])

spawn(process.execPath, [...flags, ...RECIPES[mode]()], {stdio:'inherit', cwd:ROOT})
  .on('exit', (code, signal) => signal ? process.kill(process.pid, signal) : process.exit(code ?? 1))
