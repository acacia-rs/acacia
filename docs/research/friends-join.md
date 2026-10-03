# Joining a friend's world (client side)

Researched 2026-10-03 for 1.26.5x (protocol 2193). No vanilla capture yet: everything below is from MIT
reference code, so items marked **capture** are fingerprint guesses. Signaling (WS, JSON-RPC, TURN) is
shared with Realms: nethernet-signaling.md §2c-2f.

## Sources (shallow clones in the session scratchpad; all MIT)

| Prefix | Repo @ commit (date) | Use |
|---|---|---|
| XA | microsoft/xbox-live-api `8486ddd` (2026-08-14) | XSAPI C++ the game links: headers, RTA, MPSD writes |
| XS | df-mc/go-xsapi `58a99d3` (2026-09-25) | MPSD activity query, join, RTA |
| LF | lactyy/gophertunnel `feature/p2p` `5b3803f` (2026-05-12) | Minecraft custom properties, nonce, connection pick |
| GT | Sandertv/gophertunnel `80c811b` (2026-09-29) | `ClientData.Nonce` doc |
| PJ | PrismarineJS/bedrock-protocol `69c5141` (2026-09-22) | world join flow, title constants |
| PX | PrismarineJS/prismarine-xbox-services `052a967` (2026-09-20) | MPSD + RTA client |
| BP | LucienHH/bedrock-portal `c073809` (2026-09-16) | host side, peoplehub endpoints |

## 1. Tokens

- Every call below: XSTS for RP **`http://xboxlive.com`** (SISU's authorization token), `Authorization: XBL3.0
  x=<uhs>;<token>`, plus `Signature` (proof key, policy 1). XSAPI/go-xsapi sign every NSAL-matched URL with the
  default auth policy when no `SignaturePolicyIndex` is given (XS xal/nsal/title.go:183-201).
- Signaling: the MCToken (session/start), as for Realms. Login: the usual chain + multiplayer token.

## 2. Headers (XSAPI `XblHttpCall::Init`, XA Source/Shared/http_call_wrapper_internal.cpp:545-630)

`x-xbl-contract-version` (MPSD: `107`), `Content-Type: application/json; charset=utf-8`, `Accept-Language: <locales>`,
`User-Agent: XboxServicesAPI/<XSAPI version> c` (C API; XA build_version.h is `2025.10.20251000.0`; **capture** the
Android build's), `Authorization`, `Signature`. MPSD writes disable retries (multiplayer_service.cpp:992).

## 3. Listing friends' worlds

- `POST https://sessiondirectory.xboxlive.com/handles/query?include=relatedInfo,customProperties`, contract 107
  (XA multiplayer_service.cpp:16; XS mpsd/activity.go:25-40; PX src/xbox/client.js:43-51), body
  `{"type":"activity","scid":"4fc10100-5f7a-4470-899b-280835760c07","owners":{"people":{"moniker":"people","monikerXuid":"<own xuid>"}}}`.
  `owners.xuids` and `owners.people` together → 400 (XS activity.go:279-281).
- Response `{"results":[ActivityHandle]}`: `id` (handle id), `sessionRef{scid,templateName,name}`, `ownerXuid`,
  `titleId`, `createTime`, `relatedInfo{closed,joinRestriction,maxMembersCount,membersCount,visibility,postedTime}`,
  `customProperties` (XS activity.go:195-262).
- Title: SCID above, template `MinecraftLobby`, titleId `896928775` (PJ src/client/xboxSession.js:4-8).
- Social graph: `moniker:"people"` is the caller's followed list, so no separate friends call is needed. The
  in-game list decorates people via peoplehub `GET https://peoplehub.xboxlive.com/users/me/people/social/decoration/detail,preferredColor,follower`
  contract 5 (BP src/rest.ts:169); presence `userpresence.xboxlive.com`. Whether vanilla calls them before the
  handle query is **capture**; not implemented.

### customProperties (Minecraft, LF minecraft/p2p/world.go:17-141; PJ xboxSession.js:45-82)
`hostName`, `ownerId` (xuid), `worldName`, `version`, `levelId`, `worldType`, `protocol` (int), `MemberCount`,
`MaxMemberCount`, `Joinability` (`invite_only`/`joinable_by_friends`), `BroadcastSetting` (1 invite, 2 friends, 3
friends of friends), `TransportLayer` (0 RakNet, 2 NetherNet), `LanGame`, `isEditorWorld`, `isHardcore`,
`OnlineCrossPlatformGame`, `CrossPlayDisabled`, `TitleId`, `rakNetGUID`, `SupportedConnections[]`, `nonces{xuid:nonce}`.

`SupportedConnections[]` = `{ConnectionType, HostIpAddress, HostPort, NetherNetId, PmsgId}`:

| ConnectionType | Meaning | Dial |
|---|---|---|
| 7 | JSON-RPC signaling (default; live 1.26.51 hosts advertise it, PJ auth.js:100-101) | `toPlayerId` = `PmsgId` (UUID) (LF world.go:165-175) |
| 3 | legacy WS signaling (host's `mc-host-json-rpc=false`) | `To` = `NetherNetId` |
| 4 | LAN (only in LAN ServerData) | — |
| 0/1 | RakNet `HostIpAddress:HostPort` (dead for normal worlds) | — |

`NetherNetId` may be a JSON number or string (u64; LF uses `json.Number`). PJ ignores `PmsgId` and sends to the
NetherNetId over legacy WS (auth.js:102); which the service accepts for a type-7 host is unverified.

## 4. Joining

1. **RTA** (XA Source/Services/RealTimeActivityManager/real_time_activity_connection.cpp:10-11,355-650;
   web_socket.cpp:78-83): `wss://rta.xboxlive.com/connect`, subprotocol `rta.xboxlive.com.V2`, headers
   `Authorization`, `Signature` (GET, path `/connect`), `Accept-Language`, `User-Agent: XboxServicesAPI/<ver>` (no ` c`).
   PX instead fetches `GET https://rta.xboxlive.com/nonce` and dials `?nonce=` (browser WS can't set headers).
   Frames are JSON arrays:

   | Dir | Frame |
   |---|---|
   | C→S | `[1, seq, "<uri>"]` subscribe; `[2, seq, subId]` unsubscribe |
   | S→C | `[1, seq, status, subId, data]`; `[2, seq, status]`; `[3, subId, data]` event; `[4]` resync |

   Status 0 ok, 1 unknown resource, 2 limit, 3 no data, 1001 throttled, 1002 unavailable (PX src/rta/constants.js:2).
   Subscribe `https://sessiondirectory.xboxlive.com/connections/` → data `{"ConnectionId":"<guid>"}`
   (XA multiplayer_subscription.cpp:9,63). Events on it are shoulder taps for sessions we are in.
2. **Join by handle**: `PUT https://sessiondirectory.xboxlive.com/handles/<handle id>/session`, contract 107. XSAPI's
   lobby join uses write mode UpdateOrCreateNew (no match header; multiplayer_lobby_client.cpp:907-910,
   multiplayer_service.cpp:995-1011); go-xsapi sends `If-Match: *` and retries 412 (XS mpsd/join.go:84-127). Body
   (XS join.go:57-80; PX session.js:49-56):
   `{"members":{"me":{"constants":{"system":{"xuid":"<own>","initialize":true}},"properties":{"system":{"active":true,"connection":"<ConnectionId>","subscription":{"id":"<UPPERCASE uuid>","changeTypes":["everything"]}}}}}}`.
   200 → session document; `Content-Location` names the session (same as the handle's `sessionRef`).
3. **Activity**: `POST https://sessiondirectory.xboxlive.com/handles` `{"version":1,"type":"activity","sessionRef":{scid,templateName,name}}`
   (XA multiplayer_lobby_client.cpp:1039-1044; PJ auth.js:106) so our friends see where we are.
4. **Nonce**: the host notices the new member and writes `customProperties.nonces[<our xuid>]`; the client waits
   for it (RTA tap → `GET` the session) and sends it as `ClientData.Nonce` (GT protocol/login/data.go:255-260;
   LF p2p/client.go:141-172). It stops clients that only know the host's PMID. PJ skips this.
5. **Dial**: signaling host from discovery `signaling.prod.serviceUri` (as Realms), peer per §3 table, the usual
   NetherNet offer/trickle. Login `ServerAddress` for this case: **capture**.
6. **Stay / leave**: the member stays active only while its RTA connection lives; on reconnect the new ConnectionId
   is written with `{"members":{"me":{"properties":{"system":{"active":true,"connection":"<id>"}}}}}` (PX
   session.js:121-124). Leave: `PUT` the session `{"members":{"me":null}}` (PX session.js:177-180).

Session URL: `https://sessiondirectory.xboxlive.com/serviceconfigs/<scid>/sessionTemplates/<template>/sessions/<name>`.

## 5. Fingerprint risks (capture)
- Whether vanilla Android joins by handle or by session name, and its match headers.
- XSAPI version in `User-Agent`; `Accept-Language` value; any member `custom` constants/properties vanilla writes.
- peoplehub/presence calls around the friends tab; RTA renewal cadence; whether vanilla waits for the nonce.
- Login `ServerAddress` and the signaling peer id for type-7 hosts (PMID vs NetherNetId).
