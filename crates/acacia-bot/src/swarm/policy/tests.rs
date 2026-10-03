use std::sync::Arc;
use std::time::Duration;

use acacia_client::DisconnectReason;

use super::{Decision, Ended, Next, Policy, Retry};
use crate::swarm::BotId;

fn kick() -> DisconnectReason {
    DisconnectReason::Kicked { reason: "kicked".into(), message: "bye".into() }
}

fn session(policy: &Policy, retry: &mut Retry, reason: &DisconnectReason, secs: u64) -> Next {
    // noise 25 = exactly the base delay
    policy.next(&BotId::from("a"), retry, Ended::Session { reason, uptime: Duration::from_secs(secs) }, 25)
}

#[test]
fn backoff_doubles_to_the_cap_and_resets_after_a_healthy_session() {
    let policy = Policy::default();
    let mut retry = Retry::default();
    let io = DisconnectReason::Io("reset".into());
    let delays: Vec<_> = (0..8).map(|_| session(&policy, &mut retry, &io, 1)).collect();
    let secs = [1, 2, 4, 8, 16, 32, 60, 60].map(|s| Next::After(Duration::from_secs(s)));
    assert_eq!(delays, secs);
    assert_eq!(session(&policy, &mut retry, &io, 120), Next::After(Duration::from_secs(1)));
}

#[test]
fn jitter_stays_within_a_quarter() {
    let policy = Policy::default();
    let io = DisconnectReason::Io("reset".into());
    for noise in [0, 50, 1234] {
        let ended = Ended::Session { reason: &io, uptime: Duration::ZERO };
        let next = policy.next(&BotId::from("a"), &mut Retry::default(), ended, noise);
        let Next::After(d) = next else { panic!("{next:?}") };
        assert!(d >= Duration::from_millis(750) && d <= Duration::from_millis(1250), "{d:?}");
    }
}

#[test]
fn quick_kicks_give_up_but_a_late_kick_resets_the_count() {
    let policy = Policy::default();
    let mut retry = Retry::default();
    assert!(matches!(session(&policy, &mut retry, &kick(), 5), Next::After(_)));
    assert!(matches!(session(&policy, &mut retry, &kick(), 5), Next::After(_)));
    assert!(matches!(session(&policy, &mut retry, &kick(), 600), Next::After(_)));
    assert!(matches!(session(&policy, &mut retry, &kick(), 5), Next::After(_)));
    assert!(matches!(session(&policy, &mut retry, &kick(), 5), Next::After(_)));
    assert!(matches!(session(&policy, &mut retry, &kick(), 5), Next::Stop(e) if e.contains("bye")));
}

#[test]
fn transfer_is_followed_with_ipv6_brackets() {
    let policy = Policy::default();
    let mut retry = Retry::default();
    let v4 = DisconnectReason::Transfer { address: "play.example.net".into(), port: 19132 };
    let v6 = DisconnectReason::Transfer { address: "::1".into(), port: 19133 };
    assert_eq!(session(&policy, &mut retry, &v4, 9), Next::Follow("play.example.net:19132".into()));
    assert_eq!(session(&policy, &mut retry, &v6, 9), Next::Follow("[::1]:19133".into()));
}

#[test]
fn hook_overrides_the_default() {
    let policy = Policy { on_disconnect: Some(Arc::new(|_, _, _| Decision::Stop { error: "no".into() })), ..Policy::default() };
    let next = session(&policy, &mut Retry::default(), &DisconnectReason::Io("x".into()), 1);
    assert_eq!(next, Next::Stop("no".into()));
}

#[test]
fn invalid_spec_stops() {
    let policy = Policy::default();
    let next = policy.next(&BotId::from("a"), &mut Retry::default(), Ended::Invalid("realm needs online".into()), 0);
    assert_eq!(next, Next::Stop("realm needs online".into()));
}
