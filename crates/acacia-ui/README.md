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
  the player's slots (`ChestMenu`), with hit-testing; panels and slots drawn flat in vanilla's greys.
- `menu.rs`: a title over a column of 200×20 buttons (pause, options); hit-testing.
- `overlay.rs`: title (scale 4) and subtitle (scale 2) with Java's 10/70/20-tick fade, the action
  bar 68 px up for 3 s, boss bars 19 px apart from the top.

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

Workstation and other non-chest containers, the crafting grid and portrait, server forms (in
progress on `ui/forms`), scoreboard, Unicode glyph pages, offhand and attack indicator, Java's
widget sprites (screens are drawn flat).
