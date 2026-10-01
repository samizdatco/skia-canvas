// @ts-check

"use strict"

const {assert, describe, test} = require('../runner'),
      {App} = require('../../lib');

describe('App', () => {
  describe('import behaviour', () => {
    test('importing skia-canvas does not register the dispatch callback (no Neon root reference created)', () => {
      // The App singleton is created at import time. Prior to this fix, the constructor
      // immediately called App_register(), creating a Neon Root<JsFunction> that kept
      // the Node.js event loop alive — causing spurious open-handle warnings in Jest
      // and similar test runners when no windows were ever opened.
      //
      // After the fix, register() is deferred to launch(), so App.running is false
      // and no Neon root reference exists until a window is actually opened.
      assert.strictEqual(App.running, false)
    })

    test('App is exported as a singleton', () => {
      const {App: App2} = require('../../lib')
      assert.strictEqual(App, App2)
    })
  })
})