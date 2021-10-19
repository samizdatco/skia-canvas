const childProcess = require('child_process')
const fs = require('fs')
const path = require('path')
const util = require('util')

const execAsync = util.promisify(childProcess.exec)
const readFileAsync = util.promisify(fs.readFile)

const commitMessage = process.env.CI_COMMIT_MESSAGE
const branchName = process.env.CI_COMMIT_REF_NAME
console.log(`commitMessage: ${commitMessage}`)
console.log(`branchName: ${branchName}`)

if (
  !commitMessage ||
  !commitMessage.includes(`chore(release): publish`) ||
  (branchName !== 'master' && !branchName.startsWith('release/'))
) {
  console.log(`publish skipped.`)
  process.exit()
}

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
    console.log(`publishing ${pkg.name}@${version} to @arkie npm...`)
    await execAsync(`npm publish`)
  }
}

main().catch((e) => {
  console.error(e)
  process.exit(1)
})
