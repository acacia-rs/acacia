// Items: Java 26.3's registry (order = Java's network id), stack sizes from the component reports, placed
// blocks from BlockDump, Bedrock names from Geyser's items.json, item tags from the jar.
import { HEADER, camel, fail, read, str, write, writeTags } from './lib.mjs';

export function genItems(blockNames) {
  const dump = JSON.parse(read('java-26.3-block-dump.json')).items;
  const registry = JSON.parse(read('java-26.3-registries.json'))['minecraft:item'].entries;
  const stacks = JSON.parse(read('java-26.3-item-stacks.json'));
  const geyser = JSON.parse(read('geyser-items.json'));
  const blocks = new Set(blockNames);

  dump.forEach((it, i) => { if (registry[it.name]?.protocol_id !== i) fail(`${it.name}: registry order`); });
  // Bedrock merges some items under one name; a data value tells them apart only where it differs (beds and
  // banners carry their colour). Elsewhere Geyser's data values are pre-flattening leftovers: Bedrock 26.50
  // gives those items their own names, so they are ignored.
  const groups = new Map();
  for (const it of dump) {
    const g = geyser[it.name] ?? fail(`${it.name}: not in Geyser`);
    (groups.get(g.bedrock_identifier) ?? groups.set(g.bedrock_identifier, []).get(g.bedrock_identifier)).push({ java: it.name, data: g.bedrock_data ?? 0 });
  }
  const bedrockOf = new Map();
  const fromBedrock = [];
  for (const [name, members] of groups) {
    const byData = members.length > 1 && new Set(members.map((m) => m.data)).size === members.length;
    if (byData) {
      for (const m of members) { bedrockOf.set(m.java, [name, m.data]); fromBedrock.push([name, m.data, m.java]); }
    } else {
      const main = members.find((m) => m.java === name) ?? members[0];
      for (const m of members) bedrockOf.set(m.java, [name, 0]);
      fromBedrock.push([name, -1, main.java]);
    }
  }
  fromBedrock.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : a[1] - b[1]));

  const variants = dump.map((it) => camel(it.name));
  if (new Set(variants).size !== variants.length) fail('item variant names collide');
  const variant = new Map(dump.map((it, i) => [it.name, variants[i]]));
  let out = `${HEADER}use crate::block::Block;\nuse crate::item::ItemInfo;\n\n`;
  out += '/// Every item, in Java 26.3 registry order: `item as u16` is Java\'s network id.\n';
  out += '#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]\n#[repr(u16)]\npub enum Item {\n';
  out += variants.map((v) => `    ${v},\n`).join('');
  out += '}\n\n';
  out += `pub(crate) static ITEMS: [ItemInfo; ${dump.length}] = [\n`;
  for (const [i, it] of dump.entries()) {
    const stack = stacks[it.name.slice('minecraft:'.length)] ?? fail(`${it.name}: no stack size`);
    if (it.block && !blocks.has(it.block)) fail(`${it.name}: unknown block ${it.block}`);
    const block = it.block ? `Some(Block::${camel(it.block)})` : 'None';
    const [bn, bd] = bedrockOf.get(it.name);
    out += `    ItemInfo { item: Item::${variants[i]}, name: ${str(it.name)}, stack: ${stack}, block: ${block}, bedrock: (${str(bn)}, ${bd}) },\n`;
  }
  out += '];\n\n';
  out += '/// Bedrock (name, data, item), sorted by name then data; data -1 matches any.\n';
  out += `pub(crate) static FROM_BEDROCK: [(&str, i16, Item); ${fromBedrock.length}] = [\n`;
  out += fromBedrock.map(([n, d, j]) => `    (${str(n)}, ${d}, Item::${variant.get(j)}),\n`).join('');
  out += '];\n';
  write('items.rs', out);
  const tags = writeTags('item', 'ItemTag', dump.map((it) => it.name));
  return `${dump.length} items, ${fromBedrock.length} Bedrock names, ${tags} item tags`;
}
