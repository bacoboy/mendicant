mod common;

use db::challenges::ChallengeRepository;
use db::error::DbError;
use domain::challenge::Challenge;

#[tokio::test]
async fn put_and_get() {
    let env = common::TestEnv::new().await;
    let repo = ChallengeRepository::new(env.db);
    let challenge = Challenge::new_registration("{\"opaque\":true}".into(), 9999999999);

    repo.put(&challenge).await.unwrap();
    let fetched = repo.get(&challenge.id).await.unwrap();

    assert_eq!(fetched.id, challenge.id);
    assert_eq!(fetched.state_json, challenge.state_json);
    assert_eq!(fetched.expires_at, challenge.expires_at);
    assert!(fetched.user_id.is_none());
}

#[tokio::test]
async fn get_missing_returns_not_found() {
    let env = common::TestEnv::new().await;
    let repo = ChallengeRepository::new(env.db);

    let err = repo.get("nonexistent-id").await.unwrap_err();
    assert!(matches!(err, DbError::NotFound));
}

#[tokio::test]
async fn take_consumes_challenge_preventing_replay() {
    let env = common::TestEnv::new().await;
    let repo = ChallengeRepository::new(env.db);
    let challenge = Challenge::new_registration("{}".into(), 9999999999);

    repo.put(&challenge).await.unwrap();

    // First take succeeds and returns the challenge.
    let taken = repo.take(&challenge.id).await.unwrap();
    assert_eq!(taken.id, challenge.id);

    // Second take fails — challenge was deleted.
    let err = repo.take(&challenge.id).await.unwrap_err();
    assert!(matches!(err, DbError::NotFound));
}

#[tokio::test]
async fn authentication_challenge_preserves_user_id() {
    let env = common::TestEnv::new().await;
    let repo = ChallengeRepository::new(env.db);
    let uid = "user-abc-123".to_string();
    let challenge = Challenge::new_authentication(uid.clone(), "{}".into(), 9999999999);

    repo.put(&challenge).await.unwrap();
    let fetched = repo.get(&challenge.id).await.unwrap();

    assert_eq!(fetched.user_id.as_deref(), Some("user-abc-123"));
}
