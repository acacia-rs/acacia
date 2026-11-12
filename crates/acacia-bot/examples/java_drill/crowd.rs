//! `crowd`: a second player (`Peer`) and the driller walk into each other, then stand overlapped and push apart.
//! Both are acacia bots, so Grim judges each side of the push; the peer's setbacks fail the drill too.

use crate::drills::{check, start, Outcome};
use crate::{Drill, FLOOR};

const PEER: &str = "Peer";

/// Both bots tick together: a bot only steps its physics while awaited.
async fn both(d: &mut Drill, peer: &mut Drill, n: u32, forward: f32) -> Result<(), String> {
    let (a, b) = tokio::join!(d.walk(n, forward, false, false), peer.walk(n, forward, false, false));
    a.and(b)
}

pub async fn run(d: &mut Drill, x: i32) -> Outcome {
    let surface = FLOOR + 1;
    let base = start(d, x, &[]).await?;
    let mut peer = Drill::connect(&d.server.clone(), PEER).await?;
    for c in [format!("gamemode survival {PEER}"), format!("effect clear {PEER}"), format!("tp {PEER} {}.5 {surface} 0.5 90 0", x + 4)] {
        d.bot.command(&c);
        both(d, &mut peer, 3, 0.0).await?;
    }
    both(d, &mut peer, 40, 0.0).await?;
    let peer_base = peer.setbacks();
    both(d, &mut peer, 30, 1.0).await?;
    let walked = peer.setbacks() - peer_base;
    let met = d.feet()[0];
    // Overlapping by 0.3 in x: each pushes the other until clear. The teleport itself counts as one setback.
    d.bot.command(&format!("tp {PEER} {:.3} {surface} {:.3} 90 0", met + 0.3, d.feet()[2]));
    both(d, &mut peer, 5, 0.0).await?;
    let peer_base = peer.setbacks();
    both(d, &mut peer, 35, 0.0).await?;
    let shoved = met - d.feet()[0];
    both(d, &mut peer, 20, 1.0).await?;
    let peer_setbacks = walked + peer.setbacks() - peer_base;
    peer.bot.disconnect().await;
    Ok((base, check(shoved > 0.02 && peer_setbacks == 0, || format!("pushed {shoved:.3}, peer setbacks {peer_setbacks}"))))
}
