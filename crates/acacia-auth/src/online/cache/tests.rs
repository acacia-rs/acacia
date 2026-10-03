use super::*;
use crate::online::live::MsaToken;
use crate::online::xbox::XboxToken;

fn sample() -> CachedTokens {
    let mut t = CachedTokens::new(Some(MsaToken {
        access_token: "a".into(),
        refresh_token: Some("r".into()),
        expires_at: 42,
        user_id: None,
    }));
    t.xsts.insert("rp".into(), XboxToken {
        token: "x".into(),
        not_after: 1,
        user_hash: Some("uhs".into()),
        xuid: None,
        gamertag: None,
    });
    t
}

fn temp_dir() -> PathBuf {
    std::env::temp_dir().join(format!("acacia-auth-test-{}", uuid::Uuid::new_v4()))
}

async fn compare_and_swap(cache: &dyn TokenCache) {
    let tokens = sample();
    assert!(cache.load("a").await.unwrap().is_none());
    assert_eq!(cache.store("a", &tokens, Some(1)).await.unwrap(), Stored::Conflict);
    assert_eq!(cache.store("a", &tokens, None).await.unwrap(), Stored::Written { version: 1 });
    assert_eq!(cache.store("a", &tokens, None).await.unwrap(), Stored::Conflict);
    let mut newer = tokens.clone();
    newer.device_id = "other".into();
    assert_eq!(cache.store("a", &newer, Some(1)).await.unwrap(), Stored::Written { version: 2 });
    assert_eq!(cache.store("a", &tokens, Some(1)).await.unwrap(), Stored::Conflict, "stale writer must lose");
    assert_eq!(cache.load("a").await.unwrap(), Some(Versioned { tokens: newer, version: 2 }));
}

#[tokio::test]
async fn memory_compare_and_swap() {
    compare_and_swap(&MemoryTokenCache::new()).await;
}

#[tokio::test]
async fn file_compare_and_swap() {
    let dir = temp_dir();
    compare_and_swap(&FileTokenCache::new(&dir).unwrap()).await;
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn file_round_trip_keeps_device_key() {
    let dir = temp_dir();
    let cache = FileTokenCache::new(&dir).unwrap();
    let mut tokens = sample();
    let key = tokens.xbox_key();
    cache.store("user@example.com", &tokens, None).await.unwrap();
    let mut loaded = cache.load("user@example.com").await.unwrap().unwrap().tokens;
    assert_eq!(loaded, tokens);
    assert_eq!(loaded.xbox_key().to_bytes(), key.to_bytes());
    assert!(cache.path_for("a/b").ends_with("a_b.json"));
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn file_reads_unversioned_cache_as_version_zero() {
    let dir = temp_dir();
    let cache = FileTokenCache::new(&dir).unwrap();
    let tokens = sample();
    std::fs::write(cache.path_for("old"), serde_json::to_vec(&tokens).unwrap()).unwrap();
    assert_eq!(cache.load("old").await.unwrap(), Some(Versioned { tokens: tokens.clone(), version: 0 }));
    assert_eq!(cache.store("old", &tokens, Some(0)).await.unwrap(), Stored::Written { version: 1 });
    std::fs::remove_dir_all(dir).unwrap();
}
