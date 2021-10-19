const childProcess = require('child_process')
const fs = require('fs')
const path = require('path')
const util = require('util')
const nopt = require('nopt');
const versioning = require('@mapbox/node-pre-gyp/lib/util/versioning.js')
const OSS = require('ali-oss')

const execAsync = util.promisify(childProcess.exec)
const readFileAsync = util.promisify(fs.readFile)

const configDefs = {
  help: Boolean,     // everywhere
  arch: String,      // 'configure'
  debug: Boolean,    // 'build'
  directory: String, // bin
  proxy: String,     // 'install'
  loglevel: String  // everywhere
};

/**
 * nopt shorthands
 */

const shorthands = {
  release: '--no-debug',
  C: '--directory',
  debug: '--debug',
  j: '--jobs',
  silent: '--loglevel=silent',
  silly: '--loglevel=silly',
  verbose: '--loglevel=verbose'
};


const ALI_ACCESS_KEY_ID = process.env.ALI_ACCESS_KEY_ID
const ALI_SECRET_ACCESS_KEY = process.env.ALI_SECRET_ACCESS_KEY || 'WfVPoKubLjh1fM9P9fnhtaMlw8HnXs'

const main = async () => {
  
  const buffer = await readFileAsync(path.resolve(__dirname, 'package.json'))
  const pkg = JSON.parse(buffer.toString())

  let versions
  try {
    versions = (await execAsync(`npm view ${pkg.name} versions`)).stdout
  } catch (error) {
    if (typeof error.message === 'string' && error.message.includes('404 Not Found')) {
      versions = ''
    } else {
      console.error(error)
      process.exit(1)
    }
  }
  const version = pkg.version
  if (!versions.includes(`'${version}'`)) {
    await upload(pkg)
  }
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})


const upload = async (pkg) => {

  const gypOpts = nopt(configDefs, shorthands);

  const npm_config_prefix = 'npm_config_';
    Object.keys(process.env).forEach((name) => {
      if (name.indexOf(npm_config_prefix) !== 0) return;
      const val = process.env[name];
      if (name !== npm_config_prefix + 'loglevel') {
        // add the user-defined options to the config
        name = name.substring(npm_config_prefix.length);
        // avoid npm argv clobber already present args
        // which avoids problem of 'npm test' calling
        // script that runs unique npm install commands
        if (name === 'argv') {
          if (gypOpts.argv &&
               gypOpts.argv.remain &&
               gypOpts.argv.remain.length) {
            // do nothing
          } else {
            gypOpts[name] = val;
          }
        } else {
          gypOpts[name] = val;
        }
      }
    })
    const opts = versioning.evaluate(pkg, gypOpts, 6);
    const tarball = opts.staged_tarball;

    const client = new OSS({
      region: 'oss-cn-beijing',
      accessKeyId:  ALI_ACCESS_KEY_ID,
      accessKeySecret: ALI_SECRET_ACCESS_KEY,
      bucket: 'mock-test'
    })

    try{
      const result = await client.put(`${opts.remote_path}${opts.package_name}`.slice(1), __dirname + "/" + tarball);
    }catch(e){
      console.error(e)
    }
}