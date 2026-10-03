# Vanilla QoS + LAN discovery capture (2026-10-03)

Windows game 1.26.52 (protocol 2193), pktmon UDP filtered to ports 3075 and 7551, ~165 s: launch, Realms
screen + realm join, Friends tab, hosting a LAN world, then our `lan_join` example joining it. pktmon logs each
packet once per network component; dedupe by (src, dst, payload) within 50 ms. Analysis script was ad hoc
(tshark fields → Python; LAN decrypt per nethernet-signaling.md §1).

## QoS beacons (UDP 3075) — implemented in `acacia-client/src/qos.rs`
- One burst during the realm join: **one ping per region**, all 28 `qos-beacons.prod` regions, from one socket
  (ephemeral port), sends **~15 ms apart** (13–33 ms), whole burst 0.43 s. No retries; every region answered.
- Payload **10 B**: `FF FF <id> 10 00 00 00 00 00 00`. Echo: same with `00 00` first. `id` is a per-region
  number 0x01–0x1C; `centralUs` and `centralUsEuap` share a host and got separate pings (0x0A/0x0B).
- Slowest echo 0.29 s (australiaSoutheast). Give-up time unknown.
- Unexplained (copy only after a second capture shows it is stable): the id → region map is not discovery's
  alphabetical order (canadaCentral 1, brazilSouth 2, australiaEast 3, australiaSoutheast 4, mexicoCentral 5,
  centralIndia 6, uaeNorth 7, eastAsia 8, koreaCentral 9, centralUs/centralUsEuap 10/11, eastUs 12, eastUs2 13,
  franceCentral 14, southCentralUs 15, japanEast 16, japanWest 17, northCentralUs 18, northEurope 19,
  southAfricaNorth 20, southeastAsia 21, swedenCentral 22, ukSouth 23, westUs 24, westCentralUs 25,
  westEurope 26, westUs2 27, westUs3 28), and the send order is neither (ids 1,3,7,15,18,28,27,…,5,2,4).
  Ours: id = discovery index + 1, sent in discovery order. Byte 3 (`0x10`) may be a counter; unknown.

## LAN discovery (UDP 7551)
- Request plaintext 20 B, **length field 20** (includes itself), packet id 0, 8 zero bytes, empty body;
  wire 64 B. Ours is byte-identical apart from the sender id.
- Cadence **2.00–2.02 s** while the Play screen is open; stops on leaving it.
- Destinations: the first request of a burst goes to `255.255.255.255`; every later one to the **subnet
  broadcast** (`192.168.1.255`) and **`ff02::1`** (IPv6 link-local all-nodes) at the same instant.
- Source port **7551** (the hosting socket is reused for discovery). Ours: ephemeral, `255.255.255.255` every
  time.
- Sender id: one per game session (same id across bursts 50 s apart). Ours now keeps the discovery id for
  the dial (`LanServer::local_id`).
- Not captured: the host's Response and Message packets (same-machine traffic stays on loopback). Needs
  a second device on the LAN.

## LAN behaviour of BDS (BDS 1.26.x; NetherNet's LAN code is shared with the client)
Quoted strings are BDS's `[LAN] …` log lines.
- Socket: IPv6 UDP first (dual-stack: an option set to 0 before binding, read as `IPV6_V6ONLY` off), broadcast
  enabled; IPv4 socket only if IPv6 creation fails. Bind the configured port (7551); on "address in use" log
  `port %hu already occupied, trying ephemeral port` and bind port 0.
- Destinations ("broadcast domain"), rebuilt by a network-interface watcher; until its first result the
  fallback `255.255.255.255:7551` is used (the capture's first request of a burst). Per interface address:
  loopback (127/8, ::1) skipped; IPv4 link-local 169.254/16 "not viable"; other IPv4 → address with all host
  bits set (prefix from the interface); IPv6 → `ff02::1` once, only if the socket is IPv6. Then
  `network discovery complete!`. A send error removes that address from the domain.
- Every request goes to every address in the domain (log `[%s] sending broadcast request to %s`).
- Receive: drops plaintext < 4 B or shorter than its length field (a smaller length field is accepted);
  minimum sizes Request 20, Response 24, Message 32; Response/Message payload = rest of the datagram (the
  inner u32 lengths are not trusted). Unknown ids log `Unknown DiscoveryPacketType`.
