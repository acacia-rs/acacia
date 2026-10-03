// Command diffharness drives bedsim through scripted scenarios and writes the
// per-tick results that acacia-physics' differential tests replay.
package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/go-gl/mathgl/mgl32"
	"github.com/oomph-ac/bedsim"
	"github.com/sandertv/gophertunnel/minecraft/protocol/packet"
)

// Input is the high-level client intent; toInputState mirrors Input::frame in src/input.rs.
type Input struct {
	Move      [2]float32 `json:"move"`
	Yaw       float32    `json:"yaw"`
	Pitch     float32    `json:"pitch"`
	Jump      bool       `json:"jump,omitempty"`
	Sneak     bool       `json:"sneak,omitempty"`
	Sprint    bool       `json:"sprint,omitempty"`
	Swim      bool       `json:"swim,omitempty"`
	Glide     bool       `json:"glide,omitempty"`
	UsingItem bool       `json:"using_item,omitempty"`
}

type Step struct {
	Input     Input       `json:"input"`
	Repeat    int         `json:"repeat"`
	Teleport  *[3]float32 `json:"teleport,omitempty"`
	Knockback *[3]float32 `json:"knockback,omitempty"`
}

type Effects struct {
	JumpBoost   *int32 `json:"jump_boost,omitempty"`
	Levitation  *int32 `json:"levitation,omitempty"`
	SlowFalling bool   `json:"slow_falling,omitempty"`
	Weaving     bool   `json:"weaving,omitempty"`
}

func (e Effects) GetEffect(id int32) (int32, bool) {
	switch {
	case id == packet.EffectJumpBoost && e.JumpBoost != nil:
		return *e.JumpBoost, true
	case id == packet.EffectLevitation && e.Levitation != nil:
		return *e.Levitation, true
	case id == packet.EffectSlowFalling && e.SlowFalling, id == bedsim.EffectWeaving && e.Weaving:
		return 0, true
	}
	return 0, false
}

type inventory struct{ elytra bool }

func (i inventory) HasElytra() bool { return i.elytra }

type Scenario struct {
	Name     string     `json:"name"`
	World    []Fill     `json:"world"`
	Start    [3]float32 `json:"start"`
	OnGround bool       `json:"on_ground"`
	Effects  Effects    `json:"effects"`
	Elytra   bool       `json:"elytra,omitempty"`
	Steps    []Step     `json:"steps"`
	Ticks    []Tick     `json:"ticks"`
}

type Tick struct {
	Pos          [3]float32 `json:"pos"`
	Vel          [3]float32 `json:"vel"`
	OnGround     bool       `json:"on_ground"`
	Collide      [3]bool    `json:"collide"`
	Sneaking     bool       `json:"sneaking"`
	Sprinting    bool       `json:"sprinting"`
	Swimming     bool       `json:"swimming"`
	Gliding      bool       `json:"gliding"`
	FallDistance float32    `json:"fall_distance"`
	Outcome      uint8      `json:"outcome"`
}

func toInputState(state *bedsim.MovementState, in Input, blocked bool) bedsim.InputState {
	canSprint := in.Sprint && in.Move[1] > 0 && !in.Sneak && !blocked
	return bedsim.InputState{
		MoveVector:      mgl32.Vec2{in.Move[0], in.Move[1]},
		MoveVectorIsRaw: true,
		Pitch:           in.Pitch,
		Yaw:             in.Yaw,
		HeadYaw:         in.Yaw,
		StartSprinting:  canSprint && !state.Sprinting,
		StopSprinting:   !canSprint && state.Sprinting,
		SprintDown:      in.Sprint,
		StartSneaking:   in.Sneak && !state.Sneaking,
		StopSneaking:    !in.Sneak && state.Sneaking,
		SneakDown:       in.Sneak,
		Sneaking:        in.Sneak,
		StartJumping:    in.Jump,
		Jumping:         in.Jump,
		StartSwimming:   in.Swim && !state.Swimming,
		StopSwimming:    !in.Swim && state.Swimming,
		StartGliding:    in.Glide && !state.Gliding,
		StopGliding:     !in.Glide && state.Gliding,
		UsingItem:       in.UsingItem,
	}
}

func run(sc *Scenario) {
	sim := bedsim.Simulator{
		World:     newWorld(sc.World),
		Effects:   sc.Effects,
		Inventory: inventory{sc.Elytra},
		Options:   bedsim.SimulationOptions{Mode: bedsim.SimulationModePassive, IgnoreClientStepTiebreaker: true},
	}
	state := &bedsim.MovementState{
		Pos:                  sc.Start,
		Size:                 bedsim.DefaultPlayerSize(),
		MovementSpeed:        0.1,
		DefaultMovementSpeed: 0.1,
		AirSpeed:             bedsim.WalkAirSpeed,
		OnGround:             sc.OnGround,
		HasGravity:           true,
		Ready:                true,
		Alive:                true,
		GameMode:             packet.GameTypeSurvival,
		TicksSinceKnockback:  1,
		TicksSinceTeleport:   1,
	}
	blocked := false
	for _, step := range sc.Steps {
		for i := 0; i < step.Repeat; i++ {
			if i == 0 && step.Teleport != nil {
				state.QueueTeleport(*step.Teleport, false, 0)
			}
			if i == 0 && step.Knockback != nil {
				state.QueueKnockback(*step.Knockback)
			}
			res := sim.Simulate(state, toInputState(state, step.Input, blocked))
			blocked = res.SprintMovementBlocked
			sc.Ticks = append(sc.Ticks, Tick{
				Pos: state.Pos, Vel: state.Vel, OnGround: state.OnGround,
				Collide:  [3]bool{state.CollideX, state.CollideY, state.CollideZ},
				Sneaking: state.Sneaking, Sprinting: state.Sprinting, Swimming: state.Swimming,
				Gliding: state.Gliding, FallDistance: state.FallDistance, Outcome: uint8(res.Outcome),
			})
		}
	}
}

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: diffharness <out.json>")
		os.Exit(2)
	}
	all := scenarios()
	for i := range all {
		run(&all[i])
	}
	data, err := json.Marshal(all)
	if err != nil {
		panic(err)
	}
	if err := os.WriteFile(os.Args[1], data, 0o644); err != nil {
		panic(err)
	}
	total := 0
	for _, sc := range all {
		total += len(sc.Ticks)
	}
	fmt.Printf("wrote %d scenarios, %d ticks\n", len(all), total)
}
