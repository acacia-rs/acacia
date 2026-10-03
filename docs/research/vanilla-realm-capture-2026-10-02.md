# Vanilla Realm join capture (Windows 1.26.52, 2026-10-02)

Capture `.testserver/realm-capture/20261002-204544/` (`https.flow`, `mitm.log`, `udp.pcapng`). Windows GDK client,
owner account, sleeping `NETHERNET_JSONRPC` realm in UKSouth (three 503s before 200). Redacted: tokens, XUIDs,
gamertag, public IP, realm/club/session ids. **t = seconds from the first Realms call** (19:45:53.344 UTC).

Capture caveats:
- mitmproxy's local redirector also proxied the game's **UDP**: on the wire the realm flow leaves from mitm's
  port (55265), not the game's (55259). Payloads are untouched. At t+39.1 mitm stopped, the game's own port
  appeared, and ICE survived the change (the realm re-learned it as prflx).
- pktmon logs each datagram 2 to 6 times. All counts below are deduplicated.
- The signaling WS and RTA were TLS passthrough, so only their TCP connect times are known. No disconnect is logged
  for either before mitm stopped (about t+38.5).
- Discovery, PlayFab, `/api/v1.0/session/start` (MCToken) and `signaling-afd` were **not** called in the window.
  They were cached from game start. A startup capture is needed for those.

## 1. HTTPS calls

### 1a. Timeline (OneCollector telemetry omitted: 16 POSTs to `self.events.data.microsoft.com`)

| t (s) | Host | Call | Status (ms) |
|---|---|---|---|
| −0.34 | rta.xboxlive.com (x.x.x.x, passthrough) | TCP/TLS connect (RTA WS) | — |
| 0.000 | R = bedrock.frontendlegacy.realms.minecraft-services.net | GET /invites/count/pending | 200 (144) body `0` |
| 0.001 | R | GET /roles/user/<xuid> | 200 `[]` |
| 0.001 | R | GET /worlds/<id>/configuration/tier | 200 |
| 0.002 | R | GET /worlds | 200 |
| 0.003 | R | GET /invites/all | 200 `{invites:[]}` |
| 0.003 | R | GET /worlds/<id>/configuration/tier (again) | 200 |
| 0.004 | R | GET /activities/live/players | 200 `{servers:[]}` |
| 0.214 | avty.xboxlive.com | POST /timelines/club/BulkUnreadCount | 200 |
| 0.251 | privacy.xboxlive.com | GET /users/xuid(<xuid>)/people/avoid | 200 |
| 0.259 | clubpresence.xboxlive.com | POST /clubs/<club>/users/xuid(<xuid>)/session?titleFamilyId=… `{"inGame":false}` | 204 |
| 0.270 | sessiondirectory.xboxlive.com | POST /handles/query?include=relatedInfo,customProperties (activity, people) | 200 |
| 0.359 | R | GET /subscriptions/<id>/details (owner only) | 200 |
| 1.078 | R | GET /worlds/<id> | 200 |
| **1.749** | R | **POST /worlds/<id>/join** | **503** (818) |
| 2.08 | UDP | RakNet unconnected pings to 7 featured servers :19132 (3 each, 5 s apart) | — |
| 3.98 | x.x.x.x:443 (West Europe Azure, passthrough) | TCP/TLS connect, **inferred signaling WS** | — |
| 5.723 | R | POST /worlds/<id>/join | 503 (159) |
| 8.718 | sessiondirectory | POST /handles/query (periodic, about 15 s) | 200 |
| 8.987 | R | POST /worlds/<id>/join | 503 (136) |
| **12.269** | R | **POST /worlds/<id>/join** | **200** (490), done at t+12.759 |
| 13.016 | clubpresence | POST …/session `{"inGame":true}` | 204 |
| **13.077** | authorization.franchise.minecraft-services.net | **POST /api/v1.0/multiplayer/session/start** | 200 (343) |
| 14.163 | UDP | first ICE check → realm (§3) | — |
| 23.442 | sessiondirectory | POST /handles/query | 200 |
| 23.506 / 23.512 | privacy | GET …/people/avoid, …/people/mute | 200 |
| 23.866–24.094 | userpresence.xboxlive.com | 4× POST /users/xuid(<xuid>)/devices/current/titles/current (`Menus`, then 3× `Realm_Survival`) | 200, `x-heartbeat-after: 300` |
| 24.167 | sessiondirectory | **PUT** /serviceconfigs/4fc10100-…/sessionTemplates/MinecraftLobby/sessions/<GUID> | 201 |
| 24.564 | sessiondirectory | POST /handles `{type:"activity",sessionRef,version:1}` | 201 |
| 24.975 | sessiondirectory | GET the MPSD session | 200 |
| 36.351 | R | GET /worlds/<id> | 200 |
| 37.496 | avty | GET /timelines/club/<club>/UnreadCount | 200 |

Connections: 7 parallel TCP+TLS connections to R at t−0.5. Only one carried requests (HTTP/2, multiplexed); the
other 6 closed idle at t+4.8. Realms, privacy, clubpresence, userpresence and session/start use HTTP/2.
sessiondirectory uses **HTTP/1.1** (`Connection: Keep-Alive`, `Host` last).

### 1b. Realms headers (every R call, this order)

| # | Header | Value |
|---|---|---|
| 1 | content-type | `application/json` (also on GETs) |
| 2 | authorization | `XBL3.0 x=<uhs>;<xsts>` (2473 chars, a different XSTS from the xboxlive.com calls) |
| 3 | user-agent | `libhttpclient/1.0.0.0` |
| 4 | charset | `utf-8` |
| 5 | client-ref | 40-hex (SHA-1 shaped, `54116bb5…`). Likely a per-build id: compare with a 2nd machine/Android build |
| 6 | client-version | `1.26.52` |
| 7 | x-clientplatform | `Windows` |
| 8 | x-networkprotocolversion | `2193` |
| 9 | content-length | POST only |

No `accept`, `accept-language`, `is-prerelease` or `cache-control`. Responses carry `request-id` (12 hex),
`x-azure-ref` and `x-cache: CONFIG_NOCACHE` (Azure Front Door).

### 1c. Join

- **POST** (not GET) with body (key order as sent, 1180 B):
  `{"joinIntention":"VANILLA","pingRegions":[{"latencyMs":<int>,"region":"<name>"}, …]}`. There are 28 entries in
  discovery `qos-beacons.prod` key order (alphabetical: australiaEast … westUs3). In this capture ukSouth was
  9 ms, westEurope/northEurope 17, franceCentral 18, and australiaSoutheast 292. Latencies come from PlayFab QoS
  beacons `mpsqosprod.<region>.cloudapp.azure.com`, measured before the window (cached). The same pingRegions
  body was reused on every retry.
- 503: body `Retry again later` (text, despite `content-type: application/json`), header `retry-after: 3`.
- Retries: 3× 503 then 200. Gap from 503 response end to next request: 3.156 / 3.105 / 3.145 s (= retry-after
  + 0.10–0.16 s). Request-to-request: 3.97 / 3.26 / 3.28 s.
- 200: `{"networkProtocol":"NETHERNET_JSONRPC","address":"<uuid>","pendingUpdate":false,"sessionRegionData":{"regionName":"UKSouth","serviceQuality":1}}`.

### 1d. multiplayer/session/start (per join, right after join 200)

- Header order: `content-type: application/json`, `authorization: MCToken <…>` (4840 chars),
  `user-agent: libhttpclient/1.0.0.0`, **`session-id: <uuid v4>`**, `content-length`. No `accept`.
- Body `{"publicKey":"<base64 P-384 SPKI, 160 chars>"}`. Response `{"result":{"signedToken":<JWT 1076>,"validUntil","issuedAt"}}`
  (4 h validity). The response echoes `session-id` and adds `x-ms-expires`, `service-version: 2026.9.23.2603`.

### 1e. Xbox services (not join-critical; visible to Xbox Live, not the realm)

| Service | UA | contract | Notes |
|---|---|---|---|
| clubpresence | `libhttpclient/1.0.0.0` | 1 | `accept-language: en-GB`. `inGame:false` on the realm screen, `true` 0.26 s after join 200 |
| avty | `libhttpclient/1.0.0.0` | 18 | — |
| privacy, userpresence | `XboxServicesAPI/2024.10.20250509.0 c` | 1 / 3 | `signature` header (104 chars), `accept-language: en-GB,en,en-GB,en` |
| sessiondirectory | `XboxServicesAPI/2024.10.20250509.0 c` (query) / `…0MultiplayerManager c` (session PUT/handles) | 107 | HTTP/1.1, `Signature` |

MPSD session PUT after spawn (t+24.2): `properties.system {joinRestriction:"followed", readRestriction:"followed", closed:false}`.
`custom` holds: Joinability `joinable_by_friends`, hostName, ownerId, `rakNetGUID:""`, version, `levelId:""`, worldName,
worldType `Survival`, protocol 2193, MemberCount, MaxMemberCount, BroadcastSetting 2, LanGame false,
isEditorWorld false, isHardcore false, **TransportLayer 2**, OnlineCrossPlatformGame true, CrossPlayDisabled false,
TitleId 0, `nonces:{}`, RealmId, `SupportedConnections:[{ConnectionType:7, HostIpAddress:"", HostPort:0, NetherNetId:<realm address>, PmsgId:<realm address>}]`.
`members.me`: `{constants.system{xuid, initialize:true}, properties.system{active:true, connection:<RTA conn id>, subscription{id, changeTypes:["everything"]}}}`.
This needs an RTA connection id, so vanilla holds an RTA WS.

## 2. Signaling WebSocket

| Item | Observed |
|---|---|
| Connect | TCP to x.x.x.x:443 at **t+3.98**, between join retries 1 and 2 (8.8 s before join 200). Host by elimination: a West Europe Azure IP. Discovery `signaling.prod.serviceUri` = `wss://signaling-tm-westeurope.franchise.minecraft-services.net` (CNAME → `signal-westeurope.franchise…`, a TM endpoint). The only other passthrough is rta.xboxlive.com (akadns) |
| TCP connections | 1 |
| Lifetime | Still open when mitm stopped (≈ t+38.5): ≥ 34 s, **≥ 24 s after DTLS**. The client keeps it for the session |
| Offer → first check | Offer not visible. session/start finished at t+13.42 and the first ICE check went out at t+14.16, so offer/answer took ≤ 0.74 s |
| Headers, ping cadence, TurnAuth | not visible (passthrough). Next capture: intercept `signaling-tm-*` |

(`signal-<region>` names do resolve. `signal-westeurope` is the TM target. Correct the "don't resolve" note in
nethernet-signaling.md §2 for at least `westeurope`.)

## 3. UDP

### 3a. STUN/TURN to Microsoft relay: **none**
There was no datagram to port 3478, `relay.communication.microsoft.com`, `turn.azure.com` or 20.202.x in the pcap
or in mitm's UDP log. That rules out Binding, Allocate, 300 Try-Alternate, CreatePermission, ChannelBind and
Refresh. The game socket's first datagram was the ICE check to the realm, so there was no srflx/relay gathering
on it. Vanilla offered **host candidates only** (inferred), and the realm learned the client's public address as
peer-reflexive from the client's checks.

### 3b. Game sockets during the window

| Port | Use |
|---|---|
| 55259 (IPv4) | **the realm session: one socket**, created about t+14.16 |
| 57482 (IPv4) | featured-server RakNet pings |
| 57483 (IPv6) | RakNet LAN ping to `ff02::1:19133` every 1.0 s, 33 B (stops in-game, about t+16.6) |
| 7551 (IPv6) | NetherNet LAN discovery to `ff02::1:7551` every 2.0 s, 64 B (stops in-game) |

### 3c. ICE with the realm (remote host candidate x.x.x.x:30007)

| Item | Vanilla |
|---|---|
| Path | direct: client host/prflx ↔ realm public host. No relay |
| Roles | client = ICE **controlling** + DTLS **server**; the realm sends the ClientHello (as in the LAN/BDS captures) |
| Client check attrs (all 22 identical in order) | USERNAME (4:4), GOOG-NETWORK-INFO (net-id 1, cost 10), ICE-CONTROLLING, **USE-CANDIDATE on every check**, PRIORITY 1845501695 (prflx 110 / local-pref 30 / comp 1), MESSAGE-INTEGRITY, FINGERPRINT. No SOFTWARE |
| Realm check attrs | USERNAME, GOOG-NETWORK-INFO (net-id 1, cost 0), ICE-CONTROLLED, PRIORITY 1853824767, MI, FP |
| Responses (both ways) | XOR-MAPPED-ADDRESS, MI, FP |
| Start pacing | checks at t+14.163, +14.210, +14.261, +15.440, +16.495 |
| Steady pacing | client every **2.57–2.77 s**, realm every 2.50 s. (The vanilla-capture-2026-10-01 BDS LAN trace showed about 25 s, so the cadence depends on the peer/session.) There was a 7 s silence (t+16.5 to +23.5) during world load, then a burst of checks and responses |
| Timing | join 200 → first check **1.40 s**; → realm ClientHello (first DTLS) **t+14.215 = 1.46 s**; server flight t+14.224 (647 B); final flight 546 B (NewSessionTicket, as in the LAN/BDS captures); first app data t+14.244; spawn presence t+23.9 |

## 4. Gap table (vanilla Windows vs ours)

The P column is priority for indistinguishability. Observer: R = Realms API, S = signaling service, T = Microsoft
TURN, H = realm host, X = Xbox Live, A = auth service.

| # | Item | Vanilla | Ours | Fix | P |
|---|---|---|---|---|---|
| 1 | Realms host | `bedrock.frontendlegacy.realms.minecraft-services.net` (discovery `realms_frontend_bedrock_legacy`) | `pocket.realms.minecraft.net` (`realms.rs:15`) | Use the discovery value; keep the XSTS RP | P0 (R) |
| 2 | Join method/body | `POST` + `{joinIntention:"VANILLA", pingRegions[28]}` | `GET`, no body | POST that body; same body on retries | P0 (R) |
| 3 | pingRegions | Real QoS latencies (UDP to `mpsqosprod.<region>.cloudapp.azure.com`), all 28 discovery regions, alphabetical | none | Measure QoS from the bot's egress (via its proxy) and cache it per egress. Never fabricate numbers that disagree with the IP's geo | P0 (R) |
| 4 | Realms headers | 8 headers in §1b order; UA `libhttpclient/1.0.0.0`, `charset`, `client-ref`, `x-clientplatform`, `x-networkprotocolversion`, content-type on GETs | `Authorization`, `Client-Version`, `User-Agent: MCPE/Android` | Replicate the order/set. The Android persona needs its own `x-clientplatform`/`client-ref` values: capture Android | P0 (R) |
| 5 | TURN/STUN use | **none**; host-only candidates, prflx discovered by the realm | TurnAuth creds → unauthenticated Allocate (+300 follow), srflx + relay CANDIDATEADDs | For Realms: skip STUN/TURN and trickle host only. Keep TURN as a fallback only after a direct failure (unverified whether vanilla ever does that) | P0 (T, S, H) |
| 6 | Signaling WS lifetime | Opened during join retries, **kept open** for the session | Opened after join 200, closed on `Open` (`signaling/mod.rs:150`) | Keep the WS (and its pings) until disconnect. Connecting before the join 200 is optional | P1 (S) |
| 7 | ICE check attrs | GOOG-NETWORK-INFO, USE-CANDIDATE on every check, local-pref 30 | none / separate nomination / 65534 | `is`/str0m patch | P1 (H) |
| 8 | ICE consent cadence | ~2.6 s controlling-side checks during the session | str0m defaults | Match 2.5 s + small jitter | P1 (H) |
| 9 | multiplayer/session/start | Per join, right after join 200; UA libhttpclient, **`session-id` uuid**, no `accept` | Cached with credentials; `Content-Type`, `Accept`, no UA, no session-id (`minecraft.rs:209-220`) | Fresh per join, vanilla header set; check whether WS `session-id` reuses this uuid (next capture) | P1 (A) |
| 10 | Pre-join Realms calls | invites/count/pending, roles/user, configuration/tier ×2, /worlds, invites/all, activities/live/players, (owner: subscriptions/details), then `GET /worlds/{id}` about 0.7 s before join | join only | Emit the member subset (no subscriptions/details) with UI-like spacing | P1 (R) |
| 11 | 503 retry | Honors `retry-after` (+0.1–0.16 s), same body | fixed 3 s sleep | Sleep `retry-after` + 100–160 ms jitter | P2 (R) |
| 12 | Post-join Realms | `GET /worlds/{id}` at about t+36 | none | Optional refresh after spawn | P2 (R) |
| 13 | clubpresence | `inGame:false` on the screen, `true` 0.26 s after join 200 | none | Optional; the realm owner sees club presence | P2 (X) |
| 14 | MPSD + userpresence + RTA | RTA WS; PUT MinecraftLobby session (ConnectionType 7, realm address) + handle; richPresence `Realm_Survival` | none | Defer. Friends would see the bot as not in a realm. Needs RTA | P2 (X) |
| 15 | HTTP transport shape | HTTP/2, 7 parallel TCP conns at screen open | reqwest pool (verify h2 ALPN) | Ensure h2 to R. Ignore the conn fan-out | P3 (R) |
| 16 | LAN/featured pings, telemetry | IPv6 LAN pings 1 s/2 s, featured-server pings, OneCollector | none | Ignore: not visible to the realm | P3 |

Open: WS upgrade headers, ping cadence, whether TurnAuth is called (intercept `signaling-tm-*`); Android values for
`x-clientplatform`/`client-ref`; whether vanilla ever falls back to TURN (block UDP to the realm and capture);
startup sequence (discovery, `signaling-afd` configuration, QoS beacons).
