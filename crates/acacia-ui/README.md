# acacia-ui

HUD, chat and (later) menus for an Acacia client. It lays widgets out into a `DrawList` of textured
quads in window pixels and never touches a GPU or a window: `acacia-render` draws the list
(`Renderer::render(camera, Some((atlas, quads)))`, a pass after the world), and the viewer feeds it
player state. Layout is tested headless.

## Pieces

- `atlas.rs`: every UI image in one RGBA texture, packed onto shelves by name. `version` bumps on
  each change so the renderer re-uploads. `white` is one opaque texel for flat fills.
- `draw.rs`: `DrawList` takes GUI pixels and scales to window pixels as quads are added; UVs are in
  atlas texels so sprites stay pixel-exact. The renderer samples with nearest filtering.
- `scale.rs`: GUI scale, the largest whole scale leaving 320×240 GUI pixels (Java's rule), or the
  user's choice when smaller.
- `font.rs`: bitmap fonts of 16×16 cells (Java's `ascii.png`). A glyph is as wide as its rightmost
  opaque column, advance width + 1, space 4. `§0`–`§f`, `§l`, `§r`; the shadow is each channel at a
  quarter, one pixel down and right.
- `hud.rs`: Java's `Gui` layout at scale 1: hotbar (182×22, bottom centre), selection 24×23 at
  `cx − 92 + 20·slot`, items at `+3, +3`, hearts from `cx − 91` at `h − 39` (rows of ten), food from
  `cx + 91` leftwards, armour above the hearts, air above the food, experience bar at `h − 29` with
  the level at `h − 35`. Creative and spectator draw only the hotbar and crosshair.
- `chat.rs`: the log (100 lines; 10 shown while closed, for 10 s with a 1 s fade, Java's
  `ChatComponent`), the input line and sent-line recall. Drawn bottom-left, 320 wide, 40 px up.

- `inventory.rs`: the player's screen (Java's `InventoryScreen`, 176×166) and rows of nine above
  the player's slots (`ChestMenu`), with hit-testing. `inventory/art.rs` draws the panel: Java's
  `container/inventory.png` and `generic_54.png`, Bedrock's `dialog_background_opaque` with a
  `cell_image` per slot (its classic screen has the same 176×166 root), flat greys otherwise.
- `recipes.rs`: the recipe book left of the player's screen, in Java's `recipe_book.png` and slot
  buttons or Bedrock's panel and cells.
- `menu.rs`: button menus (pause, options, death) on the widget skins: Java's 200×20 column a
  quarter down; Bedrock's pause column under the logo over a dimmed left band
  (`research/pause-bedrock-layout.md`).
- `sidebar.rs`: the scoreboard sidebar in Java's `displayScoreboardSidebar` layout, for both looks.
- `overlay.rs`: title (scale 4) and subtitle (scale 2) with Java's 10/70/20-tick fade, the action
  bar 68 px up for 3 s, boss bars 19 px apart from the top.
- `nine.rs`: nine-slice sprites with their borders from the file beside them (`.mcmeta`, Bedrock's
  `.json`); Java tiles edges and centre, Bedrock stretches. `DrawList::clip` cuts quads for scrolling.
- `input.rs`: mouse, wheel, keys and typed text in GUI pixels; the viewer translates window events.
- `widget/`: `Widget` (text, header, divider, button, toggle, slider, step slider, dropdown, text
  input) and `Panel`, widgets at caller-given places with a scrolling viewport, focus (Tab, arrows),
  text editing, slider drags and the open dropdown list. The theme's `Skin` draws them:
  `widget/bedrock.rs` from `textures/ui`, `widget/java.rs` from the jar's `sprites/widget`, flat
  fills where a sprite is missing.
- `form/`: server forms (action, modal, custom) as a `FormView` that answers with an `Outcome`.
  Bedrock lays them out as `server_form.json` (a 225×200 dialog, 32 px buttons, a close X); Java as
  its own server dialogs (33 px header and footer, a 200 px column 10 apart, Esc to close; modal and
  submit buttons in the footer). Measurements: the workspace's `research/forms-bedrock-layout.md`
  and `research/forms-java-widgets.md`. Unverified there: Bedrock's line height (10), the open
  dropdown's place, the slider handle's travel, its `large` header font (drawn bold).

## Themes (`theme/`)

A `Theme` is an atlas holding the HUD sprites under the names in `hud::sprite`, a font and a
`Style` (what the games lay out differently). Sources and measurements: the workspace's
`research/ui-assets.md`.

| | Java (`theme::java`) | Bedrock (`theme::bedrock`) |
|---|---|---|
| Sprites | The jar's `textures/gui/sprites/hud/*.png`, one per name, copied into the Java look by `lookbake java` | The pack's `textures/ui`: the hotbar assembled from `hotbar_0..8` and the end caps at 65% opacity; hearts, food, armour, air by file; crosshair and experience bar from `textures/gui/icons.png` |
| Font | `textures/font/ascii.png`, characters per `font/include/default.json` | None in the pack: borrows the Java look's `ascii.png` (the same Mojangles glyphs) when that look was baked |
| Level number | `0x80FF20`, black outline | `0x80FF00`, drop shadow |

Without a font, text is not drawn; the sprites still are. A Java look that was not baked draws the
Bedrock sprites (the same pixels) with the Java style.

## Not yet

Workstation and other non-chest containers, the crafting grid and portrait, Unicode glyph pages,
offhand and attack indicator, Java's background blur, Bedrock's own settings screen and sidebar
placement, multiselect form elements, JPEG button images.
