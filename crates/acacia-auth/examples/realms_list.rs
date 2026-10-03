//! Lists the Realms a cached account can see; `--invite <code|realms.gg link>` accepts an invite
//! first, `--join <id>` also prints that realm's join target.
//!
//! `cargo run -p acacia-auth --example realms_list -- [account] [--invite <code>] [--join <id>]`

use std::sync::Arc;

use acacia_auth::{Account, AuthClient, AuthConfig, FileTokenCache};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let join = args.iter().position(|a| a == "--join").and_then(|i| args.get(i + 1)).map(|id| id.parse::<i64>());
    let account_id = args.first().filter(|a| !a.starts_with("--")).cloned().unwrap_or("default".into());
    let client = Arc::new(AuthClient::new(AuthConfig::default())?);
    let account = Account::new(client, Arc::new(FileTokenCache::new(".tokens")?), account_id);

    if let Some(code) = args.iter().position(|a| a == "--invite").and_then(|i| args.get(i + 1)) {
        let r = account.realm_by_invite(code).await?;
        println!("invite: id={} name={:?} owner={:?} state={}", r.id, r.name, r.owner, r.state);
        let r = account.accept_realm_invite(code).await?;
        println!("accepted: id={} name={:?}", r.id, r.name);
    }
    let realms = account.realms().await?;
    println!("{} realm(s)", realms.len());
    for r in &realms {
        println!("id={} name={:?} owner={:?} state={} expired={}", r.id, r.name, r.owner, r.state, r.expired);
    }
    if let Some(id) = join {
        // TODO: measured QoS latencies; vanilla never sends an empty list.
        let target = account.join_realm(id?, &[]).await?;
        println!("join: protocol={:?} address={} region={:?}", target.protocol, target.address, target.region);
    }
    Ok(())
}
