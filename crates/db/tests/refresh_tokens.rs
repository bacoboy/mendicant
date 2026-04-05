mod common;

use db::error::DbError;
use db::refresh_tokens::RefreshTokenRepository;
use domain::token::RefreshToken;
use domain::user::{Role, UserId};

fn token() -> RefreshToken {
    RefreshToken::new(UserId::new(), Role::Free, 9999999999)
}

#[tokio::test]
async fn put_and_get() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db);
    let t = token();

    repo.put(&t).await.unwrap();
    let fetched = repo.get(&t.jti).await.unwrap();

    assert_eq!(fetched.jti, t.jti);
    assert_eq!(fetched.user_id, t.user_id);
    assert_eq!(fetched.role, Role::Free);
    assert!(!fetched.revoked);
}

#[tokio::test]
async fn get_missing_returns_not_found() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db);

    let err = repo.get("no-such-jti").await.unwrap_err();
    assert!(matches!(err, DbError::NotFound));
}

#[tokio::test]
async fn duplicate_put_fails() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db);
    let t = token();

    repo.put(&t).await.unwrap();
    let err = repo.put(&t).await.unwrap_err();
    assert!(matches!(err, DbError::ConditionalCheckFailed));
}

#[tokio::test]
async fn revoke_marks_token_revoked() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db);
    let t = token();

    repo.put(&t).await.unwrap();
    repo.revoke(&t.jti).await.unwrap();

    let fetched = repo.get(&t.jti).await.unwrap();
    assert!(fetched.revoked);
}

#[tokio::test]
async fn revoke_missing_token_fails() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db);

    let err = repo.revoke("ghost-jti").await.unwrap_err();
    assert!(matches!(err, DbError::ConditionalCheckFailed));
}
