"""The item-tooltip areas cases.py draws from: each a generator of one stack (ItemStack.CODEC JSON)."""
from stacks import (ARMOR, BANNER_PATTERNS, BLOCKS, DYES, ENTITIES, FISH, INSTRUMENTS, LORE, PAINTINGS, PLAIN, SHERDS, SONGS,
                    TOOLS, TRIM_MATERIALS, TRIM_PATTERNS, adventure, bundle, container, effect, enchantments, firework, mc,
                    modifiers, name, potion, stack, template)


def plain(rng):
    item = rng.choice(PLAIN + TOOLS + ARMOR)
    c = {}
    if rng.random() < 0.3:
        c[mc('custom_name')] = name(rng)
    if rng.random() < 0.15:
        c[mc('item_name')] = name(rng)
    if rng.random() < 0.2:
        c[mc('rarity')] = rng.choice(['common', 'uncommon', 'rare', 'epic'])
    if rng.random() < 0.3:
        c[mc('lore')] = rng.sample(LORE, rng.randint(1, 4))
    if rng.random() < 0.1:
        c[mc('unbreakable')] = {}
    if rng.random() < 0.08:
        c[mc('intangible_projectile')] = {}
    if rng.random() < 0.2:
        c[mc('damage')] = rng.choice([0, 1, 5, 100])
    return stack(item, c, rng.choice([1, 1, 2, 64]))


def enchanted(rng):
    r = rng.random()
    if r < 0.3:
        return stack('enchanted_book', {mc('stored_enchantments'): enchantments(rng)})
    item = rng.choice(TOOLS + ARMOR + PLAIN)
    c = {mc('enchantments'): enchantments(rng)}
    if rng.random() < 0.2:
        c[mc('rarity')] = rng.choice(['common', 'uncommon', 'rare', 'epic'])
    if rng.random() < 0.2:
        c[mc('stored_enchantments')] = enchantments(rng, 1)
    return stack(item, c)


def attributes(rng):
    item = rng.choice(TOOLS + ARMOR + PLAIN)
    if rng.random() < 0.35:
        return stack(item)
    c = {mc('attribute_modifiers'): modifiers(rng)}
    if rng.random() < 0.3:
        c[mc('enchantments')] = enchantments(rng, rng.randint(1, 3))
    return stack(item, c)


def display(rng):
    """tooltip_display: hide the whole tooltip or some components."""
    base = rng.choice([plain, enchanted, attributes, firework, potion, container])(rng)
    c = base.setdefault('components', {})
    if rng.random() < 0.3:
        c[mc('tooltip_display')] = {'hide_tooltip': True}
    else:
        hidden = [mc(x) for x in ['enchantments', 'attribute_modifiers', 'lore', 'unbreakable', 'stored_enchantments',
                                  'fireworks', 'firework_explosion', 'potion_contents', 'container', 'damage', 'dyed_color', 'trim']]
        c[mc('tooltip_display')] = {'hidden_components': rng.sample(hidden, rng.randint(1, 4))}
    if rng.random() < 0.3:
        c[mc('block_entity_data')] = {'id': mc('command_block')}
        base['id'] = mc('command_block')
    return base


def styled(rng):
    s = plain(rng)
    s.setdefault('components', {})[mc('tooltip_style')] = rng.choice(['minecraft:fancy', 'test:custom', 'minecraft:missing'])
    return s


def dyed(rng):
    item = rng.choice(['leather_helmet', 'leather_chestplate', 'leather_boots', 'leather_horse_armor', 'wolf_armor'])
    c = {mc('dyed_color'): rng.randrange(1 << 24)} if rng.random() < 0.8 else {}
    if rng.random() < 0.5:
        c[mc('trim')] = {'pattern': mc(rng.choice(TRIM_PATTERNS)), 'material': mc(rng.choice(TRIM_MATERIALS))}
    return stack(item, c)


def trimmed(rng):
    c = {mc('trim'): {'pattern': mc(rng.choice(TRIM_PATTERNS)), 'material': mc(rng.choice(TRIM_MATERIALS))}}
    if rng.random() < 0.2:
        return stack(rng.choice(TRIM_PATTERNS) + '_armor_trim_smithing_template')
    if rng.random() < 0.1:
        return stack('netherite_upgrade_smithing_template')
    return stack(rng.choice(ARMOR), c)


def music(rng):
    r = rng.random()
    if r < 0.4:
        return stack('music_disc_' + rng.choice(SONGS))
    if r < 0.55:
        return stack('disc_fragment_5')
    if r < 0.8:
        return stack('goat_horn', {mc('instrument'): mc(rng.choice(INSTRUMENTS) + '_goat_horn')})
    return stack(rng.choice(PLAIN), {mc('jukebox_playable'): mc(rng.choice(SONGS))})


def banner(rng):
    layers = [{'pattern': mc(rng.choice(BANNER_PATTERNS)), 'color': rng.choice(DYES)} for _ in range(rng.choice([0, 1, 2, 6, 7, 9]))]
    if rng.random() < 0.4:
        c = {mc('banner_patterns'): layers}
        if rng.random() < 0.7:
            c[mc('base_color')] = rng.choice(DYES)
        return stack('shield', c)
    return stack(rng.choice(DYES) + '_banner', {mc('banner_patterns'): layers})


def book(rng):
    if rng.random() < 0.2:
        return stack('writable_book', {mc('writable_book_content'): {'pages': ['a']}})
    content = {'title': rng.choice(['Book', ' ', 'A long book title', '']), 'author': rng.choice(['Steve', '', 'Alex']),
               'generation': rng.choice([0, 1, 2, 3]), 'pages': []}
    return stack('written_book', {mc('written_book_content'): content})


def creatures(rng):
    r = rng.random()
    if r < 0.25:
        bees = [{'entity_data': {'id': mc('bee')}, 'ticks_in_hive': 0, 'min_ticks_in_hive': 0} for _ in range(rng.randint(0, 3))]
        c = {mc('bees'): bees}
        if rng.random() < 0.6:
            c[mc('block_state')] = {'honey_level': str(rng.randint(0, 5))}
        return stack(rng.choice(['beehive', 'bee_nest']), c)
    if r < 0.5:
        egg = rng.choice(['zombie', 'creeper', 'pig', 'bee', 'ghast', 'slime', 'villager'])
        c = {}
        if rng.random() < 0.5:
            c[mc('entity_data')] = {'id': mc(rng.choice(ENTITIES))}
        return stack(egg + '_spawn_egg', c)
    if r < 0.7:
        c = {}
        if rng.random() < 0.6:
            c[mc('block_entity_data')] = {'id': mc('mob_spawner'), 'SpawnData': {'entity': {'id': mc(rng.choice(ENTITIES))}}}
        return stack(rng.choice(['spawner', 'trial_spawner']), c)
    if r < 0.85:
        c = {}
        if rng.random() < 0.8:
            c[mc('tropical_fish/pattern')] = rng.choice(FISH)
            c[mc('tropical_fish/base_color')] = rng.choice(DYES)
            c[mc('tropical_fish/pattern_color')] = rng.choice(DYES)
        return stack('tropical_fish_bucket', c)
    c = {mc('painting/variant'): mc(rng.choice(PAINTINGS))} if rng.random() < 0.7 else {}
    return stack('painting', c)


def gadgets(rng):
    r = rng.random()
    if r < 0.2:
        return stack('filled_map', {mc('map_id'): rng.choice([0, 1, 2, 7])})
    if r < 0.3:
        return stack('compass', {mc('lodestone_tracker'): {}})
    if r < 0.45:
        profile = rng.choice(['Notch', {'name': 'jeb_'}, {'id': [1, 2, 3, 4]}, {'name': 'Dinnerbone', 'id': [5, 6, 7, 8]}])
        return stack('player_head', {mc('profile'): profile})
    if r < 0.6:
        projectiles = [rng.choice([stack('arrow'), stack('spectral_arrow'), firework(rng), potion(rng)]) for _ in range(rng.choice([1, 1, 2, 3]))]
        if len(projectiles) > 1 and rng.random() < 0.5:
            projectiles[1] = projectiles[0]
        return stack('crossbow', {mc('charged_projectiles'): projectiles})
    if r < 0.75:
        sides = {side: {'id': mc(rng.choice(SHERDS))} for side in ('back', 'left', 'right', 'front') if rng.random() < 0.7}
        return stack('decorated_pot', {mc('pot_decorations'): sides})
    if r < 0.85:
        sides = {}
        for key in ('sign_text_front', 'sign_text_back'):
            if rng.random() < 0.6:
                sides[mc(key)] = {'messages': [rng.choice(['', 'Hi', 'Sign text']) for _ in range(4)]}
        return stack(rng.choice(['oak_sign', 'birch_hanging_sign']), sides)
    if r < 0.92:
        return stack('ominous_bottle', {mc('ominous_bottle_amplifier'): rng.randint(0, 4)})
    return stack('suspicious_stew', {mc('suspicious_stew_effects'): [{'id': effect(rng)['id'], 'duration': rng.choice([160, 7, 600])}]})


def mixed(rng):
    """Several areas on one stack."""
    s = rng.choice([enchanted, attributes, plain])(rng)
    c = s.setdefault('components', {})
    for gen in rng.sample([firework, potion, adventure, container, dyed, trimmed], rng.randint(1, 3)):
        c.update(gen(rng).get('components', {}))
    return s


AREAS = {'plain': plain, 'enchanted': enchanted, 'attributes': attributes, 'display': display, 'styled': styled,
         'dyed': dyed, 'trimmed': trimmed, 'music': music, 'banner': banner, 'book': book, 'creatures': creatures,
         'gadgets': gadgets, 'firework': firework, 'potion': potion, 'adventure': adventure, 'container': container,
         'bundle': bundle, 'mixed': mixed, 'blocks': lambda rng: stack(rng.choice(BLOCKS[:-1] + SHERDS))}
