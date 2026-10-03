package main

func floor(kind string) Fill { return Fill{kind, [3]int{-8, -1, -8}, [3]int{8, -1, 24}} }

func box(kind string, x0, y0, z0, x1, y1, z1 int) Fill {
	return Fill{kind, [3]int{x0, y0, z0}, [3]int{x1, y1, z1}}
}

func steps(parts ...Step) []Step { return parts }

func hold(in Input, ticks int) Step { return Step{Input: in, Repeat: ticks} }

func amp(v int32) *int32 { return &v }

var (
	idle     = Input{}
	forward  = Input{Move: [2]float32{0, 1}}
	sprint   = Input{Move: [2]float32{0, 1}, Sprint: true}
	sprintUp = Input{Move: [2]float32{0, 1}, Sprint: true, Jump: true}
	jump     = Input{Jump: true}
	jumpFwd  = Input{Move: [2]float32{0, 1}, Jump: true}
	sneakFwd = Input{Move: [2]float32{0, 1}, Sneak: true}
)

func ground(name string, world []Fill, s ...Step) Scenario {
	return Scenario{Name: name, World: world, Start: [3]float32{0.5, 0, 0.5}, OnGround: true, Steps: s}
}

func scenarios() []Scenario {
	flat := []Fill{floor("stone")}
	pool := []Fill{floor("stone"), box("stone", -3, -1, 2, 3, 3, 8), box("water", -2, 0, 3, 2, 3, 7)}
	list := []Scenario{
		ground("idle", flat, hold(idle, 20)),
		ground("walk_forward", flat, hold(forward, 40), hold(idle, 15)),
		ground("walk_diagonal_yaw", flat, hold(Input{Move: [2]float32{0.7071, 0.7071}, Yaw: 37.5}, 30),
			hold(Input{Move: [2]float32{-1, 0}, Yaw: -120.25}, 20)),
		ground("sprint_jump", flat, hold(sprint, 10), hold(sprintUp, 30), hold(idle, 20)),
		ground("jump_in_place", flat, hold(jump, 30), hold(idle, 10)),
		{Name: "fall_from_height", World: flat, Start: [3]float32{0.5, 20, 0.5}, Steps: steps(hold(idle, 60))},
		ground("walk_into_wall", []Fill{floor("stone"), box("stone", -8, 0, 3, 8, 2, 3)},
			hold(forward, 30), hold(sprint, 20)),
		ground("walk_into_wall_diagonal", []Fill{floor("stone"), box("stone", -8, 0, 3, 8, 2, 3)},
			hold(Input{Move: [2]float32{0, 1}, Yaw: -30}, 40)),
		ground("step_up_slab", []Fill{floor("stone"), box("slab", -8, 0, 3, 8, 0, 24)}, hold(forward, 40)),
		ground("block_no_step", []Fill{floor("stone"), box("stone", -8, 0, 3, 8, 0, 24)}, hold(forward, 30), hold(jumpFwd, 20)),
		ground("stairs", []Fill{floor("stone"), box("stair", -8, 0, 3, 8, 0, 3), box("stone", -8, 0, 4, 8, 0, 24),
			box("stair", -8, 1, 4, 8, 1, 4), box("stone", -8, 1, 5, 8, 1, 24)}, hold(forward, 50)),
		ground("ladder_climb", []Fill{floor("stone"), box("stone", -8, 0, 4, 8, 12, 4), box("ladder", 0, 0, 3, 0, 8, 3)},
			hold(forward, 20), hold(jumpFwd, 40), hold(forward, 10), hold(Input{Move: [2]float32{0, 1}, Sneak: true}, 10), hold(idle, 30)),
		{Name: "vine_descend", World: []Fill{floor("stone"), box("vine", 0, 0, 0, 0, 10, 0)}, Start: [3]float32{0.5, 6, 0.5},
			Steps: steps(hold(idle, 10), hold(Input{Sneak: true}, 10), hold(idle, 30))},
		ground("ice_walk", []Fill{floor("ice")}, hold(forward, 20), hold(idle, 40)),
		ground("blue_ice_sprint", []Fill{floor("blue_ice")}, hold(sprint, 20), hold(sprintUp, 20), hold(idle, 30)),
		ground("soul_sand_walk", []Fill{floor("stone"), box("soul_sand", -8, -1, 0, 8, -1, 24)}, hold(idle, 3), hold(forward, 40)),
		ground("slime_walk", []Fill{floor("slime")}, hold(forward, 30)),
		{Name: "slime_bounce", World: []Fill{floor("slime")}, Start: [3]float32{0.5, 8, 0.5}, Steps: steps(hold(idle, 80))},
		{Name: "bed_bounce", World: []Fill{floor("stone"), box("bed", -2, 0, -2, 2, 0, 2)}, Start: [3]float32{0.5, 5, 0.5}, Steps: steps(hold(idle, 50))},
		ground("honey_walk", []Fill{floor("honey")}, hold(idle, 2), hold(forward, 30), hold(jump, 20)),
		ground("cobweb", []Fill{floor("stone"), box("web", -8, 0, 2, 8, 1, 4)}, hold(forward, 60)),
		ground("powder_snow", []Fill{floor("stone"), box("powder_snow", -8, 0, 2, 8, 1, 4)}, hold(forward, 40), hold(jumpFwd, 20)),
		ground("berry_bush", []Fill{floor("stone"), box("berry_bush", -8, 0, 2, 8, 0, 4)}, hold(sprint, 40)),
		ground("sneak_edge", []Fill{box("stone", -2, -1, -2, 2, -1, 2)}, hold(sneakFwd, 40), hold(Input{Move: [2]float32{1, 1}, Sneak: true, Yaw: 45}, 20)),
		ground("sneak_toggle", flat, hold(sneakFwd, 15), hold(forward, 15), hold(sneakFwd, 10)),
		ground("sneak_low_ceiling", []Fill{floor("stone"), box("stone", -8, 1, 3, 8, 1, 10)},
			hold(sneakFwd, 30), hold(forward, 20), hold(idle, 5)),
		ground("using_item_walk", flat, hold(Input{Move: [2]float32{0, 1}, UsingItem: true}, 30)),
		{Name: "water_fall_in", World: pool, Start: [3]float32{0.5, 6, 5.5}, Steps: steps(hold(idle, 60), hold(jump, 60))},
		{Name: "water_swim", World: pool, Start: [3]float32{0.5, 1.2, 3.5}, Steps: steps(
			hold(Input{Move: [2]float32{0, 1}, Sprint: true}, 5),
			hold(Input{Move: [2]float32{0, 1}, Sprint: true, Swim: true, Pitch: 30}, 20),
			hold(Input{Move: [2]float32{0, 1}, Sprint: true, Swim: true, Pitch: -40}, 20),
			hold(idle, 20))},
		{Name: "water_exit_ledge", World: pool, Start: [3]float32{0.5, 2.5, 5.5}, Steps: steps(
			hold(Input{Move: [2]float32{0, 1}, Jump: true}, 60))},
		{Name: "water_flow", World: []Fill{floor("stone"), box("water", -4, 0, 0, 4, 0, 0), box("water@7", -4, 0, 1, 4, 0, 1),
			box("water@6", -4, 0, 2, 4, 0, 2), box("water@5", -4, 0, 3, 4, 0, 3)}, Start: [3]float32{0.5, 0, 0.5}, OnGround: true,
			Steps: steps(hold(idle, 40))},
		{Name: "waterfall", World: []Fill{floor("stone"), box("water_falling", 0, 0, 0, 0, 10, 0), box("stone", 1, 0, 0, 1, 10, 0)},
			Start: [3]float32{0.5, 8, 0.5}, Steps: steps(hold(idle, 40))},
		{Name: "lava", World: []Fill{floor("stone"), box("lava", -4, 0, -4, 4, 1, 4)}, Start: [3]float32{0.5, 3, 0.5},
			Steps: steps(hold(idle, 30), hold(jumpFwd, 30))},
		{Name: "jump_boost", World: flat, Start: [3]float32{0.5, 0, 0.5}, OnGround: true, Effects: Effects{JumpBoost: amp(1)},
			Steps: steps(hold(jump, 40))},
		{Name: "levitation", World: []Fill{floor("stone"), box("stone", -8, 6, -8, 8, 6, 8)}, Start: [3]float32{0.5, 0, 0.5}, OnGround: true,
			Effects: Effects{Levitation: amp(2)}, Steps: steps(hold(forward, 80))},
		{Name: "slow_falling", World: flat, Start: [3]float32{0.5, 10, 0.5}, Effects: Effects{SlowFalling: true},
			Steps: steps(hold(forward, 60), hold(jump, 30))},
		{Name: "weaving_web", World: []Fill{floor("stone"), box("web", -8, 0, 2, 8, 1, 4)}, Start: [3]float32{0.5, 0, 0.5}, OnGround: true,
			Effects: Effects{Weaving: true}, Steps: steps(hold(forward, 40))},
		ground("teleport", flat, hold(forward, 10), Step{Input: forward, Repeat: 1, Teleport: &[3]float32{3.25, 4, -2.75}},
			hold(forward, 30)),
		ground("knockback", flat, hold(forward, 5), Step{Input: idle, Repeat: 30, Knockback: &[3]float32{0.4, 0.36, -0.2}}),
		{Name: "elytra_glide", World: []Fill{box("stone", -80, -1, -80, 80, -1, 80)}, Start: [3]float32{0.5, 40, 0.5}, Elytra: true, Steps: steps(
			hold(idle, 5), hold(Input{Glide: true, Pitch: 20}, 30), hold(Input{Glide: true, Pitch: -25, Yaw: 70}, 20),
			hold(Input{Glide: true, Pitch: 45, Yaw: 70}, 40), hold(idle, 20))},
	}
	return list
}
