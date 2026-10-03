// Usage: node tools/transport-probe.mjs host[:port] [host[:port] ...]
// Sends a RakNet unconnected ping (UDP) and a NetherNet signalling probe (HTTP GET /v1/join on the same port).
import dgram from 'node:dgram';

const MAGIC = Buffer.from('00ffff00fefefefefdfdfdfd12345678', 'hex');
const TIMEOUT_MS = 3000;

function raknetPing(host, port) {
  return new Promise((resolve) => {
    const sock = dgram.createSocket('udp4');
    const ping = Buffer.alloc(33);
    ping[0] = 0x01;
    ping.writeBigInt64BE(BigInt(Date.now()), 1);
    MAGIC.copy(ping, 9);
    ping.writeBigInt64BE(BigInt(Math.floor(Math.random() * 2 ** 48)), 25);

    const start = performance.now();
    const timer = setTimeout(() => { sock.close(); resolve({ ok: false, reason: 'timeout' }); }, TIMEOUT_MS);
    sock.on('error', (e) => { clearTimeout(timer); sock.close(); resolve({ ok: false, reason: e.code }); });
    sock.on('message', (msg) => {
      if (msg[0] !== 0x1c) return;
      clearTimeout(timer);
      sock.close();
      const len = msg.readUInt16BE(33);
      const motd = msg.subarray(35, 35 + len).toString('utf8').split(';');
      resolve({ ok: true, rttMs: Math.round(performance.now() - start), protocol: motd[2], version: motd[3], players: `${motd[4]}/${motd[5]}`, name: motd[1] });
    });
    sock.send(ping, port, host);
  });
}

async function nethernetProbe(host, port) {
  try {
    const res = await fetch(`http://${host}:${port}/v1/join`, { signal: AbortSignal.timeout(TIMEOUT_MS) });
    return { ok: true, status: res.status };
  } catch (e) {
    return { ok: false, reason: e.cause?.code ?? e.name };
  }
}

const targets = process.argv.slice(2);
if (!targets.length) { console.error('usage: node transport-probe.mjs host[:port] ...'); process.exit(1); }

const results = await Promise.all(targets.map(async (t) => {
  const [host, p] = t.split(':');
  const port = Number(p ?? 19132);
  const [rak, nn] = await Promise.all([raknetPing(host, port), nethernetProbe(host, port)]);
  return { target: `${host}:${port}`, rak, nn };
}));

for (const { target, rak, nn } of results) {
  const rakStr = rak.ok ? `RakNet OK ${rak.rttMs}ms proto=${rak.protocol} v${rak.version} players=${rak.players}` : `RakNet FAIL (${rak.reason})`;
  const nnStr = nn.ok ? `HTTP /v1/join -> ${nn.status}` : `HTTP /v1/join FAIL (${nn.reason})`;
  console.log(`${target.padEnd(34)} ${rakStr} | ${nnStr}`);
}
