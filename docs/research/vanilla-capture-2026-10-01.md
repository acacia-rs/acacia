# Vanilla client captures (2026-10-01)

Minecraft for Windows 1.26.52.3 against local BDS 1.26.52.3. Raw captures (git-ignored, they contain
the player's ClientData; the NetherNet `tcp.log` also holds a MultiplayerToken):
`.testserver/nn-capture/{vanilla-1,vanilla,ours}` and `.testserver/mitm/20261001-211901.jsonl`.

Tools: `examples/nn_record.rs` (NetherNet signaling + UDP relay recorder), `tools/mitm` (Go,
gophertunnel: recording RakNet MITM; `-selftest` checks it), `examples/mitm_inspect.rs` (input-flag
runs + corrections per run, decoded with acacia-proto).

## NetherNet signaling (vanilla-1)
- Discovery: TLS ClientHello to `https://host:port` first, then `GET /v1/join` over HTTP, then the
  POST on the scheme that answered. It probes even on a direct join.
- Headers, in order: `Connection: Keep-Alive`, `Content-Type: application/sdp` (POST only),
  `User-Agent: libhttpclient/1.0.0.0`, `Content-Length` (POST only), `Host: host:port`.
- Offer layout: `v=0`, `o=- <u63> 2 IN IP4 127.0.0.1`, `s=-`, `t=0 0`, `a=group:BUNDLE 0`,
  `a=extmap-allow-mixed`, `a=msid-semantic: WMS`, **`a=identity` (session level)**,
  `m=application <first candidate port> UDP/DTLS/SCTP webrtc-datachannel`, `c=IN IP4 <first candidate ip>`,
  candidates, `a=ice-ufrag` (4 chars), `a=ice-pwd` (24 chars), `a=ice-options:trickle`,
  **`a=fingerprint` (media level)**, `a=setup:actpass`, `a=mid:0`, `a=sctp-port:5000`,
  `a=max-message-size:262144`. No `a=end-of-candidates`.
- Candidates: one per interface, each its own port, private/LAN and IPv6 only (no srflx):
  `candidate:<u32> 1 udp <prio> <ip> <port> typ host generation 0 network-id <n> network-cost 10`.
  The server learns a remote player's public address peer-reflexively.

## Movement (mitm capture, strict BDS)
- Strict BDS (`player-position-acceptance-threshold=0.0001`) corrected the vanilla client on 31% of
  ticks in session 1 and 5% in session 2. Almost all echo the client's own state to ~8e-6 (X axis),
  including while standing still: corrections there are not physics disagreements.
- Knockback: `SetActorMotion` stamped tick K arrives a couple of ticks late; the client rewinds and
  applies it on input tick K+1 (vertical 0.4 → 0.314, 0.229, 0.146 …). Matches
  `acacia-bot/src/movement/mod.rs` knockback handling. 34 of 122 session-2 corrections fall within 6
  ticks after a knockback, even for vanilla.
- Input flag sequences per jump/sprint/sneak/swim are in the capture (`mitm_inspect`).

## Login (session 3, raw Login logged structurally by tools/mitm/login.go)
- Envelope `{"AuthenticationType":0,"Token":<MultiplayerToken RS256>}`: no legacy `Certificate` chain.
- Client data: 46 claims in byte order of their names; `ProfileHash`, `SelfSignedId` and the platform ids
  empty; no `PlayFabId`/`PartyId`/`IsPartyLeader`.

## Not captured yet
- Android (persona values come from public ClientData dumps, not a capture).
- Skeleton arrows; a non-fatal fall.
