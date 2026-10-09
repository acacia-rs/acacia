# acacia-auth

Login payloads and sign-in. Protocol details and endpoints: `docs/auth.md`.

## Persona piece types

BDS refuses a Login whose `PersonaPieces` or `PieceTintColors` holds a `PieceType` it has no name
for. It answers `PacketViolationWarning(packet 1, Malformed, "Connection Request invalid.")` and
closes the connection without a Disconnect. The names it reads are `skin::PERSONA_PIECE_TYPES`.

The wire's numeric piece type 9 is `hands` in the protocol data (`PersonaPieceType::Hands`), but
Login's name for it is `persona_hand`. `skin_harvest` wrote `persona_hands`, so the one pool skin
with a hands piece (`assets/skins/fd291d366cf763de.json`) could not log in. Skins are picked by
name, so the failure looked random across names but was fixed per name.

Measured 2026-10-09 against Linux BDS 1.26.52.3, offline, loopback
(`cargo run --release -p acacia-client --example login_loop`):

| | Attempts | Closed during login |
|---|---|---|
| before, 200 names | 200 | 27, all 27 with that skin; 0 of 171 with the other eight (2 more ended `RecentlyConnected`) |
| before, one name with that skin / one without, 10 joins each | 20 | 10 of 10 / 0 of 10 |
| after, 300 names | 300 | 0 |

One join each, changing only the piece list of a skin:

- That skin without its `persona_hands` piece, or with it renamed `persona_hand`: accepted. Without
  its face accessory, outerwear or high pants piece instead, or with the hands piece's id changed: refused.
- A skin that logs in, plus one piece of type `persona_hands`, `persona_unknown`,
  `persona_unsupported`, `persona_piece`, `persona_bogus` or `Persona_Top`: refused. Every name in
  the list was accepted, as an added piece or in a pool skin. The piece count does not matter
  (16 pieces accepted).
- The same for a tint's type: `persona_hand` accepted, `persona_hands` and `persona_bogus` refused.

The BDS binary holds `persona_hand` and no `persona_hands`, next to a cereal enum mapping for
`SharedTypes::persona::PieceType`. The check itself was not located in the decompiler.

`every_embedded_skin_names_piece_types_bds_reads` fails on a pool skin with any other name, and
`skin_harvest` no longer writes one.
