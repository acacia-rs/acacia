//! Builds acacia-auth's skin pool (`assets/skins`) from default-character skins.
//! - `--capture <mitm.jsonl> <out dir>`: the game's own in-session skin changes from a tools/mitm
//!   capture (change character in the Dressing Room while joined). This is the working source.
//! - `<server> @account <seconds> [out dir]`: other players' skins from PlayerList. Big networks
//!   flatten skins, so this mostly prints why each was rejected.
//! Keeps only all-default persona pieces without cape or premium (same on everyone, nothing personal),
//! converted to Login claims byte-exact (styled.rs) with the persona id zeroed. One file per look.
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use acacia_client::auth::login::Skin;
use acacia_client::auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_client::proto::packets::{PlayerList, PlayerSkin};
use acacia_client::proto::types::{PlayerRecordContent, Skin as WireSkin, SkinArmSize};
use acacia_client::{Client, Event};
use serde_json::json;
use styled::styled;

mod styled;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Look {
    players: usize,
    persona_ids: BTreeSet<String>,
}

struct Harvest {
    out: PathBuf,
    looks: BTreeMap<String, Look>,
    seen: usize,
    rejected: BTreeMap<String, usize>,
}

impl Harvest {
    fn add(&mut self, wire: &WireSkin) -> Result<()> {
        self.seen += 1;
        let Some((look, mut skin)) = default_character(wire) else {
            *self.rejected.entry(shape(wire)).or_default() += 1;
            return Ok(());
        };
        let entry = self.looks.entry(look.clone()).or_default();
        entry.players += 1;
        entry.persona_ids.insert(skin.skin_id.clone());
        if entry.players == 1 {
            skin.personalise("0000000000000000");
            std::fs::write(self.out.join(format!("{look}.json")), serde_json::to_vec_pretty(&skin)?)?;
            println!("new look {look} ({} pieces)", skin.persona_pieces.len());
        }
        Ok(())
    }

    fn finish(self) -> Result<()> {
        let mut summary = format!("{} skins seen, {} not a default character\n", self.seen, self.rejected.values().sum::<usize>());
        for (s, n) in &self.rejected {
            summary += &format!("rejected x{n}: {s}\n");
        }
        for (look, l) in &self.looks {
            summary += &format!("{look}: {} players, {} distinct skin ids {:?}\n", l.players, l.persona_ids.len(), l.persona_ids);
        }
        print!("{summary}");
        Ok(std::fs::write(self.out.join("summary.txt"), summary)?)
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [flag, capture, out] = &args[..]
        && flag == "--capture"
    {
        return from_capture(capture, harvest(out)?);
    }
    let [server, account, secs, ..] = &args[..] else {
        return Err("usage: <server> @account <seconds> [out dir] | --capture <mitm.jsonl> <out dir>".into());
    };
    let mut harvest = harvest(args.get(3).map_or(".testserver/skins-harvest", String::as_str))?;

    let auth = Account::new(
        Arc::new(AuthClient::new(AuthConfig::default())?),
        Arc::new(FileTokenCache::new(".tokens")?),
        account.trim_start_matches('@'),
    );
    let (key, credentials) = auth.login_credentials().await?;
    let mut client = Client::builder(server).online(credentials, key).connect().await?;
    println!("spawned; collecting for {secs} s");

    let deadline = tokio::time::sleep(Duration::from_secs(secs.parse()?));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            event = client.recv() => match event {
                Some(Event::Packet(p)) => {
                    let skins: Vec<WireSkin> = if let Ok(list) = p.decode::<PlayerList>() {
                        list.records.into_iter().filter_map(|r| match r.content {
                            PlayerRecordContent::Add(add) => Some(add.skin_data),
                            _ => None,
                        }).collect()
                    } else if let Ok(change) = p.decode::<PlayerSkin>() {
                        vec![change.skin]
                    } else {
                        continue;
                    };
                    for wire in &skins {
                        harvest.add(wire)?;
                    }
                }
                Some(Event::Disconnected(reason)) => { println!("disconnected: {reason:?}"); break; }
                None => break,
            },
            _ = &mut deadline => client.close(),
        }
    }
    harvest.finish()
}

fn harvest(out: &str) -> Result<Harvest> {
    std::fs::create_dir_all(out)?;
    Ok(Harvest { out: out.into(), looks: BTreeMap::new(), seen: 0, rejected: BTreeMap::new() })
}

/// The game's own in-session skin changes (`PlayerSkin`, game to server) from a tools/mitm capture.
fn from_capture(path: &str, mut harvest: Harvest) -> Result<()> {
    for line in std::fs::read_to_string(path)?.lines() {
        let v: serde_json::Value = serde_json::from_str(line)?;
        if v["dir"] != "C>S" || v["name"] != "PlayerSkin" {
            continue;
        }
        let hex = v["raw"].as_str().unwrap_or_default();
        let raw: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16)).collect::<std::result::Result<_, _>>()?;
        harvest.add(&PlayerSkin::read(&mut &raw[..])?.skin)?;
    }
    harvest.finish()
}

/// The skin as Login claims, keyed by its look, when it is an unmodified default character.
fn default_character(w: &WireSkin) -> Option<(String, Skin)> {
    let default = w.persona
        && !w.premium
        && w.cape_id.is_empty()
        && !w.geometry_data.is_empty()
        && !w.personal_pieces.is_empty()
        && w.personal_pieces.iter().all(|p| p.is_default_piece && p.product_id.is_empty());
    if !default {
        return None;
    }
    // The wire carries colours byte-reversed relative to Login's #AARRGGBB.
    let colour = |c: i32| format!("#{:x}", (c as u32).swap_bytes());
    let pieces: Vec<_> = w.personal_pieces.iter().map(|p| {
        json!({
            "IsDefault": true,
            "PackId": p.pack_id.to_string(),
            "PieceId": p.piece_id,
            "PieceType": format!("persona_{}", snake(&format!("{:?}", p.piece_type))),
            "ProductId": "",
        })
    }).collect();
    let tints: Vec<_> = w.piece_tint_colors.iter().map(|t| {
        json!({"Colors": t.colors.map(colour), "PieceType": format!("persona_{}", t.piece_type)})
    }).collect();
    let mut key: Vec<String> = pieces.iter().map(|p| p["PieceId"].as_str().unwrap_or_default().to_owned()).collect();
    key.sort();
    key.push(colour(w.skin_color));
    key.push(serde_json::to_string(&tints).ok()?);
    let look = format!("{:016x}", fnv(key.join("|").as_bytes()));
    let skin = Skin {
        animated_image_data: w.animations.iter().map(|a| json!({
            "AnimationExpression": a.expression_type,
            "Frames": a.animation_frames as u32,
            "Image": STANDARD.encode(&a.skin_image.data),
            "ImageHeight": a.skin_image.height,
            "ImageWidth": a.skin_image.width,
            "Type": a.animation_type,
        })).collect(),
        arm_size: match w.arm_size { SkinArmSize::Slim => "slim", _ => "wide" }.into(),
        cape_data: String::new(),
        cape_id: String::new(),
        cape_image_height: 0,
        cape_image_width: 0,
        cape_on_classic_skin: false,
        override_skin: false,
        persona_pieces: pieces,
        persona_skin: true,
        piece_tint_colours: tints,
        premium_skin: false,
        skin_animation_data: String::new(),
        skin_colour: colour(w.skin_color),
        skin_data: STANDARD.encode(&w.skin_data.data),
        skin_geometry: STANDARD.encode(styled(&w.geometry_data)),
        skin_geometry_version: STANDARD.encode(&w.geometry_data_version),
        skin_id: w.skin_id.clone(),
        skin_image_height: w.skin_data.height as u32,
        skin_image_width: w.skin_data.width as u32,
        skin_resource_patch: STANDARD.encode(styled(&w.skin_resource_pack)),
        trusted_skin: true,
    };
    Some((look, skin))
}

/// Why a skin was rejected, without anything identifying.
fn shape(w: &WireSkin) -> String {
    let defaults = w.personal_pieces.iter().filter(|p| p.is_default_piece).count();
    format!(
        "persona={} premium={} cape={} anims={} geometry={}B {}x{} pieces={} default={} id_shape={}",
        w.persona, w.premium, !w.cape_id.is_empty(), w.animations.len(), w.geometry_data.len(),
        w.skin_data.width, w.skin_data.height, w.personal_pieces.len(), defaults,
        w.skin_id.chars().map(|c| if c.is_ascii_hexdigit() { 'x' } else { c }).collect::<String>(),
    )
}

fn snake(camel: &str) -> String {
    camel.chars().enumerate().fold(String::new(), |mut s, (i, c)| {
        if c.is_uppercase() && i > 0 {
            s.push('_');
        }
        s.push(c.to_ascii_lowercase());
        s
    })
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3))
}
