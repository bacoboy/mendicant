mod common;

use db::error::DbError;
use db::refresh_tokens::RefreshTokenRepository;
use domain::token::RefreshToken;
use domain::user::UserId;

fn token() -> RefreshToken {
    RefreshToken::new(UserId::new(), 9999999999, None)
}

#[tokio::test]
async fn put_and_get() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db.clone());
    let t = token();

    repo.put(&t).await.unwrap();
    let fetched = repo.get(&t.jti).await.unwrap();

    assert_eq!(fetched.jti, t.jti);
    assert_eq!(fetched.user_id, t.user_id);
    assert!(!fetched.revoked);
}

#[tokio::test]
async fn get_missing_returns_not_found() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db.clone());

    let err = repo.get("no-such-jti").await.unwrap_err();
    assert!(matches!(err, DbError::NotFound));
}

#[tokio::test]
async fn duplicate_put_fails() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db.clone());
    let t = token();

    repo.put(&t).await.unwrap();
    let err = repo.put(&t).await.unwrap_err();
    assert!(matches!(err, DbError::ConditionalCheckFailed));
}

#[tokio::test]
async fn revoke_marks_token_revoked() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db.clone());
    let t = token();

    repo.put(&t).await.unwrap();
    repo.revoke(&t.jti).await.unwrap();

    let fetched = repo.get(&t.jti).await.unwrap();
    assert!(fetched.revoked);
}

#[tokio::test]
async fn revoke_missing_token_fails() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db.clone());

    let err = repo.revoke("ghost-jti").await.unwrap_err();
    assert!(matches!(err, DbError::ConditionalCheckFailed));
}

#[tokio::test]
async fn list_for_user_returns_tokens() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db.clone());
    let user_id = UserId::new();
    let other_id = UserId::new();

    let t1 = RefreshToken::new(user_id.clone(), 9999999999, None);
    let t2 = RefreshToken::new(user_id.clone(), 9999999999, None);
    let t3 = RefreshToken::new(other_id.clone(), 9999999999, None);
    repo.put(&t1).await.unwrap();
    repo.put(&t2).await.unwrap();
    repo.put(&t3).await.unwrap();

    let tokens = repo.list_for_user(&user_id).await.unwrap();
    assert_eq!(tokens.len(), 2);
    assert!(tokens.iter().all(|t| t.user_id == user_id));
}

#[tokio::test]
async fn list_for_user_excludes_revoked() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db.clone());
    let user_id = UserId::new();

    let active = RefreshToken::new(user_id.clone(), 9999999999, None);
    let revoked = RefreshToken::new(user_id.clone(), 9999999999, None);
    repo.put(&active).await.unwrap();
    repo.put(&revoked).await.unwrap();
    repo.revoke(&revoked.jti).await.unwrap();

    let tokens = repo.list_for_user(&user_id).await.unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].jti, active.jti);
}

#[tokio::test]
async fn revoke_all_for_user() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db.clone());
    let user_id = UserId::new();

    let t1 = RefreshToken::new(user_id.clone(), 9999999999, None);
    let t2 = RefreshToken::new(user_id.clone(), 9999999999, None);
    repo.put(&t1).await.unwrap();
    repo.put(&t2).await.unwrap();

    repo.revoke_all_for_user(&user_id).await.unwrap();

    let tokens = repo.list_for_user(&user_id).await.unwrap();
    assert!(tokens.is_empty());
}

#[tokio::test]
async fn client_hint_round_trips() {
    let env = common::TestEnv::new().await;
    let repo = RefreshTokenRepository::new(env.db.clone());
    let t = RefreshToken::new(UserId::new(), 9999999999, Some("Safari · macOS".into()));

    repo.put(&t).await.unwrap();
    let fetched = repo.get(&t.jti).await.unwrap();
    assert_eq!(fetched.client_hint.as_deref(), Some("Safari · macOS"));
}
