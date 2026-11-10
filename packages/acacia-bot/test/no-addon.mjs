// `node --import ./test/no-addon.mjs script.mjs`: fails the script's exit if acacia's native addon got loaded, read
// from the process report's shared objects (a WebSocket-only script must not need a binary for its platform).
process.on('exit', () => {
  const loaded = process.report.getReport().sharedObjects.filter((path) => /acacia-bot\.[^/\\]*\.node$/.test(path))
  if (loaded.length === 0) return
  process.stderr.write(`no-addon: the native addon was loaded: ${loaded.join(', ')}\n`)
  process.exitCode = 1
})
