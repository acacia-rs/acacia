# Third-party material

Acacia is MIT-licensed ([LICENSE](LICENSE)). It includes or derives from these MIT-licensed works.

| Source | Copyright | Used in |
|---|---|---|
| [PrismarineJS/minecraft-data](https://github.com/PrismarineJS/minecraft-data) | PrismarineJS contributors | `tools/codegen/data/protocol.json` (generates `acacia-proto`); block, state and collision data in `acacia-world/data/blocks.bin` |
| [opencollab-incubator/Boar](https://github.com/opencollab-incubator/Boar) | (c) 2026 oryxel | Bedrock collision, friction and door rules in `acacia-world/tools/rules.mjs` → `blocks.bin` |
| [oomph-ac/bedsim](https://github.com/oomph-ac/bedsim) | (c) 2026 Oomph AC | `acacia-physics` is a port; full notice in [crates/acacia-physics/LICENSE-bedsim](crates/acacia-physics/LICENSE-bedsim) |
| [Sandertv/gophertunnel](https://github.com/Sandertv/gophertunnel) | (c) 2019 Sandertv | `acacia-auth/assets/skin_geometry.json` (default skin geometry) |
| [df-mc/go-xsapi](https://github.com/df-mc/go-xsapi), [lactyy/gophertunnel `feature/p2p`](https://github.com/lactyy/gophertunnel), [PrismarineJS/prismarine-xbox-services](https://github.com/PrismarineJS/prismarine-xbox-services), [microsoft/xbox-live-api](https://github.com/microsoft/xbox-live-api) | (c) df-mc, lactyy, PrismarineJS contributors, Microsoft Corporation | MPSD and RTA request shapes and the friend-world properties in `acacia-auth/src/online/{mpsd,friend_world,xsapi}.rs` and `acacia-client/src/friend/` (docs/research/friends-join.md) |

Each is used under the MIT License:

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
> associated documentation files (the "Software"), to deal in the Software without restriction, including
> without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the
> following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial
> portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT
> LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO
> EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
> IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
> USE OR OTHER DEALINGS IN THE SOFTWARE.

Other projects (Geyser, Dragonfly, PocketMine, PowerNukkitX, gophertunnel) are cited in comments for
how servers behave; no code is taken from them.
