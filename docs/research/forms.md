# Bedrock server forms: wire formats (target 1.26.x, protocol 2193)

Researched 2026-10-02. **[real]** = confirmed by code/dumps handling real client data; **[impl]** = what
server implementations emit/accept (agreement across several, not a client capture); **[unverified]** = inferred.

## Packets

| ID | Name | Dir | Layout |
|---|---|---|---|
| 0x64 (100) | ModalFormRequest | S→C | `varuint32 form_id, string form_json` |
| 0x65 (101) | ModalFormResponse | C→S | `varuint32 form_id, bool has_data, [string json], bool has_reason, [u8 reason]` |
| 0x66 (102) | ServerSettingsRequest | C→S | empty |
| 0x67 (103) | ServerSettingsResponse | S→C | same as ModalFormRequest |
| 0x136 (310) | ClientboundCloseForm | S→C | empty |

Cancel reason enum `ModalFormCancelReason`: `UserClosed=0`, `UserBusy=1` (uint8). The optional data/reason pair
dates from **1.19.20 (protocol 544)** (BedrockProtocol commit "Protocol changes for 1.19.20"). ClientboundCloseForm
dates from **1.21.20 (712)** (originally `CloseFormPacket`, renamed for parity with Mojang's name).
Sources: Mojang `bedrock-protocol-docs` json/ModalFormResponsePacketPayload.json, ModalFormCancelReason.json,
ClientboundCloseFormPacketPayload.json (x-protocol-version 2223, 1.26.60-beta.29);
gophertunnel `minecraft/protocol/packet/{modal_form_response,client_bound_close_form,id}.go` (CurrentProtocol 2193 / 1.26.50);
pmmp/BedrockProtocol `src/ModalFormResponsePacket.php`, `src/ProtocolInfo.php`; PrismarineJS minecraft-data `bedrock/latest/proto.yml`.

## Request schemas (`form_json`)

Any `text`/`title`/`content`/button text can be a plain string **or** a rawtext object
`{"rawtext":[{"text":"..."}]}` / `{"rawtext":[{"translate":"key","with":[...]}]}`. EndstoneMC (wraps BDS) always
sends rawtext (`src/endstone/core/form/form_codec.cpp`). Parse both.

**Modal** (`ModalFormData`/MessageForm) [impl: dragonfly `server/player/form/modal.go`, Endstone, Cumulus]
```json
{"type":"modal","title":"T","content":"Body","button1":"gui.yes","button2":"gui.no"}
```

**Action/simple** (`ActionFormData`). Two encodings are accepted by current clients:
```json
{"type":"form","title":"T","content":"Body","buttons":[{"text":"A","image":{"type":"path","data":"textures/items/apple"}}]}
{"type":"form","title":"T","content":"Body","elements":[
  {"type":"header","text":"H"},{"type":"label","text":"L"},{"type":"divider","text":""},
  {"type":"button","text":"A","image":{"type":"url","data":"https://..."}},{"type":"button","text":"B"}]}
```
- Legacy `buttons` (buttons only, no `type` key) is still what GeyserMC/Cumulus sends (`SimpleFormCodec.java`).
- `elements` (headers/labels/dividers interleaved with `"type":"button"`) is what dragonfly (PR #1154, 2025-12),
  PowerNukkitX (`form/window/SimpleForm.java`) and Endstone (`ActionForm`) send since ActionFormData got
  `header/label/divider` in @minecraft/server-ui 2.0.0 (game 1.21.80-era). Bot should read either key.
- Image: `{"type":"path"|"url","data":"..."}`; `url` when data starts with `http(s):` (all impls agree).

**Custom** (`ModalFormData`) [impl: dragonfly `element.go`/`form.go`, PNX `form/element/**`, Endstone]
```json
{"type":"custom_form","title":"T","submit":"Go","icon":{"type":"path","data":"..."},"content":[
  {"type":"header","text":"H"},
  {"type":"label","text":"L"},
  {"type":"divider","text":""},
  {"type":"input","text":"Name","placeholder":"hint","default":"","tooltip":"opt"},
  {"type":"toggle","text":"On?","default":false,"tooltip":"opt"},
  {"type":"slider","text":"N","min":0,"max":10,"step":1,"default":5,"tooltip":"opt"},
  {"type":"step_slider","text":"S","steps":["a","b","c"],"default":0,"tooltip":"opt"},
  {"type":"dropdown","text":"D","options":["x","y"],"default":0,"tooltip":"opt"}]}
```
- Slider uses `step` (number); step slider uses `steps` (string array); dropdown uses `options`. `default` of
  dropdown/step_slider is an index. `tooltip` is optional on input/toggle/slider/dropdown/step_slider (dragonfly, PNX).
- `submit` (custom submit-button text) and `icon` (settings-tab icon) are optional top-level keys (Endstone).
- Any of `default`/`placeholder` may be absent; treat absent as `""`/`false`/`min`/`0`.

## ModalFormResponse: what the client sends

| Form | Action | has_data / json | has_reason / reason | Confidence |
|---|---|---|---|---|
| action | click Nth **button** | `"N\n"` e.g. `"0\n"` | none | **[real]** (1.21.90 PM dump; 2026-10-02 capture) |
| modal | button1 / button2 | `"true\n"` / `"false\n"` | none | **[real]** `"true\n"` (2026-10-02 capture) |
| custom | submit | `"[...]\n"` JSON array, one slot per `content` element | none | **[real]** (2026-10-02 capture) |
| any | X / back | none | `0` (UserClosed) | [impl] (see below) |
| any | can't show (busy) | none | `1` (UserBusy) | [impl] |

- **Trailing newline is real**: a PMMP plugin dump of a vanilla client clicking a button showed
  `"formData": "0\n"` (PrismarineJS/bedrock-protocol#626, client 1.21.90). Consistent with Mojang's jsoncpp
  FastWriter (compact, appends `\n`). Every server parser trims or uses a whitespace-tolerant JSON decoder
  (Cumulus `data.trim()`; PNX `jsonResponse.trim()`; PM `json_decode`; Go `json.Unmarshal`), so `\n` never
  breaks a server, but send it to match vanilla byte-for-byte.
- **Action-form index counts buttons only** (headers/labels/dividers in `elements` are skipped): dragonfly
  `Menu.Buttons()`, PNX filters `ElementButton`, Endstone counts `holds_alternative<Button>`.
- **Custom-form slots**: label/header/divider each occupy a slot with JSON `null`. Cumulus rejects anything but
  `null` for a label ("Return value of label should be null", `CustomFormCodec.validateComponent`); dragonfly
  consumes one slot per read-only element; @minecraft/server-ui 2.0.0 `formValues` became `(…|undefined)[]` with
  entries for labels/headers/dividers. **Gotcha**: client 1.21.70 (786) briefly *omitted* read-only slots;
  dragonfly adapted (commit 5033755, 2025-03-26) then reverted when the client went back to null slots
  (PR #1068, 2025-05-19, 1.21.80). Current = null slots.
- Value types per slot: input → JSON string; toggle → `true`/`false`; dropdown & step_slider → integer index;
  slider → JSON number (Cumulus `getAsFloat`, dragonfly `Float64`, PNX `Float.parseFloat`, all accept `5` or `5.0`).
  **[real]** A slider at 5 (min 1.0, max 100.0, step 1.0) is written as the integer `5` (2026-10-02 capture:
  label, empty input, toggle, slider, dropdown → `[null,"",true,5,1]\n`). Spelling of a fractional value is
  **[unverified]**; the bot writes it as serde_json prints the f64 (`2.5`).
- Example vanilla-style custom answer for the schema above: `[null,null,null,"",false,5,0,0]\n`.
- **Closed**: modern servers treat `reason=Some(0)` as closed (PM `InGamePacketHandler::handleModalFormResponse`,
  PNX `ModalFormResponseHandler`) and also legacy `json="null"`/empty data as closed (dragonfly, Cumulus).
  Mojang's schema makes both fields optional. Exact vanilla bytes for a close (`has_data=false, has_reason=true, 0`
  vs `"null\n"`) **[unverified]**. Recommend `has_data=false, reason=0`: PM rejects a packet with *neither* field set
  ("expected to have formData or cancelReason set", bedrock-protocol#626).
- Historical: pre-2020 clients emitted malformed JSON (PM3 `stupid_json_decode`, pmmp#3113); modern PM uses
  plain `json_decode`, so no longer relevant.

## Timing

2026-10-02 capture (one tester, n=4; `human::FORM_READ_*`): shown → reply 2944 ms (action form, 26 chars),
1712 (modal, 16 chars), 4557 (custom form, 42 chars, 4 inputs left at their defaults), 4073 (action form
"Close me", 9 chars; meant to be closed, then submitted). The bot waits 1.3–3.0 s + 15 ms per character
+ 600 ms per custom-form input, capped at 8 s. No PlayerAuthInput flag changed while a form was up. A
close (X) was never captured, so its bytes and delay stay **[unverified]**.

## Busy / stacking

- `UserBusy` = "the player was busy with another UI interaction" (MS Learn `FormCancelationReason`); PM comment:
  "Sent if a form is sent when the player is on a loading screen" (`ModalFormResponsePacket.php`). bedrock.dev:
  forms "only open when no other UI is open … you cannot [open from chat] because the chat UI is open".
  So vanilla answers Busy immediately when chat/inventory/pause/loading screen is up. Scripts commonly re-show
  on `UserBusy`. A headless bot has no other UI, so it should never send Busy unless deliberately emulating it.
- A ModalFormRequest arriving while another form is open: the client keeps a **form stack** (gophertunnel:
  ClientboundCloseForm "clear[s] the entire form stack … all forms that are currently open"; Geyser `FormCache`
  tracks several outstanding IDs). Whether the new one shows on top or is answered Busy **[unverified]**;
  safest bot model: track every outstanding form_id and answer each exactly once.
- dragonfly ignores a second, data-less response for an already-answered ID ("Sometimes the client seems to send a
  second response with no data", `server/session/handler_modal_form_response.go`) — vanilla quirk, do not emulate.

## ServerSettings

Client sends ServerSettingsRequest (empty) when the player opens the Settings screen (vanilla sent it in 1 of 4
captured sessions; idle bots send none). Server may answer ServerSettingsResponse
`{form_id, custom_form JSON}` (usually with `icon`), shown as a server tab. The client answers with a normal
ModalFormResponse (same form_id) when the settings screen is closed (gophertunnel/PrismarineJS comments; PNX
handles it via `getServerSettings()`).

## ClientboundCloseForm (0x136)

Empty payload. Closes all open server forms (the whole stack); not inventories/containers (gophertunnel doc).
Server-side libs (Geyser `FormCache.closeForms`, Endstone `closeForm`) treat their pending forms as closed
locally and expect **no** response. Whether vanilla sends a ModalFormResponse(reason=0) per closed form
**[unverified]**; safest: drop pending IDs silently.

## Related gotchas

- Geyser servers: after sending a simple form, they send NetworkStackLatency with timestamp -1234567890 (url-image
  hack, `FormCache.sendForm`) and expect the normal reply; then an UpdateAttributes packet.
- @minecraft/server-ui 2.1.0+ `CustomForm`/`MessageBox` are **DDUI** (ClientboundDataDrivenUIShowScreen 0x14d,
  ServerboundDataDrivenScreenClosed 0x157), not ModalForm. Out of scope here; separate research if a target uses them.
