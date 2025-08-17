// rapper that exports either native fetch (on node v18+) or node-fetch (for older systems)
module.exports = {
  fetch: global.fetch || require('node-fetch')
}
