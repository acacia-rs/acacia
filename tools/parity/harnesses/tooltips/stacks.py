"""Random item stacks (ItemStack.CODEC JSON) by tooltip area, for cases.py; one generator per area.

The firework generator is adapted from Horizon's tools/test_firework_tooltip_parity.py (MIT, (c) Ludovic Marie).
"""

PLAIN = ['stone', 'dirt', 'apple', 'oak_log', 'torch', 'ender_pearl', 'bucket', 'saddle', 'cake', 'golden_apple',
         'enchanted_golden_apple', 'nether_star', 'dragon_egg', 'white_wool', 'heart_of_the_sea', 'totem_of_undying',
         'elytra', 'trident', 'mace', 'experience_bottle', 'command_block', 'barrier', 'structure_void', 'beacon', 'conduit',
         'debug_stick', 'knowledge_book', 'creeper_head', 'wither_skeleton_skull', 'echo_shard', 'bundle', 'compass',
         'clock', 'spyglass', 'brush', 'recovery_compass', 'name_tag', 'lead', 'end_crystal', 'sculk_sensor']
TOOLS = ['wooden_sword', 'stone_sword', 'iron_sword', 'golden_sword', 'diamond_sword', 'netherite_sword', 'iron_pickaxe',
         'diamond_axe', 'netherite_shovel', 'golden_hoe', 'bow', 'crossbow', 'trident', 'mace', 'fishing_rod', 'shears',
         'flint_and_steel', 'carrot_on_a_stick', 'shield', 'elytra', 'brush', 'copper_sword', 'wooden_spear', 'diamond_spear']
ARMOR = ['leather_helmet', 'leather_chestplate', 'leather_leggings', 'leather_boots', 'chainmail_helmet', 'iron_chestplate',
         'golden_leggings', 'diamond_boots', 'netherite_helmet', 'netherite_chestplate', 'turtle_helmet', 'copper_boots',
         'diamond_horse_armor', 'leather_horse_armor', 'wolf_armor', 'netherite_nautilus_armor']
ENCHANTMENTS = ['protection', 'fire_protection', 'feather_falling', 'blast_protection', 'projectile_protection', 'respiration',
                'aqua_affinity', 'thorns', 'depth_strider', 'frost_walker', 'binding_curse', 'soul_speed', 'swift_sneak',
                'sharpness', 'smite', 'bane_of_arthropods', 'knockback', 'fire_aspect', 'looting', 'sweeping_edge',
                'efficiency', 'silk_touch', 'unbreaking', 'fortune', 'power', 'punch', 'flame', 'infinity',
                'luck_of_the_sea', 'lure', 'loyalty', 'impaling', 'riptide', 'channeling', 'multishot', 'quick_charge',
                'piercing', 'density', 'breach', 'wind_burst', 'mending', 'vanishing_curse', 'lunge']
ATTRIBUTES = ['armor', 'armor_toughness', 'attack_damage', 'attack_knockback', 'attack_speed', 'block_break_speed',
              'block_interaction_range', 'burning_time', 'entity_interaction_range', 'explosion_knockback_resistance',
              'fall_damage_multiplier', 'gravity', 'jump_strength', 'knockback_resistance', 'luck', 'max_absorption',
              'max_health', 'mining_efficiency', 'movement_efficiency', 'movement_speed', 'oxygen_bonus',
              'safe_fall_distance', 'scale', 'sneaking_speed', 'step_height', 'submerged_mining_speed',
              'sweeping_damage_ratio', 'water_movement_efficiency', 'waypoint_transmit_range', 'camera_distance']
SLOTS = ['any', 'mainhand', 'offhand', 'hand', 'feet', 'legs', 'chest', 'head', 'armor', 'body', 'saddle']
OPERATIONS = ['add_value', 'add_multiplied_base', 'add_multiplied_total']
POTIONS = ['water', 'mundane', 'thick', 'awkward', 'night_vision', 'long_night_vision', 'invisibility', 'leaping',
           'strong_leaping', 'fire_resistance', 'swiftness', 'long_swiftness', 'strong_swiftness', 'slowness',
           'strong_slowness', 'turtle_master', 'strong_turtle_master', 'water_breathing', 'healing', 'strong_healing',
           'harming', 'poison', 'strong_poison', 'regeneration', 'strength', 'strong_strength', 'weakness', 'luck',
           'slow_falling', 'wind_charged', 'weaving', 'oozing', 'infested']
EFFECTS = ['speed', 'slowness', 'haste', 'mining_fatigue', 'strength', 'instant_health', 'instant_damage', 'jump_boost',
           'nausea', 'regeneration', 'resistance', 'fire_resistance', 'water_breathing', 'invisibility', 'blindness',
           'night_vision', 'hunger', 'weakness', 'poison', 'wither', 'health_boost', 'absorption', 'saturation',
           'glowing', 'levitation', 'luck', 'unluck', 'slow_falling', 'conduit_power', 'dolphins_grace', 'bad_omen',
           'darkness', 'trial_omen', 'wind_charged', 'weaving', 'oozing', 'infested']
DYES = ['white', 'orange', 'magenta', 'light_blue', 'yellow', 'lime', 'pink', 'gray', 'light_gray', 'cyan', 'purple',
        'blue', 'brown', 'green', 'red', 'black']
FIREWORK_DYES = [0xF0F0F0, 0xEB8844, 0xC354CD, 0x6689D3, 0xDECF2A, 0x41CD34, 0xD88198, 0x434343, 0xABABAB, 0x287697,
                 0x7B2FBE, 0x253192, 0x51301A, 0x3B511A, 0xB3312C, 0x1E1B1B]
TRIM_PATTERNS = ['sentry', 'dune', 'coast', 'wild', 'ward', 'eye', 'vex', 'tide', 'snout', 'rib', 'spire', 'wayfinder',
                 'shaper', 'silence', 'raiser', 'host', 'flow', 'bolt']
TRIM_MATERIALS = ['quartz', 'iron', 'netherite', 'redstone', 'copper', 'gold', 'emerald', 'diamond', 'lapis', 'amethyst', 'resin']
BANNER_PATTERNS = ['square_bottom_left', 'stripe_bottom', 'stripe_top', 'cross', 'border', 'creeper', 'skull', 'flower',
                   'mojang', 'globe', 'piglin', 'flow', 'guster', 'bricks', 'gradient', 'circle', 'rhombus',
                   'half_horizontal', 'curly_border', 'triangles_bottom', 'small_stripes', 'diagonal_left']
INSTRUMENTS = ['ponder', 'sing', 'seek', 'feel', 'admire', 'call', 'yearn', 'dream']
SONGS = ['13', 'cat', 'blocks', 'chirp', 'far', 'mall', 'mellohi', 'stal', 'strad', 'ward', '11', 'wait', 'pigstep',
         'otherside', '5', 'relic', 'precipice', 'creator', 'creator_music_box', 'tears', 'lava_chicken']
PAINTINGS = ['kebab', 'aztec', 'alban', 'wasteland', 'pool', 'courbet', 'sea', 'sunset', 'creebet', 'wanderer', 'graham',
             'match', 'bust', 'stage', 'void', 'skull_and_roses', 'wither', 'fighters', 'pointer', 'pigscene',
             'burning_skull', 'skeleton', 'donkey_kong', 'earth', 'wind', 'water', 'fire', 'baroque', 'humble',
             'meditative', 'prairie_ride', 'backyard', 'bouquet', 'cavebird', 'changing', 'cotan', 'endboss', 'fern']
FISH = ['kob', 'sunstreak', 'snooper', 'dasher', 'brinely', 'spotty', 'flopper', 'stripey', 'glitter', 'blockfish', 'betty', 'clayfish']
SHERDS = ['brick', 'angler_pottery_sherd', 'archer_pottery_sherd', 'heart_pottery_sherd', 'skull_pottery_sherd', 'flow_pottery_sherd']
ENTITIES = ['zombie', 'creeper', 'pig', 'bee', 'ender_dragon', 'wither', 'villager', 'allay', 'tnt', 'armor_stand', 'falling_block']
BLOCKS = ['stone', 'dirt', 'oak_planks', 'obsidian', 'diamond_ore', 'glass', 'sand', 'spawner', 'chest', 'water']
BLOCK_TAGS = ['#minecraft:logs', '#minecraft:wool', '#minecraft:planks', '#minecraft:leaves', '#minecraft:anvil', '#minecraft:beds']
NAMES = ['Named', 'Excalibur', 'x', 'A rather long custom item name for a tooltip', 'Sword of Many Words and More']
STYLED = [{'text': 'Fancy', 'color': 'gold'}, {'text': 'Bold', 'bold': True}, {'text': 'Plain', 'italic': False},
          {'translate': 'item.minecraft.diamond'}, {'text': 'Hex', 'color': '#12AB34'}, ['A', {'text': 'B', 'color': 'red'}]]
LORE = ['Lore line', 'Second line of lore', {'text': 'Red lore', 'color': 'red'}, {'text': 'Not italic', 'italic': False},
        '', {'translate': 'block.minecraft.stone'}, 'A long line of lore text that will make the tooltip wide']
TEXTS = ['Hello', 'Tooltip line', 'A longer tooltip line with words', '', 'x', '§cLegacy red', 'Wide' * 12]


def mc(name):
    return 'minecraft:' + name


def stack(item, components=None, count=1):
    s = {'id': mc(item), 'count': count}
    if components:
        s['components'] = components
    return s


def name(rng):
    return rng.choice(NAMES + STYLED)


def enchantments(rng, n=None):
    chosen = rng.sample(ENCHANTMENTS, n if n is not None else rng.choice([1, 1, 2, 3, 5, 8]))
    return {mc(e): rng.choice([1, 1, 2, 3, 4, 5, 10, 255]) for e in chosen}


def modifiers(rng):
    out = []
    for i in range(rng.choice([1, 1, 2, 3, 5])):
        m = {'type': mc(rng.choice(ATTRIBUTES)), 'id': f'test:m{i}',
             'amount': rng.choice([0, 1, -1, 0.5, -0.25, 2.345, 0.005, 0.125, 10, -3.5, 100, 0.1, 1e-3]),
             'operation': rng.choice(OPERATIONS), 'slot': rng.choice(SLOTS)}
        if rng.random() < 0.15:
            m['id'] = rng.choice(['minecraft:base_attack_damage', 'minecraft:base_attack_speed'])
        if rng.random() < 0.15:
            m['display'] = rng.choice([{'type': 'hidden'}, {'type': 'override', 'value': {'text': 'Override', 'color': 'aqua'}}, {'type': 'default'}])
        out.append(m)
    return out


def explosion(rng):
    def colors():
        return [rng.choice(FIREWORK_DYES) if rng.random() < 0.8 else rng.randrange(1 << 24) for _ in range(rng.choice([0, 1, 1, 2, 3]))]
    e = {'shape': rng.choice(['small_ball', 'large_ball', 'star', 'creeper', 'burst'])}
    for key, values in (('colors', colors()), ('fade_colors', colors() if rng.random() < 0.4 else [])):
        if values:
            e[key] = values
    if rng.random() < 0.4:
        e['has_trail'] = True
    if rng.random() < 0.4:
        e['has_twinkle'] = True
    return e


def firework(rng):
    if rng.random() < 0.7:
        if rng.random() < 0.1:
            return stack('firework_rocket')
        explosions = []
        for _ in range(rng.choice([0, 0, 1, 1, 2, 3, 6])):
            explosions.append(explosions[-1] if explosions and rng.random() < 0.4 else explosion(rng))
        return stack('firework_rocket', {mc('fireworks'): {'flight_duration': rng.choice([0, 1, 1, 2, 3, 3, 7, 255]), 'explosions': explosions}})
    return stack('firework_star', {mc('firework_explosion'): explosion(rng)} if rng.random() < 0.9 else None)


def effect(rng):
    e = {'id': mc(rng.choice(EFFECTS)), 'amplifier': rng.choice([0, 0, 1, 2, 4, 9, 10, 127]),
         # No infinite (-1) durations: "∞" is outside the probe's stand-in ASCII font.
         'duration': rng.choice([0, 1, 20, 21, 600, 1199, 3600, 72000, 1728000, rng.randint(0, 100000)])}
    return e


def potion(rng):
    item = rng.choice(['potion', 'splash_potion', 'lingering_potion', 'tipped_arrow'])
    contents = {}
    if rng.random() < 0.85:
        contents['potion'] = mc(rng.choice(POTIONS))
    if rng.random() < 0.35:
        contents['custom_effects'] = [effect(rng) for _ in range(rng.choice([1, 1, 2, 3]))]
    if rng.random() < 0.1:
        contents['custom_name'] = 'test'
    if rng.random() < 0.1:
        contents['custom_color'] = rng.randrange(1 << 24)
    components = {mc('potion_contents'): contents}
    if rng.random() < 0.2:
        components[mc('potion_duration_scale')] = rng.choice([0.25, 0.125, 2.0, 0.0])
    return stack(item, components)


def predicate(rng):
    r = rng.random()
    if r < 0.15:
        return {}
    if r < 0.5:
        return {'blocks': mc(rng.choice(BLOCKS))}
    if r < 0.75:
        return {'blocks': [mc(b) for b in rng.sample(BLOCKS, rng.randint(1, 4))]}
    return {'blocks': rng.choice(BLOCK_TAGS)}


def adventure(rng):
    components = {}
    for key in ('can_break', 'can_place_on'):
        if rng.random() < 0.6:
            preds = [predicate(rng) for _ in range(rng.choice([1, 1, 2]))]
            components[mc(key)] = preds[0] if len(preds) == 1 and rng.random() < 0.5 else preds
    return stack(rng.choice(TOOLS + PLAIN), components or {mc('can_break'): {'blocks': mc('stone')}})


def template(rng, count=None):
    return stack(rng.choice(PLAIN + TOOLS + SHERDS), count=count or rng.choice([1, 1, 16, 64]))


def container(rng):
    item = rng.choice(['shulker_box', 'red_shulker_box', 'chest', 'barrel', 'dispenser'])
    if rng.random() < 0.2:
        return stack(item, {mc('container_loot'): {'loot_table': 'minecraft:chests/simple_dungeon'}})
    slots = rng.sample(range(27), rng.choice([0, 1, 3, 5, 6, 9, 27]))
    items = []
    for s in sorted(slots):
        i = template(rng)
        if rng.random() < 0.15:
            i['components'] = {mc('custom_name'): name(rng)}
        items.append({'slot': s, 'item': i})
    return stack(item, {mc('container'): items})


def bundle(rng):
    items = [template(rng, rng.choice([1, 2, 8])) for _ in range(rng.choice([0, 0, 1, 3, 4, 5, 8, 12, 13, 20]))]
    c = {mc('bundle_contents'): items}
    if rng.random() < 0.2:
        c[mc('tooltip_display')] = {'hidden_components': [mc('bundle_contents')]}
    return stack(rng.choice(['bundle', 'red_bundle']), c)
