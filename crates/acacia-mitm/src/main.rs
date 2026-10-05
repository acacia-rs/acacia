//! Recording proxy for ground-truth captures of the vanilla client: the game joins this proxy,
//! the proxy joins the server as the same player, and every packet on the game's side is logged
//! (record.rs).
//!
//! `cargo run -p acacia-mitm -- [--transport raknet|nethernet] [--listen 0.0.0.0:19180] [--server <addr>] [--out .testserver/mitm] [--online <account>] [--pack-cdn <pack.zip>]`
//!
//! RakNet (default) is offline by default, for the local test BDS on 19140. `--online <account>`
//! signs in with that account (`.tokens`, device code on first use) for real servers; sign the game
//! into the same account so the client data matches. NetherNet direct connect (nethernet/) targets
//! the NetherNet BDS on 19160 and always needs `--online`. Login is logged as a structural summary
//! only (login.rs): no tokens or signatures.

mod cdn;

use std::net::SocketAddr;
use std::sync::Arc;

use acacia_auth::{Account, AuthClient, AuthConfig, FileTokenCache};
use acacia_mitm::{Proxy, Recorder};

struct Args {
    nethernet: bool,
    listen: SocketAddr,
    /// `host:port`, resolved once at start.
    server: String,
    out: String,
    online: Option<String>,
    /// A pack zip to serve as every pack's `cdn_url` (cdn.rs).
    pack_cdn: Option<String>,
}

fn args() -> Result<Args, String> {
    let mut args = Args {
        nethernet: false,
        listen: "0.0.0.0:19180".parse().unwrap(),
        server: String::new(),
        out: ".testserver/mitm".into(),
        online: None,
        pack_cdn: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--transport" => args.nethernet = match value.as_str() {
                "raknet" => false,
                "nethernet" => true,
                _ => return Err(format!("--transport: {value} is not raknet or nethernet")),
            },
            "--listen" => args.listen = value.parse().map_err(|e| format!("--listen: {e}"))?,
            "--server" => args.server = value,
            "--out" => args.out = value,
            "--online" => args.online = Some(value),
            "--pack-cdn" => args.pack_cdn = Some(value),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if args.server.is_empty() {
        args.server = if args.nethernet { "127.0.0.1:19160" } else { "127.0.0.1:19140" }.into();
    }
    if args.nethernet && args.online.is_none() {
        return Err("--transport nethernet needs --online <account>: BDS refuses NetherNet offers without a MultiplayerToken".into());
    }
    Ok(args)
}

async fn sign_in(id: &str) -> Result<Account, Box<dyn std::error::Error>> {
    let account = Account::new(Arc::new(AuthClient::new(AuthConfig::default())?), Arc::new(FileTokenCache::new(".tokens")?), id);
    if !account.is_signed_in().await? {
        account.sign_in(|p| println!("sign in at {} with code {}", p.verification_uri, p.user_code)).await?;
    }
    Ok(account)
}

/// Debug-build str0m connections overflow Windows' 1 MiB main-thread stack, so the runtime gets its own.
const STACK: usize = 64 << 20;

fn main() -> Result<(), String> {
    let runtime = std::thread::Builder::new().stack_size(STACK).spawn(|| {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
        rt.block_on(run()).map_err(|e| e.to_string())
    });
    runtime.map_err(|e| e.to_string())?.join().map_err(|_| "proxy thread panicked".to_owned())?
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = args()?;
    let stamp = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    let name = stamp.format(time::macros::format_description!("[year][month][day]-[hour][minute][second]"))?;
    let target = tokio::net::lookup_host(&args.server).await?.next().ok_or(format!("{}: no address", args.server))?;
    let mut proxy = Proxy::new(target).listen(args.listen);
    if let Some(id) = &args.online {
        proxy = proxy.online(sign_in(id).await?);
    }
    let rec = Recorder::create(args.out.as_ref(), &name)?;
    println!("join {} from the game; recording to {}", args.listen, rec.path().display());
    if args.nethernet {
        proxy = proxy.nethernet(acacia_mitm::host_key(args.out.as_ref())?);
    }
    if let Some(zip) = &args.pack_cdn {
        let url = cdn::start(zip.as_ref(), args.listen.port() + 1, &rec.path().with_extension("cdn.log"))?;
        proxy = proxy.intercept(move |_| cdn::PointPacks(url.clone()));
    }
    // MITM_TRACE=1 logs every datagram on the game's side (first byte and size), to debug joins and
    // compare send pacing.
    let proxy = proxy.record(rec).trace_datagrams(std::env::var_os("MITM_TRACE").is_some());
    Ok(proxy.bind().await?.run().await?)
}
