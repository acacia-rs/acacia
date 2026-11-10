// The server the examples join, from the environment (offline auth: an `online-mode=false` server such as a LAN
// world or a local BDS): ACACIA_HOST (default 127.0.0.1), ACACIA_PORT (19132), ACACIA_TRANSPORT (auto, raknet or
// nethernet) and ACACIA_VIA (an acacia-daemon URL such as ws://127.0.0.1:19180; unset runs the bots in-process).
export function connection() {
  const { ACACIA_HOST, ACACIA_PORT, ACACIA_TRANSPORT, ACACIA_VIA } = process.env
  return {
    host: ACACIA_HOST || '127.0.0.1',
    port: Number(ACACIA_PORT || 19132),
    transport: ACACIA_TRANSPORT || 'auto',
    via: ACACIA_VIA || undefined,
  }
}
