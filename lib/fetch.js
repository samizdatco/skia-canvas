// wrapper that calls either native fetch (on node v18+) or node-fetch (for older systems)
const fetch = (src, options) => {
  const fn = global.fetch
    ? Promise.resolve(global.fetch)
    : import('node-fetch').then(({default:fetch}) => fetch)
  return fn.then(fetch => fetch(src, options))
}

module.exports = {fetch}
