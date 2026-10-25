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
  `cx − 92 + 20·slot`, items at `+3, +3` (`hud::Item`: icon, count, and `glint` for an enchanted
  one, which `DrawList::icon` passes on as `Quad::glint` for the renderer to shimmer), hearts from `cx − 91` at `h − 39` (rows of ten), food from
  `cx + 91` leftwards, armour above the hearts, air above the food, experience bar at `h − 29` with
  the level at `h − 35`. Creative and spectator draw only the hotbar and crosshair.
- `chat.rs`: the log (100 lines; 10 shown while closed, for 10 s with a 1 s fade, Java's
  `ChatComponent`), the input line and sent-line recall. Drawn bottom-left, 320 wide, 40 px up.

- `inventory.rs`: the player's screen (Java's `InventoryScreen`, 176×166) and rows of nine above
  the player's slots (`ChestMenu`), with hit-testing. `inventory/art.rs` draws the panel: Java's
  `container/inventory.png` and `generic_54.png`, Bedrock's `dialog_background_opaque` with a
  `cell_image` per slot (its classic screen has the same 176×166 root), flat greys otherwise.
  `inventory/station.rs`: furnaces, hopper, dispenser, brewing stand (a container's own slots, with
  progress). `inventory/bench.rs`: crafting table, anvil, smithing table, grindstone, stonecutter, loom,
  cartography and enchanting table: the player's UI slots by Bedrock's offset, a `Slot::Result`
  the caller works out, and a pick list (`Pick`, `hit_pick`) for the stonecutter's cuts and the
  enchanting options. `Layout::Trade` is Java's 276-wide `MerchantScreen`: the offers as a pick
  list left of the payment slots. `Layout::Creative` is Java's 195×136 creative screen: a tab's
  items as a nine-column pick list over the hotbar, the tabs as buttons above (`hit_tab`).
  `inventory/paint.rs` draws them all.
- `recipes.rs`: the recipe book left of the player's screen, in Java's `recipe_book.png` and slot
  buttons or Bedrock's panel and cells.
- `menu.rs`: button menus (pause, options, death) on the widget skins: Java's 200×20 column a
  quarter down; Bedrock's pause column under the logo over a dimmed left band
  (`research/pause-bedrock-layout.md`).
- `sidebar.rs`: the scoreboard sidebar in Java's `displayScoreboardSidebar` layout, for both looks.
- `overlay.rs`: title (scale 4) and subtitle (scale 2) with Java's 10/70/20-tick fade, the action
  bar 68 px up for 3 s, boss bars 19 px apart from the top.
- `book.rs`: a book's page in Java's `BookViewScreen` layout (14 lines of 114 px, the page number,
  turn buttons, Done; Sign and the title prompt for a book and quill) on flat parchment, for both looks; `fits` is the limit on what a page takes.
- `signs.rs`: a sign side's text as quads in font pixels around its centre, for the renderer to put
  on the board (see "Signs").
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
| Sprites | None for the HUD: the Java look's HUD is `java::hud::frame`, painted from the jar's files; effect icons for screens | The pack's `textures/ui`: the hotbar assembled from `hotbar_0..8` and the end caps at 65% opacity; hearts, food, armour, air by file; crosshair and experience bar from `textures/gui/icons.png` |
| Font | `textures/font/ascii.png`, characters per `font/include/default.json` | None in the pack: borrows the Java look's `ascii.png` (the same Mojangles glyphs) when that look was baked |

Without a font, text is not drawn; the sprites still are. A Java look that was not baked draws the
Bedrock HUD (the same pixels) with the Java style.

## Signs (`signs.rs`)

Java's `AbstractSignRenderer` (26.3 client): four lines, each centred (the negated width halved
towards zero), line i at `(i − 2) ×` the line height: 10 px and at most 90 px wide on signs, 9 and
60 on hanging signs. Bedrock keeps a side's text as one string: it is split at `\n`, each part
wrapped at spaces to the width, and the first four lines drawn.

| | Java theme (`Style::java_signs`) | Bedrock theme |
|---|---|---|
| Colour | Bedrock's `SignTextColor` (the dye's colour; blue, red and green seen on BDS 1.26.52, the rest from the dyes' Java values) mapped to Java's `DyeColor.getTextColor`, another colour kept | `SignTextColor` |
| Unlit text | 40% of it (`getDarkColor`) | As it is; unmeasured |
| Glowing | Full colour over an outline (the glyphs at the 8 neighbouring pixels) of the 40% colour, `0xF0EBCC` around black | The same; unmeasured. `HideGlowOutline` drops the outline |

`§` codes colour the text as the font does everywhere; the outline ignores them.

The sign editor (`sign_editor.rs`) follows Java's `AbstractSignEditScreen` in both themes: a flat
oak board with the four lines, the typed one between `>` and `<`, and Done. A hanging sign gets a smaller
board on two chains; lines are capped at the sign's own width (90 px, or 60 hanging).

## Java GUI (`java/`)

Java 26.3's GUI ported call for call. Layouts (`hud/`, `screen/`, `container/`, `chat/`,
`tooltip/`, `widget/`) take the state the game reads and emit `GuiGraphicsExtractor` calls
(`gui::GuiDraw`). The parity harnesses compare these lists with the game's own, recorded headless
by `GuiCapture` (`tools/parity/java/acacia/parity/gui`). See the `gui` rows of `docs/parity.md`.

- **Font (`font/`):** FontManager and FontSet with the bitmap, space, unihex and reference
  providers, filters, and Unifont from the asset index. Obfuscated glyphs are picked from same-width
  glyphs in Java's hash-set order. `text.rs` handles components and legacy codes; `split*.rs`
  ports StringSplitter.
- **Paint (`paint/`):** turns calls into quads the way GuiRenderState does.
  - Strata and nodes go through `findAppropriateNode`, with glyphs after the elements of each node.
  - Blits handle stretch, tile and nine-slice sprites from `.mcmeta`.
  - Scissors are intersected and poses applied.
  - Highlight blends are invert and additive.
  - Labels scroll by `defaultScrollingHelper`.
- **Pictures:** a picture-in-picture (entity, book, banner) is placed by its rect, without the
  pose. It becomes a premultiplied blit (`Quad::picture`) after its node's other blits, as
  `GuiRenderer.preparePictureInPicture` adds it.
  - The renderer draws picture entities into a window-sized texture with an orthographic
    projection and Java's `ENTITY_IN_UI` lights (acacia-render `gpu/over.rs`). The viewer poses
    them as `PictureInPictureRenderer` does (`app/portrait.rs`).
  - Java's model space is the mesh's flipped in y (`S(1,-1,1)·T(0,-1.501,0)`), so a living
    entity's place is the PiP pose times `rotY(180° − bodyRot)·S(-1,1,1)`.
  - Banners are composed flat: their face fills the picture unshaded under `ITEMS_FLAT`.
  - Atlas sprite blits are ordinary blits.
  - A map and other tooltip images come from the `Source` as calls. A bundle's image
    (`tooltip/bundle.rs`) draws itself.
  - Not drawn yet: the enchanting book (needs Java's `BookModel` posed by the renderer) and the
    end portal background.
- **Text:** `bidi/` ports ICU4J's Bidi and ArabicShaping for right-to-left text. `command/` is a
  minimal Brigadier (parse, suggestions, usage) for the chat screen's suggestions. Chat lines
  carry hover-event and tag tooltips.
- **Frame:** the `gui_frame` harness checks the painter's whole frame. It replays each source's
  calls through the real `GuiGraphicsExtractor` and compares the painted quads with
  `GuiRenderer`'s sorted elements.
- **Not ported:** creative and recipe-book search trees, BookSignScreen.

## Not yet

The loom's banner preview, the beacon's own art, a cartography table's name box, Unicode glyph pages,
offhand and attack indicator, Java's background blur, Bedrock's own settings screen and sidebar
placement, multiselect form elements, JPEG button images.
