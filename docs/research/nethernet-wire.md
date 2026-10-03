# NetherNet direct-connect wire spec (client side)

Scope: joining a BDS (`transport=nethernet`) by address. HTTP signaling on the server's TCP port, then a
WebRTC data channel. Researched 2026-10-01.

Sources (shallow clones, commit/date):
- **MOJ** = Mojang/bedrock-protocol-docs `d702218` (2026-09-30) `additional_docs/NetherNetOnboardingGuide.md`
  (web: mojang.github.io/bedrock-protocol-docs/guides/nether-net-onboarding-guide/). This is the authoritative source.
- **GN** = df-mc/go-nethernet `215e464` (2026-09-29). `endpoint/` is the HTTP signaling client and server.
- **GT** = Sandertv/gophertunnel `80c811b` (2026-09-29) `minecraft/`
- **NS** = df-mc/nethernet-spec `6671c66` (2024-01-01, RE of 1.20.50; covers LAN/WS signaling and is partly stale)
- **PJ** = PrismarineJS/bedrock-protocol `69c5141`. Supports LAN and Xbox signaling only, with no HTTP path
  (issue #830 confirms this is unimplemented).

## 1. HTTP signaling

`serverUrl` = `http(s)://host:port`. The port is the server's game port, served over TCP. GN rejects a URL that has
no port or has a path (GN endpoint/client.go:124-133).

**Discovery order (vanilla client)**: it sends `GET /v1/join` to each of `https://host:port`, `https://host`(443),
`http://host:port`, `http://host`(80) in turn and stops at the first 2xx. If an explicit port was given, only the
https and http variants on that port are tried. If none respond, the client **falls back to RakNet**
(GN endpoint/handler.go:159-165). Try https first, then http.

### 1a. `GET {serverUrl}/v1/join`: capability check and status
- Request: no body. GN sends `User-Agent: libhttpclient/1.0.0.0` (GN endpoint/client.go:103). No auth.
- Response: `2xx` means NetherNet is supported. Any other status (e.g. 404) means it is not, and the client
  does not attempt WebRTC (MOJ:123,153). `Content-Type: application/json`. Body:
  `{"name":str,"protocol":int,"version":"1.26.50","level":str,"players":int,"maxPlayers":int,"gameType":0|1|2}`
  (MOJ:127-151, GN endpoint/status.go:11-27). The GN server may return 200 with an **empty body** if no status
  has been set (GN endpoint/handler.go:333-337), so tolerate that.
- This is the NetherNet **pong/MOTD**. There is no RakNet unconnected ping. GN converts the status to the
  RakNet `MCPE;name;proto;ver;players;max;rand;level;GameType;gt;19132;19132;0;` string (GN endpoint/status.go:45-61).
- GN's server sets `req.Close=true`, so there is no keep-alive. Use a fresh TCP connection per request
  (GN endpoint/handler.go:327,349).

### 1b. `POST {serverUrl}/v1/join/{networkId}`: SDP exchange (the only signaling round-trip)
- `{networkId}` is the **client's own** NetworkID. MOJ calls it an opaque string; the current format is a
  decimal u64 (MOJ:637-649). GN's server **requires a u64** and returns 400 otherwise
  (GN endpoint/handler.go:358-362). GN's client uses a random u64 (GN endpoint/client.go:51-53).
- Headers: `Content-Type: application/sdp` (MOJ:171). GN also sends `User-Agent: libhttpclient/1.0.0.0`.
- Body: the raw UTF-8 SDP offer **with all ICE candidates embedded** (full ICE, no trickle) (MOJ:57,172).
- Response: `2xx` with `Content-Type: application/sdp` and the raw SDP answer carrying all of the server's
  candidates (MOJ:176-180). Any non-2xx is a failure with no retry, and a new attempt means a new POST
  (MOJ:184-187).
- **Trickle**: none. There are no candidate requests and no polling. The single response is the whole exchange.
  GN refuses candidate signals on this transport (GN endpoint/client.go:187-191,197-203).
- Error bodies from GN's server are `text/plain`: 400 for bad id or empty body, 413 if over 1 MiB,
  503 if not admitted, 502 on answer timeout, 500 with a decimal NetherNet error-code body
  (GN endpoint/handler.go:352-418). GN's client also treats a 2xx body that parses as a u32 as an error code
  (GN endpoint/client.go:174-176). Cap bodies at 1 MiB (GN endpoint/handler.go:134).
- **Timeouts**: GN's server waits 15 s for the answer (GN endpoint/handler.go:57-60). The default http.Server
  limits are 5 s for headers, 10 s for reads and 30 s idle (GN endpoint/handler.go:114-119). GN's dialer uses a
  15 s timeout for the whole dial (GN dial.go:131-135). Use a ~15 s HTTP timeout.
- **TLS**: both HTTP and HTTPS work. They change only how the client trusts the server's `a=identity`, not the
  wire format (§2).
- **Connection ID**: there is none on the wire for HTTP. It exists only on WS/LAN signaling (NS:106-114). GN uses a
  random u64 internally (GN endpoint/handler.go:450).
- **Auth**: there is no HTTP header. Authentication is the SDP `a=identity` (§2).

## 2. Identity assertion (`a=identity`)
This is a session-level line that goes **after `a=fingerprint` and before the first `m=`** (MOJ:240-251,386-399).
GN sometimes puts the fingerprint at media level and searches both levels (GN conn.go:517-523).
- Value: `base64std(JSON{"idp":{"domain":D,"protocol":"default"},"assertion":"<JSON string>"})`. The inner
  string is `{"token":JWT,"fingerprints":"<b64url(hdr)>..<b64url(sig)>"}`, a detached JWS (MOJ:208-236).
  Note that `assertion` is a JSON-encoded string, not an object (GN identity.go:271-289).
- Signed payload (canonical JSON with no whitespace):
  `{"fingerprint":[{"algorithm":"sha-256","digest":"AA:BB:..."}]}`, covering every `a=fingerprint` line in order
  (MOJ:263-288, GN identity.go:195-210). Digest hex is **upper-case** (GN conn.go:892-894).
- **Client offer**: `token` = MultiplayerToken / `GameServerToken` from the Minecraft auth service. It is RS256,
  its `cpk` claim is the client's P-384 login key (JWK in 26.40+, base64 DER before that), and it is bound to
  that key per connection (GN identity.go:76-82,306-334; GT multiplayer_token_source.go:10-20). JWS `alg=ES384`,
  signed with the **same ECDSA P-384 key used for Login** (GN identity.go:45-56; GT nethernet.go:49-61,
  GT dial.go:268,292). `idp.domain` = auth issuer URL with a trailing `/`, e.g.
  `https://authorization.franchise.minecraft-services.net/` (GT nethernet.go:58-59, GN identity.go:236).
  The server may reject an offer that has no identity, or allow it if it runs open/offline. This is server
  policy (MOJ:312-318).
- **Server answer**: always contains `a=identity` (MOJ:322-331). Its token is a self-signed ES384 JWT with a
  `cpk` JWK and an `x5u` header. BDS uses `domain:"self"` (GN identity.go:36-39,110-143). The client checks that
  the JWT is self-signed by `cpk`, verifies the fingerprint JWS with `cpk`, and checks `exp` (MOJ:403-414).
  Under HTTPS, TLS is the trust anchor. Under HTTP, the client uses TOFU and pins the `cpk` digest (MOJ:328-329).
  A bot can trust `cpk` blindly, which is GN's default (GN dial.go:154-163), but it should still verify the
  JWS binding.
- **Strip `a=identity` before handing the SDP to the WebRTC stack** (MOJ:277,414).

## 3. SDP offer contents
The template is MOJ:571-592. GN reproduces it in GN conn.go:763-843.
```
v=0 / o=- <rand u64> 2 IN IP4 127.0.0.1 / s=- / t=0 0
a=group:BUNDLE 0 / a=extmap-allow-mixed / a=msid-semantic: WMS
[a=identity:...]                        (session level, see §2)
m=application 9 UDP/DTLS/SCTP webrtc-datachannel
c=IN IP4 0.0.0.0                        (GN: default candidate addr/port when present)
a=candidate:... (host only; UDP) / a=ice-ufrag / a=ice-pwd / a=ice-options:trickle  (present even w/o trickle)
a=fingerprint:sha-256 <UPPER HEX> / a=setup:actpass / a=mid:0 / a=sctp-port:5000 / a=max-message-size:262144
a=end-of-candidates                     (MOJ example; GN omits)
```
- The client is **full ICE, ICE-controlling**, not ICE-lite (GN dial.go:369-373, NS:120). The server is
  ICE-controlled (GN listener.go:691-694).
- There are no TCP candidates and no STUN/TURN, so only host candidates are offered. Bundle policy is max-bundle
  (MOJ:432-440). For non-loopback joins the server must include a reachable srflx candidate (MOJ:553).
- DTLS: `setup:actpass` in the offer. The answer is `active` (MOJ:610,627), so **the server is the DTLS client
  and our client is the DTLS server** (GN listener.go:598-606). The fingerprint is sha-256.
- Attributes GN's parser requires in the answer: `ice-ufrag`, `ice-pwd`, `fingerprint`, `setup`, and
  `max-message-size` (>1). It also requires exactly 1 `m=` section (GN conn.go:502-570). Candidates may sit at
  session or media level, with or without an `a=` prefix (GN conn.go:572-582,445-448).
- GN's candidate format: `candidate:<f> 1 udp <prio> <ip> <port> typ host generation 0 ufrag <u> network-id <i>
  network-cost 0` (GN signal.go:~180-231).

## 4. Data channels
- **The client creates both channels after SCTP comes up.** They use in-band DCEP (`negotiated=false`, empty
  protocol), and the server only accepts them through `ondatachannel` (MOJ:442-454, GN dial.go:385-394,
  GN message.go:36-52). If the server opens a channel, the client closes the connection (GN dial.go:199-204).
  - `ReliableDataChannel`: ordered, reliable (no maxRetransmits/maxPacketLifeTime).
  - `UnreliableDataChannel`: **unordered**, `maxRetransmits=0`.
- The server validates label, protocol, ordered, negotiated, maxRetransmits and maxPacketLifeTime **exactly**
  (GN message.go:57-69, GN listener.go:524-526). Within 5 s of the first candidate, the server waits for both
  channels to be open, and it times out with code 15 if they are not (GN listener.go:640-643,709).
- Stream IDs are not specified. They follow standard DCEP parity from the DTLS role (RFC 8832). The client is the
  DTLS server, so it uses **odd** ids, e.g. 1 and 3 (inferred, see Open questions).
- In practice game traffic uses only Reliable. GN's Read and Write use Reliable only (GN conn.go:102-121,178-182),
  and its comments call Unreliable "seems to be unused" (GN message.go:23-28). MOJ says it carries
  movement-type data (MOJ:452).

## 5. Framing
**NetherNet layer** (MOJ:456-497, GN conn.go:184-230, GN message.go:156-209). Every SCTP message is
`[u8 remaining][payload]`.
- `remaining` = the number of fragments still to follow, counting down to `0` for the last fragment. A message
  that is not split sends `0x00`.
- Fragment payload size = negotiated `max-message-size - 1`, which is 262143 for vanilla's 262144
  (GN conn.go:491-494,385). At most 255 fragments.
- To reassemble, append payloads until you see `0`. A count that is not exactly previous−1 is fatal and closes
  the connection (GN message.go:190-192). Messages shorter than 2 bytes are rejected (GN message.go:159-161).
- Unreliable: the header must be `0`. A larger header is fatal (GN message.go:183-185), and an oversized message
  is dropped by the sender.
- NS's "10,000-byte segments" (NS:183-196) is **stale** (1.20.50). The current value is max-message-size−1.
  GN's doc comment still says 10000 (GN conn.go:178), but its code uses max-message-size−1.

**Minecraft layer inside one reassembled message**. This is the same as the RakNet batch except:
- **No `0xfe` batch header.** GN `BatchHeader()` returns nil (GN conn.go:145-148). GT's Encoder and Decoder use
  that instead of `0xfe` (GT protocol/packet/encoder.go:35-40, decoder.go:42-47). PJ: `batchHeader=null`
  (PJ src/client.js:67).
- **No encryption.** `DisableEncryption()` returns true (GN conn.go:150-157). GT's server skips
  ServerToClientHandshake entirely (GT conn.go:868-870), and its client accepts PlayStatus/ResourcePacksInfo
  straight after Login (GT dial.go:353-356). PJ: `disableEncryption=true` (PJ src/client.js:68,358-363).
  DTLS provides confidentiality.
- **Compression is the same as RakNet.** After NetworkSettings, each batch is `[u8 algo: 0x00 zlib(raw deflate)|0x01 snappy|0xff none]`
  followed by `varuint32 len + packet` repeated. Before NetworkSettings there is no algo byte
  (GT encoder.go:85-129, decoder.go:196-215).
- **Batching is allowed**: GT flushes several packets as one message (GT encoder.go:72-101). NS:182's "not
  batched" is the 1.20.50 client's habit, and the receiver must accept N packets per message.
- Login `ServerAddress` for URL-addressed joins: `https://host:port:port`, with the port repeated
  (GT dial.go:460-472, GT protocol/login/data.go:361-364).

## 6. Discovery and BDS behaviour
- For direct connect, the **client never learns a server NetworkID**. The URL is the address, and BDS keeps its
  own id internal (GN endpoint/handler.go:39-42,277-282). The `GET /v1/join` response has no network id
  (MOJ:127-137).
- BDS 26.5x with `transport=nethernet` answers `GET /v1/join` on its TCP game port with the status JSON. It was
  tested on 1.26.51.1 (protocol 2193) with `online-mode=false` (PJ issue #830, fetched 2026-10-01). GT's example
  dials a BDS at `http://localhost:19133` (GT example_nethernet_test.go:48-53).
- A RakNet-only server has nothing listening on that TCP port (connection refused) or answers non-2xx. The client
  then falls back to RakNet UDP (GN endpoint/handler.go:162-165).
- LAN discovery is separate: UDP 7551, AES-ECB/HMAC with key sha256(0xdeadbeef) (NS:11-71). It is only used when
  `enable-lan-visibility=true` (PJ #830). It is not needed for direct connect.

## 7. Timeouts, keepalive, close
- Dial budget is 15 s overall (GN dial.go:131-135). The server allows 15 s for the answer
  (GN endpoint/handler.go:57-60) and 5 s from the first candidate until both channels are open
  (GN listener.go:640-643).
- There is **no NetherNet-level keepalive or ping message**. Liveness comes from WebRTC itself: ICE consent
  STUN checks plus SCTP heartbeats. GN sets no custom ICE timeouts, so pion's defaults apply (disconnected 5 s,
  failed 25 s, keepalive 2 s). These are pion values, not Mojang's. Game-level liveness is the usual Bedrock
  packets (NetworkStackLatency, etc.).
- Close: closing either data channel, ICE entering `failed` or `closed` (not `disconnected`), DTLS entering
  `failed` or `closed`, or the SCTP association closing **tears down the whole connection**
  (GN conn.go:370-403, GN message.go:111-114). A graceful close closes both DCs, then SCTP, DTLS and ICE
  (GN conn.go:311-334). Send a Bedrock Disconnect packet first. There is no HTTP call on close.
- Signaling error codes are 0..37 (GN signal.go:234-274). Mojang's DisconnectFailReason NetherNet* values are
  offset (e.g. NetherNetInactivityTimeout=87) (MOJ legacy_changelogs/changelog_622_09_26_23.md:85-99).
  On HTTP, the only way to deliver an error code is in a response body.

## Answered by testing against BDS 26.5x (2026-10-01)
- Q2: a client `a=identity` is **required** even with `online-mode=false`, and it must carry a real
  MultiplayerToken (a self-signed token gets error 37 in a 200 body).
- Q3: BDS serves plain HTTP on the game port. `GET /v1/join` returns 200 with an **empty body** (no status JSON).
- Q8: BDS **does** send ServerToClientHandshake, but the client must **not** enable encryption afterwards.
- BDS's answer lists LAN and IPv6 host candidates; it reached a 127.0.0.1 client via peer-reflexive checks.
- The data channels str0m opened (ids 0 and 1) were accepted.

## Open questions
1. Exact DC stream IDs. Do vanilla client and BDS use DCEP odd/even ids by DTLS role, as is standard? Not in any source. Capture a real client.
2. Does BDS 26.50 **require** a client `a=identity` when `online-mode=false`? Does it validate the GameServerToken when `online-mode=true`? Is a bare `Login` chain enough? MOJ says this is server policy.
3. Does BDS serve both HTTP and HTTPS on the same port (sniffing), or plain HTTP only? It has no cert by default. MOJ and GN imply plain HTTP for raw IP:port.
4. Does BDS emit `a=end-of-candidates`, and a srflx candidate when it is behind NAT? MOJ:553 puts that on the operator.
5. How does vanilla use UnreliableDataChannel (which packets go on it, and if the server can send on it)? GN treats it as unused.
6. What timeouts does vanilla use for the HTTP request and ICE consent? The values here are GN/pion defaults.
7. Can BDS respond with a 2xx whose body is a bare error code, or does it always use non-2xx? GN's client handles both.
8. Does PlayStatus or ResourcePacksInfo always arrive with no ServerToClientHandshake on BDS NetherNet, so encryption is never negotiated? GT and PJ assume so, but neither shows a BDS capture.
