//! World viewer: joins a server with an Acacia bot and renders its terrain with a free camera.
//! `cargo run --release -p acacia-viewer -- <server> <name|@account> [chunk radius]`
//! - `@account` signs in online with tokens cached in ./.tokens (see acacia-auth's device_login).
//! - Textures: run tools/fetch-vanilla-pack.sh first, or point ACACIA_ASSETS at a pack.
//! - Controls: click to grab the mouse, WASD/Space/Shift to fly, Ctrl faster, wheel changes speed,
//!   F jumps to the bot, V toggles vsync, C toggles cave culling, Esc releases the mouse.

mod app;
#[cfg(feature = "profile")]
mod heap;
mod input;
mod net;

#[cfg(feature = "profile")]
#[global_allocator]
static ALLOC: heap::Tracking = heap::Tracking;

use winit::event_loop::EventLoop;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let mut args = std::env::args().skip(1);
    let server = args.next().unwrap_or_else(|| "127.0.0.1:19140".into());
    let name = args.next().unwrap_or_else(|| "Viewer".into());
    let radius: i32 = args.next().map_or(Ok(8), |s| s.parse())?;

    let pack = acacia_render::assets::Pack::load(&acacia_render::assets::Pack::default_dir())?;
    let net = net::spawn(net::Options { server, name, radius }, pack);
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app::App::new(net, radius))?;
    Ok(())
}
