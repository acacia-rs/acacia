# acacia-sound

Game sounds for an Acacia client: which sample a sound event plays, decoding it, and playing it
where it happens. No game state: the caller says what happened and where the listener is.

## What plays (`defs.rs`)

Bedrock's tables, from a resource pack: `sounds.json` maps block sound types (`block_sounds`,
`interactive_sounds.block_sounds` for steps and falls), entities (`entity_sounds`, with
`defaults`) and world events (`individual_event_sounds`) to sound names, each with a volume and
pitch that may be a `[min, max]` range (scaled by the group's own). `sounds/sound_definitions.json`
maps a sound name to the files it picks from by weight, each with its own volume and pitch, and a
`max_distance`. A block's sound type is the `sound` of `blocks.json`; blocks without one sound like
stone.

`Sounds::play(Event, at, volume, pitch, listener)` takes a definition by name (`PlaySound`), a
block's event (`place`, `break`, `hit`, `step`), an entity's event or a world event.

## Decoding

- `fsb/`: the pack's FMOD FSB5 files, one sample each, FADPCM (most) or PCM16, ported from
  vgmstream (`fsb5.c`, `fadpcm_decoder.c`).
- `ogg.rs`: Ogg Vorbis (Java's sounds) with lewton.

A file is looked for root by root, `.ogg` before `.fsb`, and decoded once.

## Playing (`player.rs`)

rodio's default output; without a device (a headless machine) nothing plays. Each voice's volume
falls linearly to silence at its range (the definition's `max_distance`, else 16 blocks, times the
volume when above 1, as Java does), and the far ear keeps 30% of a sound to one side. The position
is fixed when the sound starts.

## Not yet

Java's own event names (the Java look plays Bedrock's events with Java's `.ogg` where a file of the
same path exists; its files are not downloaded yet), music and ambience, `StopSound`, looping
sounds, `min_distance`, sound categories and volume settings, entity sound variants (`variants`).
