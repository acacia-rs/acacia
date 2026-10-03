//! Device code login with a file cache in `./.tokens`.
//!
//! `cargo run -p acacia-auth --example device_login -- [account] [--switch] [--start-only]`
//! `--start-only` requests a device code and exits (checks endpoint + client ID, no login).

use std::sync::Arc;

use acacia_auth::{Account, AuthClient, AuthConfig, FileTokenCache, Title};
use p384::ecdsa::SigningKey;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let account_id = args.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or("default".into());
    let title = if flag("--switch") { Title::NintendoSwitch } else { Title::Android };

    let client = Arc::new(AuthClient::new(AuthConfig { title, ..AuthConfig::default() })?);
    if flag("--start-only") {
        let p = client.start_device_code().await?;
        println!(
            "device code OK: go to {} and enter {} (expires in {}s, poll every {}s)",
            p.verification_uri, p.user_code, p.expires_in, p.interval
        );
        return Ok(());
    }

    let account = Account::new(client, Arc::new(FileTokenCache::new(".tokens")?), account_id);
    if !account.is_signed_in() {
        account
            .sign_in(|p| println!("Sign in at {} with code {}", p.verification_uri, p.user_code))
            .await?;
    }

    let client_key = SigningKey::random(&mut rand_core::OsRng);
    match account.credentials(&client_key).await {
        Ok(c) => {
            println!("xuid:              {}", c.xuid);
            println!("display name:      {}", c.display_name);
            println!("identity:          {}", c.identity);
            println!("chain length:      {}", c.chain.len());
            println!("multiplayer token: {}", c.multiplayer_token.is_some());
        }
        Err(e) if e.requires_user_action() => println!("account needs attention: {e}"),
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
