# NetherNet LAN + services signaling (client side)

Scope: joining via LAN discovery (UDP 7551) and via the Minecraft signaling service (Realms, friends'
worlds). Direct-connect HTTP signaling: nethernet-wire.md. Researched 2026-10-02 for 1.26.5x.

## Sources (shallow clones under the session scratchpad; licences checked)

| Prefix | Repo @ commit (date) | Licence | Use |
|---|---|---|---|
| GN | df-mc/go-nethernet `215e464` (2026-09-29) | MIT | LAN discovery, signal text, dialer |
| GT | Sandertv/gophertunnel `80c811b` (2026-09-28) | MIT | Realms API, service discovery, session/start. **No WS signaling** in any branch |
| LF | lactyy/gophertunnel `feature/p2p` `5b3803f` (2026-05-12) | MIT | WS signaling (legacy + JSON-RPC), friends worlds. Older GN API |
| XS | df-mc/go-xsapi `58a99d3` (2026-09-25) | MIT | MPSD handles/join |
| PJ | PrismarineJS/bedrock-protocol 3.60.1 `69c5141` (2026-09-22) | MIT | WS signaling, Realms→NetherNet selection |
| NN | PrismarineJS/node-nethernet 1.1.2 `077e86a` | MIT | LAN wire, trickle |
| PR | PrismarineJS/prismarine-realms 1.6.0 `39787cc` (2026-04-14) | MIT | Realms REST |
| PA / PX | prismarine-auth 3.1.1 `b795199` / prismarine-xbox-services `052a967` | MIT | MCToken, MPSD |
| NS | df-mc/nethernet-spec `6671c66` (2024-01-01) | **none (no LICENSE)**: read only | partly stale |
| MOJ | Mojang/bedrock-protocol-docs `d702218` (2026-09-30) | All rights reserved (EULA): read only | documents HTTP signaling only |

Do not consult Pumpkin (GPL). MOJ has nothing on LAN, WS signaling, Realms or TURN (grep of
additional_docs/, developer_notes/, changelogs); only `DisconnectFailReason` names
(MOJ json/DisconnectFailReason.json:83-152).

## 1. LAN

### 1a. Packet
- UDP **7551**, broadcast `255.255.255.255:7551`, IPv4 only (GN discovery/listener.go:59-66,89; NN src/client.js:13-14,403).
  Client socket: ephemeral port, SO_BROADCAST (NN client.js:37,43-45). Vanilla differs (source port 7551, subnet
  broadcast + `ff02::1`): qos-lan-capture-2026-10-03.md.
- Datagram = `HMAC-SHA256(32) ‖ AES-256-ECB-PKCS7(plaintext)` (GN discovery/packet.go:37-46; NN src/util.js:46-53).
- Key = `sha256(ef be ad de 00 00 00 00)` (u64 LE 0xdeadbeef); same key for AES and HMAC
  (GN discovery/crypto.go:14,18; NN src/crypto.js:3-24).
- HMAC covers the **plaintext**, length prefix included. Receive: decrypt, recompute, compare, drop <32 B
  (GN packet.go:43-45,55-64; NN util.js:55-65).
- Plaintext: `u16le length ‖ u16le packet_id ‖ u64le sender_id ‖ 8×00 ‖ body` (GN packet.go:158-161).
  - Length = whole plaintext incl. its own 2 B, as GN writes it (vanilla request: 20; qos-lan-capture-2026-10-03.md).
    NN's "excludes the 2 B" is wrong. Ignore on read.
  - GN rejects trailing bytes (packet.go:88); strings ≤65535 (packet.go:122).

| id | Name | Body |
|---|---|---|
| 0 | DiscoveryRequest | empty (GN packet_request.go:8-17) |
| 1 | DiscoveryResponse | `u32le len ‖ hex-ASCII(ServerData)` (GN packet_response.go:20-38; NN server.js:280-292) |
| 2 | DiscoveryMessage | `u64le recipient_id ‖ u32le len ‖ signal text` (GN packet_message.go:24-47) |

- Message quirk: vanilla hosts under-declare the CONNECTRESPONSE length; read to end of datagram
  (GN packet_message.go:24-47; NN util.js:70-77).

### 1b. ServerData (version 7)
Wire order (GN discovery/server_data.go:69-85,160; PJ src/nethernet/advertisement.json): `u8 version=7`,
`ServerName str`, `Protocol zz32`, `Version str`, `LevelName str`, `PlayerCount zz32`, `MaxPlayerCount zz32`,
`GameType zz32`, `EditorWorld bool`, `Hardcore bool`, `AcceptsOnlineAuth bool`, `AcceptsSelfSignedAuth bool`,
`Nonce str`, `ConnectionType zz32` (vanilla 4).
`str` = varuint32 len + bytes; `zz32` = zigzag varint32 (GN discovery/binary.go:22-68).
Older v4 layout (u8-len strings, li32 counts): PJ advertisement.js:40-49. NS:54-63 is stale.

### 1c. Flow + timing
1. Bind, broadcast Request now and every **2 s** (GN listener.go:402-419; NN client.js:62-66).
2. Server answers Response **unicast** to the requester (GN listener.go:277-287). Ignore own sender id (:244).
3. Pick a sender id → remote NetworkID. Send `CONNECTREQUEST` in a Message to the address last seen for that
   id; peer entries expire after 15 s idle (GN listener.go:133-158,426-430).
4. Receive: drop if `recipient_id` ≠ ours, or data `"Ping"`/empty (GN listener.go:303-307; NN client.js:360-369).
- No TURN/STUN on LAN: `Credentials()` = nil (GN listener.go:184-191).
- Timeouts: 15 s waiting for a response → CONNECTERROR 14; 5 s from answer to open → 9
  (NN client.js:16-17,210,429; GN dial.go:131-134,239-241,297-299).
- Own NetworkID and ConnectionID: random u64 each (GN listener.go:41-42, dial.go:145-146). New connection id per retry (NN client.js:424).

### 1d. Signal text (shared by LAN and WS)
`<TYPE> <connId u64 decimal> <data>`, split on the first two spaces (GN signal.go:125-136,173-181; NN src/signalling.js:46-54).

| Type | data |
|---|---|
| CONNECTREQUEST | SDP offer |
| CONNECTRESPONSE | SDP answer |
| CANDIDATEADD | one `candidate:…` line (libwebrtc format, GN signal.go:187-232) |
| CONNECTERROR | decimal i32 code 0..37 (GN signal.go:153-156,234-275; names in our `error.rs`) |

- Connection id parsed as a decimal prefix (GN signal.go:131,158-169). Unknown types are rejected (:141-151).
- **Trickle is the norm**: offer sent right after setLocalDescription; each candidate a separate CANDIDATEADD,
  both directions (GN conn.go:643-688; NN client.js:243-250,296-314; NN server.js:150-153). Remote candidates go in at
  mid `0`, mLineIndex 0 (NN client.js:190). Embedding all candidates (GN `DisableTrickleICE`, dial.go:206-212) is
  legal, but it is not vanilla's LAN behaviour (unconfirmed for 1.26.5x).
- `a=identity`: GN rejects answers without one unless `AllowIdentitylessServer` (GN dial.go:61-68,279-281).
  MOJ r26u4: "LAN secret is now required for self-signed auth on client-hosted games"
  (MOJ legacy_changelogs/changelog_2168_07_07_26.md:68). Unknown what it means on the wire; needs a capture.

### 1d'. Verified live 2026-10-02 (Windows 1.26.52 hosting a world, `examples/lan_join.rs`)
- Discovery answered (ServerData v7, protocol 2193); signals over Messages worked as specced.
- **The game-hosted answer has no `a=identity`** (BDS always asserts one): LAN joins accept an
  identityless answer (`Connection::allow_identityless_host`); our offer still carries the MultiplayerToken
  identity and was accepted. No "LAN secret" was needed for an online-auth join.
- ICE connected 0.4 s after the offer on host candidates; spawned ~6 s later.

### 1e. BDS 26.5x
- `enable-lan-visibility=true` (default) makes BDS answer LAN discovery
  (https://learn.microsoft.com/en-us/minecraft/creator/documents/bedrockserver/server-properties). With
  `transport=nethernet` it stops answering RakNet LAN pings on 19132 (https://mojira.dev/BDS-23111).
- BDS-23108 (open): 7551 discovery works but the handshake fails, apparently because there are no routable SDP
  candidates (https://mojira.dev/BDS-23108). Expect trouble testing BDS over LAN; same-host loopback may still work.
- Test config: `transport=nethernet`, `enable-lan-visibility=true`, `online-mode=true|false`.

## 2. Realms

**Verified live 2026-10-02** (`realm_join` example, a `NETHERNET_JSONRPC` realm in UKSouth): joined and
spawned in ~9 s. Where this contradicts the references below, these win:
- Signaling host = discovery `signaling.prod.serviceUri` (`wss://signaling-tm-<region>…`, Azure Traffic Manager;
  `signal.franchise…` is a CNAME to it); vanilla takes `signalingUri` (+ `pingFrequency` 48 s) from
  `signaling-afd…/api/v1.0/configuration`. PJ's `signal-<regionName>` exists only for some regions
  (`signal-westeurope` resolves, `signal-uksouth` doesn't), so don't derive it from the realm's region.
- The service sends JSON-RPC requests with **numeric** ids (`"id":2`); echoing a string id gets the socket closed
  (`1000 Closing connection`) within ~25 ms. Echo the id verbatim.
- `Signaling_DeliveryNotification_V1_0` arrives wrapped in `Signaling_ReceiveMessage_v1_0`; ack the outer id.
- TurnAuth gives `relay.communication.microsoft.com:3478` (stun + turn), expiry ~7 days. Its Allocate answers
  **300 Try Alternate** with ALTERNATE-SERVER; follow it (re-allocate unauthenticated there).
- The realm answered with CONNECTRESPONSE ~230 ms after the offer; ICE connected ~0.5 s after it.
- `pocket.realms.minecraft.net` serves list/invite/join; discovery also names `bedrock.frontendlegacy.realms…`
  and `frontend.realms…` (which one vanilla uses is still unknown).

### 2a. Realms REST
- Base: GT `https://bedrock.frontendlegacy.realms.minecraft-services.net` (GT minecraft/realms/realms.go:33-34);
  PR `https://pocket.realms.minecraft.net` (PR src/constants.js:2-6). **The vanilla host is unknown; capture it.**
- Auth: `Authorization: XBL3.0 x=<uhs>;<xsts>`, XSTS RP **`https://pocket.realms.minecraft.net/`**
  (GT realms.go:298,342; PR src/util.js:6-8).
- Headers: `User-Agent: MCPE/UWP`, `Client-Version: <game version>` (GT realms.go:333-334; PR src/rest.js:52-79).
  On `400 unknown_client_version`, GT renegotiates via `GET /mco/client/compatible` (GT clientversion.go:16-51).
  PR sends `Content-Type: application/json` even on GETs (a bug). Neither is proven vanilla.

| Call | Endpoint | Ref |
|---|---|---|
| list | `GET /worlds` → `{servers:[…]}` | GT realms.go:379-397; PR src/index.js:24-26 |
| world | `GET /worlds/{id}` | PR index.js:19-21 |
| invite lookup | `GET /worlds/v1/link/{code}` (strip `https://realms.gg/`) | GT realms.go:363-376; PR src/bedrock/api.js:13-19 |
| invite accept | `POST /invites/v1/link/accept/{code}` → world | PR api.js:76-81 |
| join | `GET /worlds/{id}/join`; 503 = retry (GT polls 3 s), 403 = not a member, 404 = missing | GT realms.go:163-191; PR rest.js:30,96-100 |

- Join response: `{address, networkProtocol, pendingUpdate, sessionRegionData{regionName, serviceQuality}}`
  (GT realms.go:142-150). `networkProtocol` ∈ `DEFAULT` (address = `host:port`, RakNet), `NETHERNET`,
  `NETHERNET_JSONRPC` (GT network_protocol.go:8-12).
- PJ handles only `NETHERNET_JSONRPC`: remote id = `address`, JSON-RPC signaling on host
  `signal-<regionName lowercased>.franchise.minecraft-services.net`. Anything else is RakNet `host:port`
  (PJ src/client/auth.js:140-156). There is no reference code for `NETHERNET`; presumably `address` = NetherNetId over legacy WS.

### 2b. Token for signaling
- MCToken = `authorizationHeader` from `POST <auth.serviceUri>/api/v1.0/session/start`, the PlayFab-ticket flow we
  already run (GT service/token.go:136,401,479-480; PA MinecraftBedrockServicesManager.js:28-50). Its `pmid` claim
  is the player's messaging id for JSON-RPC (GT service/token.go:456-460).
- Signaling env: discovery `GET https://client.discovery.minecraft-services.net/api/v1.0/discovery/MinecraftPE/builds/<ver>`
  → `result.serviceEnvironments.signaling.prod` = `{serviceUri, stunUri, turnUri}` (GT service/discovery.go:61-123;
  LF service/signaling/dial.go:~97-106). Default host `signal.franchise.minecraft-services.net` (PJ src/nethernet/signalling.js:106).

### 2c. WebSocket, legacy (ConnectionType 3)
- `wss://<host>/ws/v1.0/signaling/<ownNetworkId>`; headers `Authorization: <MCToken>`, PJ also sends `session-id: <ownNetworkId>`,
  `request-id: <ms epoch>` (PJ signalling.js:113-114; LF dial.go:61-78). No User-Agent set by either.
- Message `{Type, From, To, Message, MessageId}` (LF message.go:14-35):

| Type | Dir | Meaning |
|---|---|---|
| 0 | C→S | ping `{Type:0}` |
| 0 | S→C | error, Message `{"Code":1 PlayerNotFound \| 2 DeliveryFailure,"Message"}` |
| 1 | both | signal: send `{Type:1,To:<id as JSON number>,Message:<signal text>,MessageId:<uuid>}`; receive `From` = peer id |
| 2 | S→C | credentials, `From:"Server"`, Message = JSON string (2e) |
| 3 / 4 | S→C | accepted / delivered `{MessageId,ToPlayerId,DeliveredOn,AcceptedOn}` |

(LF message.go:41-129, conn.go:46-68,168-196; PJ signallingCodec.js:84-86, signalling.js:230-257.)

### 2d. WebSocket, JSON-RPC (ConnectionType 7, default for current Realms and friends)
- `wss://<host>/ws/v1.0/messaging/connect`, same `Authorization`; PJ sends random-UUID `session-id`/`request-id`
  (PJ signalling.js:110-111; LF messaging/dial.go:68-75).
- On open: call `Signaling_TurnAuth_v1_0` → credentials (PJ signalling.js:156-161).
- Send: `{jsonrpc:"2.0",id,method:"Signaling_SendClientMessage_v1_0",params:{toPlayerId,messageId,message:<string of
  {jsonrpc,method:"Signaling_WebRtc_v1_0",params:{netherNetId:<own>,message:<signal text>}}>}}`
  (PJ signallingCodec.js:65-80; LF messaging/conn.go:59-127).
- Receive: `Signaling_ReceiveMessage_v1_0` (single or array of `{From,Message}`). Ack `{id,result:null}`, also for
  `System_Pong_v1_0`, `Signaling_DeliveryNotification_V1_0` (PJ signalling.js:277-293).
- `toPlayerId`: LF = peer PMID (UUID) (LF messaging/conn.go:238-338); PJ passes the realm `address`.

### 2e. TURN/STUN
- Credentials `{"ExpirationInSeconds","TurnAuthServers":[{"Username","Password","Urls":[…]}]}` → ICE servers
  `{urls, username, credential}` (GN credentials.go:6-37; PJ signallingCodec.js:4-15). Cache for ExpirationInSeconds (LF).
- Gather srflx (STUN) and relay (TURN allocate) candidates and trickle them. GN `ICEGatherPolicy` can force relay (dial.go:75,83).

### 2f. Keepalive, errors, ids
- Ping cadence differs: PJ 5 s for both variants (signalling.js:142-152); LF 15 s legacy (conn.go:141-158), 50 s
  `System_Ping_v1_0` JSON-RPC (messaging/conn.go). **Capture.**
- PJ reconnects on close 1006 (≤5 tries), any other close is fatal; 15 s connect and request timeouts (PJ signalling.js:23,42,96,167-170,203-214).
- Connect signaling **before** creating the offer (PJ src/client.js:118-119). Same random u64 own NetworkID in URL/header and signals (PJ client.js:101).
- Dial budget 15 s; CONNECTERROR fire-and-forget, 2 s (GN dial.go:131-134,348-363).

## 3. Friends / online worlds
- Shares 2c–2f; only the peer lookup differs.
- XBL3.0 auth with RP `http://xboxlive.com` (already in `acacia-auth`). Find sessions with
  `POST https://sessiondirectory.xboxlive.com/handles/query?include=relatedInfo,customProperties`
  `{type:"activity",scid:"4fc10100-5f7a-4470-899b-280835760c07",owners:{people:{moniker:"people",monikerXuid}}}`,
  `x-xbl-contract-version: 107` (PX src/xbox/client.js:45-49; XS mpsd/activity.go:27).
- `customProperties.SupportedConnections[]` = `{ConnectionType, HostIpAddress, NetherNetId, PmsgId}`. Type 3 → dial
  NetherNetId (legacy WS); 7 → PmsgId (JSON-RPC); 4 = LAN (LF p2p/world.go:122-201). PJ takes the first with a NetherNetId (PJ auth.js:100-107).
- Join: `POST handles/<id>/session` (XS mpsd/join.go:84) with an RTA connection id (PX src/xbox/session.js:38-53). Then wait for the host to publish
  our XUID in `Nonces` (LF p2p/client.go:73,156-166). Template `MinecraftLobby`, titleId 896928775 (PJ src/client/xboxSession.js:4-8).

## 4. Gap table
Layering per DESIGN.md: network-free bytes/state go in `acacia-nethernet` (or `acacia-auth` for tokens);
sockets/WS/HTTP go in `acacia-client`.

| Need | Have | Gap | Crate |
|---|---|---|---|
| Identity line, answer verify | `identity.rs`, `server_identity.rs` | none; LAN self-signed/"LAN secret" unknown | nethernet |
| Offer SDP | `sdp.rs::vanilla_offer` (1 host cand, embedded) | trickle variant: offer without candidates + CANDIDATEADD lines | nethernet |
| str0m conn | `conn.rs` (1 local cand, no remote trickle) | `add_remote_candidate` (str0m lib.rs:1391), local srflx/relay (`Candidate::server_reflexive/relayed`, is-0.11.1 candidate.rs:256,296) | nethernet |
| TURN client | none (str0m has no STUN/TURN client, lib.rs:1348-1350) | sans-IO STUN binding + TURN Allocate/Refresh/CreatePermission/ChannelBind/Send, wrap/unwrap | nethernet (state) + client (socket) |
| Signal text | none | `Signal{kind,conn_id,data}` parse/format, error codes (have names in `error.rs`) | nethernet |
| LAN packet codec | none | AES-ECB+HMAC, packets 0/1/2, ServerData v7/v4 | nethernet (crates: aes, hmac, sha2) |
| LAN socket | `transport.rs` (unicast) | broadcast socket, 2 s request timer, peer table | client |
| WS signaling msgs | none | legacy + JSON-RPC JSON codecs, credentials parse | nethernet |
| WS transport | none | tokio-tungstenite wss, ping timer, reconnect, SOCKS5 CONNECT | client |
| MCToken | `acacia-auth` `ServiceToken` built in `session_start` (online/minecraft.rs:119-167), **not exported** in `LoginCredentials` | expose MCToken + `pmid` claim | auth |
| Signaling env | discovery parses only `auth.prod` (online/minecraft.rs:37) | parse `signaling.prod` {serviceUri,stunUri,turnUri} | auth |
| Realms XSTS | XSTS per RP cache (online/client.rs:103-110) | request RP `https://pocket.realms.minecraft.net/` | auth |
| Realms REST | none | worlds/link/join client, 503 poll | client (+ tiny JSON types) |
| MPSD | none | handles/query, join, RTA | client; defer |
| Routing | `route.rs` Choice{RakNet,NetherNet} | `Target::{Address, Lan(id), Realm(id/code), Friend(xuid)}` → signaling kind | client |
| Login ServerAddress | `nethernet.rs::server_address` (URL form) | LAN/Realm value unknown: **capture** | client |

## 5. Fingerprint risks (need a vanilla capture)
- Realms host (pocket vs frontendlegacy), header set and order, `Client-Version` value, `User-Agent` (MCPE/UWP is Windows;
  Android persona likely `MCPE/Android`), presence of `is-prerelease`, `Accept-Language`, `Charset`.
- WS upgrade: headers (`session-id`, `request-id` formats; User-Agent `libhttpclient`?), TLS ClientHello (rustls vs
  platform), ping cadence (5/15/50 s), and whether vanilla calls TurnAuth on connect.
- Which signaling variant and region host vanilla Android uses for `NETHERNET` vs `NETHERNET_JSONRPC`.
- Trickle timing: number/order of CANDIDATEADD, srflx/relay presence, candidate `ufrag`/`network-cost`
  fields; whether the offer embeds candidates.
- LAN: "LAN secret" auth, Login `ServerAddress` for LAN/Realm joins, vanilla's Response/Message packets (the
  2026-10-03 capture only saw requests; length and cadence are settled there).
- DTLS handshake details (str0m/dimpl vs the game's OpenSSL stack) apply to every mode.
- Realm-side telemetry unknown: realms see XUID and IP; reusing one TURN credential set across bots would link them (bots-indistinguishable).

## 6. Local test plan
| Piece | Local BDS / offline | Needs real service |
|---|---|---|
| LAN codec | unit vectors: round-trip and GN-compatible bytes (key/HMAC known) | — |
| LAN discovery | BDS `enable-lan-visibility=true`, `transport=nethernet`; broadcast on loopback/LAN, decode ServerData | — |
| LAN signaling + trickle | BDS (BDS-23108 risk), or a go-nethernet `discovery` listener as a fake host (MIT) | — |
| Vanilla LAN capture | Windows client in LAN tab against BDS; capture 7551 with `examples/nn_record.rs` extended to UDP 7551 | — |
| Signal text/WS JSON codecs | unit tests from LF/PJ fixtures | — |
| WS client | local fake WS server replaying JSON | real `signal*.franchise` handshake + TurnAuth (any account, no realm) |
| TURN | local coturn (`turnserver`) + str0m relayed candidate | Mojang TURN creds |
| Realms REST + join | — | a Realm (owned or invited test account); a realm subscription or trial |
| Friends worlds | — | 2 accounts, host world on vanilla client |
| Fingerprint | mitm the HTTPS/WSS of a vanilla client (proxy + cert) | vanilla client joining a Realm |
