//! World viewer: joins a server with an Acacia bot and renders its terrain with a free camera.
//! `cargo run --release -p acacia-viewer -- <server> <name|@account> [chunk radius] [--look bedrock|java]`
//! - `@account` signs in online with tokens cached in ./.tokens (see acacia-auth's device_login).
//! - Textures: run tools/fetch-vanilla-pack.sh first, or point ACACIA_ASSETS at a pack. A look
//!   baked by tools/lookbake (assets/looks/<name>) is used in place of baking at start (looks.rs).
//! - Controls: click to grab the mouse, WASD/Space/Shift to fly, Ctrl faster, wheel changes speed,
//!   F jumps to the bot, V toggles vsync, C toggles cave culling, L switches the look, Esc releases
//!   the mouse. V, C and L are remembered (settings.rs).

mod app;
mod block_data;
mod entities;
#[cfg(feature = "profile")]
mod heap;
mod input;
mod looks;
mod net;
mod overlay;
mod settings;
mod shot;
mod smooth;

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

    let pack = acacia_render::assets::Pack::load(&acacia_render::assets::Pack::default_dir())?;
    let sky = acacia_render::sky::SkyTextures::load(&pack);
    let looks = looks::Looks::load(&pack);
    let net = net::spawn(net::Options { server, name, radius }, pack);
    let event_loop = EventLoop::new()?;
    let mut app = app::App::new(net, radius, sky, settings, looks);
    event_loop.run_app(&mut app)?;
    // Unattended screenshots exit non-zero when the session ends first (kicked, e.g. ServerIdConflict).
    match app.failed {
        Some(reason) => Err(format!("no screenshot: {reason}").into()),
        None => Ok(()),
    }
}
