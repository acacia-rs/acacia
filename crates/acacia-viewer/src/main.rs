//! World viewer: joins a server with an Acacia bot and renders its terrain with a free camera.
//! `cargo run --release -p acacia-viewer -- <server> <name|@account> [chunk radius] [--look bedrock|java]`
//! - `@account` signs in online with tokens cached in ./.tokens (see acacia-auth's device_login).
//! - Assets: a look baked by tools/lookbake (assets/looks/<name>), or else the resource pack from
//!   tools/fetch-vanilla-pack.sh (ACACIA_ASSETS), baked at start (looks.rs).
//! - Controls: click to grab the mouse, Esc releases it, F6 switches between playing and a free
//!   camera, L switches the look (remembered, settings.rs).
//!   - Playing: WASD, Space jumps, Shift sneaks, Ctrl sprints; Space twice toggles flight where allowed
//!     (then Space rises, Shift descends); left button mines or hits, right
//!     button uses or places; 1-9 and the wheel pick the hotbar slot; T or / opens the chat (Enter
//!     sends, Up recalls); F5 changes the perspective; E opens the inventory (click, right-click,
//!     shift-click; outside drops); Esc opens the pause menu and its options.
//!     Right-click a boat, minecart, or saddled horse or pig to ride it: WASD steers (Space held and
//!     released jumps a horse), Shift gets off.
//!   - Either: F3 shows the debug screen.
//!   - Free camera: WASD/Space/Shift fly, Ctrl faster, wheel changes speed, F jumps to the bot,
//!     V toggles vsync, C toggles cave culling (both remembered).

mod app;
mod armor;
mod audio;
mod block_data;
mod bob;
mod control;
mod debug_lines;
mod entities;
mod forms;
#[cfg(feature = "profile")]
mod heap;
mod facts;
mod input;
mod item_look;
mod keyscript;
mod looks;
mod me;
mod net;
mod overlay;
mod particles;
mod pick;
mod player;
mod ride;
mod scoreboard;
mod settings;
mod shot;
mod sign_text;
mod smooth;
mod stations;
mod ui;

#[cfg(feature = "profile")]
#[global_allocator]
static ALLOC: heap::Tracking = heap::Tracking;

use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut settings = settings::Settings::load();
    if let Some(i) = args.iter().position(|a| a == "--look") {
        settings.look = args.get(i + 1).ok_or("--look needs bedrock or java")?.parse()?;
        args.drain(i..=i + 1);
    }
    if std::env::var_os("ACACIA_NO_CULL").is_some() {
        settings.cave_culling = false;
    }
    let mut args = args.into_iter();
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "Viewer".into());
    let radius: i32 = args.next().map_or(Ok(8), |s| s.parse())?;

    let looks = looks::Looks::load()?;
    // Entities, biome colours and the sky follow the look chosen at start; L changes blocks only.
    let files = looks.get(settings.look).files().to_owned();
    let sky = acacia_render::sky::SkyTextures::load(&files);
    let net = net::spawn(net::Options { server, name, radius }, files);
    let event_loop = EventLoop::new()?;
    let mut app = app::App::new(net, radius, sky, settings, looks);
    event_loop.run_app(&mut app)?;
    // Unattended screenshots exit non-zero when the session ends first (kicked, e.g. ServerIdConflict).
    match app.failed {
        Some(reason) => Err(format!("no screenshot: {reason}").into()),
        None => Ok(()),
    }
}
