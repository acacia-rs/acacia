package main

import (
	"math"
	"strconv"
	"strings"

	"github.com/chewxy/math32"
	"github.com/df-mc/dragonfly/server/block"
	"github.com/df-mc/dragonfly/server/block/cube"
	"github.com/df-mc/dragonfly/server/world"
)

// Must stay in sync with TestBlock in src/test_world.rs.
type kindDef struct {
	name  string
	boxes []cube.BBox32
}

var full = []cube.BBox32{cube.Box32(0, 0, 0, 1, 1, 1)}

var kinds = map[string]kindDef{
	"stone":       {"minecraft:stone", full},
	"slab":        {"minecraft:stone_slab", []cube.BBox32{cube.Box32(0, 0, 0, 1, 0.5, 1)}},
	"stair":       {"minecraft:stone_stairs", []cube.BBox32{cube.Box32(0, 0, 0, 1, 0.5, 1), cube.Box32(0, 0.5, 0.5, 1, 1, 1)}},
	"ladder":      {"minecraft:ladder", []cube.BBox32{cube.Box32(0, 0, 0.8125, 1, 1, 1)}},
	"ice":         {"minecraft:ice", full},
	"packed_ice":  {"minecraft:packed_ice", full},
	"blue_ice":    {"minecraft:blue_ice", full},
	"soul_sand":   {"minecraft:soul_sand", []cube.BBox32{cube.Box32(0, 0, 0, 1, 0.875, 1)}},
	"slime":       {"minecraft:slime", full},
	"honey":       {"minecraft:honey_block", []cube.BBox32{cube.Box32(0.0625, 0, 0.0625, 0.9375, 0.9375, 0.9375)}},
	"web":         {"minecraft:web", nil},
	"powder_snow": {"minecraft:powder_snow", nil},
	"berry_bush":  {"minecraft:sweet_berry_bush", nil},
	"vine":        {"minecraft:vine", nil},
	"scaffolding": {"minecraft:scaffolding", nil},
	"bed":         {"minecraft:bed", []cube.BBox32{cube.Box32(0, 0, 0, 1, 0.5625, 1)}},
}

type namedBlock struct{ name string }

func (b namedBlock) Hash() (uint64, uint64)                { return 0, math.MaxUint64 }
func (b namedBlock) EncodeBlock() (string, map[string]any) { return b.name, nil }
func (b namedBlock) Model() world.BlockModel               { return block.Air{}.Model() }

type Fill struct {
	Kind string `json:"kind"`
	Min  [3]int `json:"min"`
	Max  [3]int `json:"max"`
}

type testWorld struct{ cells map[cube.Pos]string }

func newWorld(fills []Fill) *testWorld {
	w := &testWorld{cells: map[cube.Pos]string{}}
	for _, f := range fills {
		for x := f.Min[0]; x <= f.Max[0]; x++ {
			for y := f.Min[1]; y <= f.Max[1]; y++ {
				for z := f.Min[2]; z <= f.Max[2]; z++ {
					if f.Kind == "air" {
						delete(w.cells, cube.Pos{x, y, z})
					} else {
						w.cells[cube.Pos{x, y, z}] = f.Kind
					}
				}
			}
		}
	}
	return w
}

// liquidKind parses "water", "water@N" (depth N, 8 = source), "water_falling" and the lava equivalents.
func liquidKind(kind string) (world.Liquid, bool) {
	base, depth, falling := kind, 8, false
	if i := strings.IndexByte(kind, '@'); i >= 0 {
		base = kind[:i]
		depth, _ = strconv.Atoi(kind[i+1:])
	}
	if strings.HasSuffix(base, "_falling") {
		base, falling = strings.TrimSuffix(base, "_falling"), true
	}
	switch base {
	case "water":
		return block.Water{Still: true, Depth: depth, Falling: falling}, true
	case "lava":
		return block.Lava{Still: true, Depth: depth, Falling: falling}, true
	}
	return nil, false
}

func (w *testWorld) Block(pos cube.Pos) world.Block {
	kind, ok := w.cells[pos]
	if !ok {
		return block.Air{}
	}
	if liquid, ok := liquidKind(kind); ok {
		return liquid
	}
	return namedBlock{kinds[kind].name}
}

func (w *testWorld) BlockCollisions(pos cube.Pos) []cube.BBox32 {
	kind, ok := w.cells[pos]
	if !ok {
		return nil
	}
	return kinds[kind].boxes
}

// GetNearbyBBoxes mirrors the default WorldView::collisions in src/world.rs.
func (w *testWorld) GetNearbyBBoxes(aabb cube.BBox32) []cube.BBox32 {
	min, max := aabb.Min(), aabb.Max()
	var out []cube.BBox32
	for x := int(math32.Floor(min.X())); x <= int(math32.Floor(max.X())); x++ {
		for y := int(math32.Floor(min.Y())) - 1; y <= int(math32.Floor(max.Y())); y++ {
			for z := int(math32.Floor(min.Z())); z <= int(math32.Floor(max.Z())); z++ {
				pos := cube.Pos{x, y, z}
				for _, bb := range w.BlockCollisions(pos) {
					bb = bb.Translate([3]float32{float32(x), float32(y), float32(z)})
					if bb.IntersectsWith(aabb) {
						out = append(out, bb)
					}
				}
			}
		}
	}
	return out
}

func (w *testWorld) IsChunkLoaded(int32, int32) bool { return true }

func (w *testWorld) LiquidFlowFaceClosed(pos cube.Pos, _ cube.Face) bool {
	boxes := w.BlockCollisions(pos)
	return len(boxes) == 1 && boxes[0] == full[0]
}

func (w *testWorld) LiquidFlowBarrier(pos cube.Pos) bool {
	return len(w.BlockCollisions(pos)) != 0
}
