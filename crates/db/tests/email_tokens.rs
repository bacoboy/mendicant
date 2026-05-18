mod common;

use db::email_tokens::EmailTokenRepository;
use db::error::DbError;
use domain::email_token::EmailToken;

fn token() -> EmailToken {
    EmailToken::new("test@example.com".into(), 9999999999)
}

#[tokio::test]
async fn put_and_get() {
    let env = common::TestEnv::new().await;
    let repo = EmailTokenRepository::new(env.db.clone());
    let t = token();

    repo.put(&t).await.unwrap();
    let fetched = repo.get(&t.id).await.unwrap();

    assert_eq!(fetched.id, t.id);
    assert_eq!(fetched.email, "test@example.com");
    assert_eq!(fetched.expires_at, 9999999999);
}

#[tokio::test]
async fn get_missing_returns_not_found() {
    let env = common::TestEnv::new().await;
    let repo = EmailTokenRepository::new(env.db.clone());

    let err = repo.get("no-such-id").await.unwrap_err();
    assert!(matches!(err, DbError::NotFound));
}

#[tokio::test]
async fn take_consumes_token_preventing_replay() {
    let env = common::TestEnv::new().await;
    let repo = EmailTokenRepository::new(env.db.clone());
    let t = token();

    repo.put(&t).await.unwrap();

    let taken = repo.take(&t.id).await.unwrap();
    assert_eq!(taken.id, t.id);
    assert_eq!(taken.email, "test@example.com");

    let err = repo.take(&t.id).await.unwrap_err();
    assert!(matches!(err, DbError::NotFound));
}

#[tokio::test]
async fn duplicate_put_fails() {
    let env = common::TestEnv::new().await;
    let repo = EmailTokenRepository::new(env.db.clone());
    let t = token();

    repo.put(&t).await.unwrap();
    let err = repo.put(&t).await.unwrap_err();
    assert!(matches!(err, DbError::ConditionalCheckFailed));
}
