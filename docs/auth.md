# acacia-auth

Ported from gophertunnel (commit 80c811b, 2026-09-29) + go-xsapi v2.0.3 + go-playfab v2.0.2;
cross-checked with prismarine-auth 2.7.0. "Live-verified" = exercised against the real endpoint.

## Features
- default `online`: `AuthClient`, `Account`, `TokenCache` (+ `FileTokenCache`, `MemoryTokenCache`).
- `--no-default-features`: `ClientData`, `build_connection_request`, `build_offline_connection_request`,
  `public_key_der_b64`, `parse_server_handshake`, `jwt`, `login::verify` (no reqwest/tokio). `socks` adds `socks5://` proxies.
- Crypto: RustCrypto `p384 0.13` (client key, ES384), `p256 0.13` (Xbox proof key) and `rsa 0.9` (RS256 verification only).

## Titles (`Title`)
| | client_id | title id | User-Agent | device |
|---|---|---|---|---|
| Android (default, gophertunnel) | 0000000048183522 | 1739947436 | `XAL Android 2025.04.20250326.000` | Android / 13 |
| NintendoSwitch (prismarine) | 00000000441cc96b | 2047319603 | `XAL` | Nintendo / 0.0.0 |

Online logins are Android-titled, so `ClientData.DeviceOS` stays 1 (Android) as gophertunnel forces.

## Token chain
1. **MSA device code** (live-verified, both client IDs): form `POST https://login.live.com/oauth20_connect.srf`
   `client_id, scope=service::user.auth.xboxlive.com::MBI_SSL, response_type=device_code`.
   Poll `POST https://login.live.com/oauth20_token.srf` `client_id, grant_type=urn:ietf:params:oauth:grant-type:device_code,
   device_code, scope`; `authorization_pending` / `slow_down` (+5 s) / `expired_token`. Refresh: same URL,
   `grant_type=refresh_token, refresh_token, scope, client_id`, `User-Agent` = title UA.
2. **Device token** (live-verified): `POST https://device.auth.xboxlive.com/device/authenticate`, headers
   `User-Agent`, `Content-Type: application/json`, `x-xbl-contract-version: 1`, `Signature`. Body
   `{RelyingParty:"http://auth.xboxlive.com", TokenType:"JWT", Properties:{AuthMethod:"ProofOfPossession",
   Id:"{uuid}", DeviceType, Version, ProofKey:{use:"sig",kty:"EC",crv:"P-256",alg:"ES256",x,y}}}`.
3. **SISU**: `POST https://sisu.xboxlive.com/authorize` (UA + Signature, no contract header) body
   `{AccessToken:"t=<msa>", AppId:client_id, DeviceToken, ProofKey, RelyingParty:"http://xboxlive.com",
   Sandbox:"RETAIL", SiteName:"user.auth.xboxlive.com", UseModernGamertag:true}` → Title/User/Authorization tokens.
4. **Relying parties** via NSAL like go-xsapi: `GET https://title.mgt.xboxlive.com/titles/current/endpoints`
   (xboxlive.com XSTS + Signature), then `titles/default/endpoints?type=1`; fqdn beats wildcard.
   Fallbacks: `https://multiplayer.minecraft.net/`, `http://playfab.xboxlive.com/`.
5. **XSTS** per RP: `POST https://xsts.auth.xboxlive.com/xsts/authorize` (headers as 2) body
   `{RelyingParty, TokenType:"JWT", Properties:{SandboxId:"RETAIL", DeviceToken, TitleToken, UserTokens:[user]}}`.
   `Authorization: XBL3.0 x=<uhs>;<token>`.
6. **Mojang chain**: `POST https://multiplayer.minecraft.net/authentication`, `User-Agent: MCPE/Android`,
   `Client-Version: 1.26.50`, XBL3.0 auth, no Signature, body `{"identityPublicKey":<client SPKI b64>}` → `{"chain":[2 JWTs]}`.
7. **Discovery** (live-verified): `GET https://client.discovery.minecraft-services.net/api/v1.0/discovery/MinecraftPE/builds/1.26.50`
   → `result.serviceEnvironments.auth.prod` = `{serviceUri: https://authorization.franchise.minecraft-services.net, playfabTitleId: 20CA2}`.
8. **PlayFab**: `POST https://20ca2.playfabapi.com/Client/LoginWithXbox` (XBL3.0 auth + Signature for the PlayFab RP)
   body `{TitleId:"20CA2", CreateAccount:true, XboxToken:"XBL3.0 x=..."}` → `data.SessionTicket`, `data.PlayFabId` (24 h).
9. **session/start**: `POST {serviceUri}/api/v1.0/session/start`, `User-Agent: libhttpclient/1.0.0.0`, body
   `{device:{applicationType:"MinecraftPE", capabilities:null, gameVersion, id:<per-account uuid>, memory:"17179869184",
   platform:"Windows10", playFabTitleId:"20CA2", storePlatform:"uwp.store", type:"Windows10"},
   user:{language:"en", languageCode:"en-US", regionCode:"US", token:<ticket>, tokenType:"PlayFab"}}` → `result.authorizationHeader` (`MCToken ...`), `validUntil`.
10. **Multiplayer token**: `POST {serviceUri}/api/v1.0/multiplayer/session/start`, `Authorization: MCToken ...`,
    body `{"publicKey":<client SPKI b64>}` → `result.signedToken` (claim `cpk` = client key).

11. **Friends' worlds (MPSD)**: `http://xboxlive.com` XSTS, signed, XSAPI headers (`xsapi.rs`). `POST
    sessiondirectory.xboxlive.com/handles/query`, `PUT handles/<id>/session`, `POST handles` (activity), `PUT`/`GET`
    the session; RTA `wss://rta.xboxlive.com/connect`. Spec docs/research/friends-join.md. Not live-verified.

**Signature header** (differentially tested vs go-xsapi): base64(`u32be 1 ‖ i64be filetime ‖ r‖s`), P-256/SHA-256 over
`u32be 1,0, i64be filetime,0, method,0, path?query,0, Authorization,0, body,0`; filetime = unix_ns/100 + 116444736000000000,
corrected by the last server `Date` header.

Not live-verified (needs a real account): steps 3–6, 8–10 and NSAL `current`. Run `examples/device_login.rs` once.

## Login connection request (bytes after the protocol i32)
```
i32le L1 ‖ envelope JSON (L1) ‖ i32le L2 ‖ client-data JWT (L2)
envelope = {"Certificate":"{\"chain\":[...]}","AuthenticationType":N,"Token":"<multiplayer token>"}
```
- Online: N=0, chain = [self-signed head {exp:+6h, nbf:-6h, identityPublicKey:<Mojang chain[0] x5u>, certificateAuthority:true}] + Mojang chain.
- Offline: N=2, chain = `[""]`, Token = self-signed `{aud:"api://auth-minecraft-services/multiplayer", exp, nbf, ipt:"", mid:"", tid:"",
  cpk, xid:"", xname, leguuid}`; leguuid = v3-style MD5 of `OfflinePlayer:<name>`. Matches `login.EncodeOffline(..., legacy=false)`
  (checked against gophertunnel until 2026-10-03; client-data keys are now pinned to vanilla in `login/tests.rs`).
- JWTs: header `{"alg":"ES384","x5u":<SPKI DER b64>}`, raw 96-byte r‖s. Client data = gophertunnel `ClientData` keys,
  Android defaults, 64×64 skin with gophertunnel's `geometry.humanoid.custom` (assets/, MIT).
- Server handshake: `parse_server_handshake(jwt)` verifies against its x5u and returns `(PublicKey, salt)`.

## Verifying logins (servers)
`login::verify` (network-free, no `online` needed), after gophertunnel `login.Parse` / `service/jwks.go` (master, 2026-10-03):
`Verifier::new(SigningKeys::from_jwks(json)).verify(request, now_unix)` → `VerifiedLogin { identity, authenticated, client_key,
client_data, client_claims }` or a `VerifyError` naming the failed check and JWT (`Part`).
- **Keys**: `GET https://authorization.franchise.minecraft-services.net/.well-known/openid-configuration` → `issuer` (with trailing
  slash), `jwks_uri` = `…/.well-known/keys` (live-verified; RSA keys, `x5t` in hex, ignored). `AuthClient::fetch_signing_keys`
  (`online`) does both; the verifier never fetches. Re-fetch on `UnknownSigningKey`, rate-limited (`SigningKeys::contains(kid)`).
- **Token** (takes precedence over a chain): RS256 by the key its `kid` names, `iss` = issuer exactly, `aud` contains the multiplayer
  audience, `exp` required, `exp`/`nbf` with 60 s leeway; the client key is `cpk`. `authenticated` = issued and `xid` non-empty;
  UUID = v3-style MD5 of `pocket-auth-1-xuid:<xid>` (`xuid_identity`). ES384 tokens are self-signed (verified against their own
  `cpk`): never authenticated. Any other `alg` is refused.
- **Legacy chain**: 1 JWT = self-signed, offline. 3 JWTs = head signed by its `x5u`, each next one by the previous
  `identityPublicKey`, links 1–2 `iss` = `Mojang`; authenticated only if the head names `MOJANG_ROOT_KEY`. A 3-chain with an XUID
  but another root is refused (`UntrustedChain`). Identity = last `extraData`; the client key is the last `identityPublicKey`.
- **Client data**: ES384 by that client key. This is what binds a replayed token or chain to its owner, so use
  `VerifiedLogin::client_key` for the encryption handshake. Parsed leniently (missing claims default, unknown ones only in
  `client_claims`); field values are not validated.
- Unauthenticated logins drop any claimed XUID; the UUID is the client's claim (`leguuid` / `extraData.identity`) or derived
  from the name. Guest logins (`AuthenticationType` 1) are refused. Policy (online-mode, duplicate XUIDs) is the server's.
- Not live-verified: a real vanilla token against the live JWKS; tests use a stand-in RSA issuer.

## Cache (`CachedTokens`, one JSON per account in `FileTokenCache`)
`msa`, `xbox_device_key` (P-256 hex, reused so device tokens stay valid), `device_id`, `device_token`, `sisu`,
`xsts{rp→token}`, `playfab`, `service_token`, `credentials` (+ client pubkey; reused while unexpired for the same key).
Tokens are refreshed 60 s before expiry. Files contain refresh tokens: keep the directory private.
`TokenCache` is async and versioned: `store(account, tokens, expected)` writes only if the stored version is still
`expected` (`None` = absent), else `Stored::Conflict`. `Account` writes back with the version it read and, on conflict,
keeps the stored copy (another writer rotated the refresh token first); `sign_in` retries on top of the newer copy.
A shared implementation (database) must make that check atomic. `FileTokenCache` checks within one process only and
stores `version` in the JSON (files without it read as 0).

## Errors
`Error::Xbox(XboxError)` from `X-Err` header or `XErr` body: `NoXboxProfile` (2148916233), `ChildAccount` (2148916238),
`Banned`, `ParentallyRestricted`, `TermsNotAccepted`, `CountryNotAuthorized`, `AgeVerificationRequired`, `ScreenTimeExceeded`,
`GamertagChangeRequired`, `DeviceSignInLimit`, `SignedInElsewhere`, `Other(code)`. `Error::requires_user_action()` is true for all
named variants (report "xbox_required"). Also `DeviceCodeExpired`, `DeviceCodeDeclined`, `OAuth`, `Status`, `Http`, `Protocol`, `Jwt`, `Key`, `Cache`.
