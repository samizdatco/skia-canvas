//
// Check the npm packaging pipeline (for CI testing)
//

import {execSync} from 'child_process'
import {existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync, readdirSync} from 'fs'
import {tmpdir} from 'os'
import {join, resolve} from 'path'

const ROOT = resolve(`${import.meta.dirname}/..`)
const sh = (cmd, opts={}) => execSync(cmd, {stdio:'pipe', encoding:'utf8', ...opts})

if (!existsSync(`${ROOT}/lib/skia.node`)){
  console.error(`No lib/skia.node found — build or download one first`)
  process.exit(1)
}

const work = mkdtempSync(join(tmpdir(), 'skia-canvas-packaging-'))
try{
  // assemble this platform's package, then pack it & the root module as tarballs
  sh(`node "${ROOT}/lib/prebuild.mjs" packages --local`)
  let [platformDir] = readdirSync(`${ROOT}/assets/npm`),
      platformPkg = `@skia-canvas/${platformDir}`
  console.log(`Packing skia-canvas + ${platformPkg}`)
  let tarballs = [ROOT, `${ROOT}/assets/npm/${platformDir}`].map(dir =>
    join(work, sh(`npm pack "${dir}" --silent`, {cwd:work}).trim().split('\n').pop())
  )

  // install both into a scratch project the same way an end user would get them
  let app = join(work, 'app')
  mkdirSync(app)
  writeFileSync(join(app, 'package.json'), JSON.stringify({name:'packaging-probe', private:true}))
  console.log(`Installing with --ignore-scripts into ${app}`)
  sh(`npm install --ignore-scripts --no-audit --no-fund ${tarballs.map(t => `"${t}"`).join(' ')}`, {cwd:app})

  if (existsSync(join(app, 'node_modules/skia-canvas/lib/skia.node'))){
    throw Error(`lib/skia.node was included in the packed module — the loader's platform-package path went untested`)
  }

  // render through the platform package
  writeFileSync(join(app, 'probe.mjs'), `
    import {Canvas} from 'skia-canvas'
    let canvas = new Canvas(32, 32),
        ctx = canvas.getContext('2d')
    ctx.fillStyle = 'red'
    ctx.fillRect(0, 0, 32, 32)
    let buf = await canvas.toBuffer('png')
    if (buf.readUInt32BE(0) != 0x89504e47) throw Error('output is not a PNG')
    console.log('render ok')
  `)
  let rendered = sh(`node probe.mjs`, {cwd:app})
  if (!rendered.includes('render ok')) throw Error(`probe render failed:\n${rendered}`)
  console.log(`✓ rendering works via ${platformPkg}`)

  // with the platform package gone, the loader should fail loudly & specifically
  rmSync(join(app, 'node_modules/@skia-canvas'), {recursive:true})
  let failure
  try{
    sh(`node probe.mjs`, {cwd:app})
    throw Error(`loader unexpectedly succeeded with no binary present`)
  }catch(e){
    failure = `${e.stderr || ''}${e.stdout || ''}${e.message}`
  }
  for (const expected of [platformPkg, 'npm/cli/issues/4828', 'prebuild.mjs download']){
    if (!failure.includes(expected)) throw Error(`missing-binary error doesn't mention "${expected}":\n${failure}`)
  }
  console.log(`✓ missing-binary error names ${platformPkg} and remedies`)

  console.log(`\npackaging checks passed`)
}finally{
  rmSync(work, {recursive:true, force:true})
}
