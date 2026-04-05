mod common;

use db::credentials::CredentialRepository;
use db::users::UserRepository;
use domain::credential::{Credential, CredentialId};
use domain::user::{User, UserId};
use time::OffsetDateTime;
use uuid::Uuid;

fn user() -> User {
    User::new("test@example.com".into(), "Test".into())
}

fn credential(user_id: UserId) -> Credential {
    let now = OffsetDateTime::now_utc();
    Credential {
        id: CredentialId(Uuid::new_v4().to_string()),
        user_id,
        public_key: b"fake-passkey-bytes".to_vec(),
        sign_count: 0,
        aaguid: Uuid::nil(),
        nickname: Some("YubiKey".into()),
        created_at: now,
        last_used_at: now,
    }
}

#[tokio::test]
async fn put_and_list_for_user() {
    let env = common::TestEnv::new().await;
    let user_repo = UserRepository::new(env.db.clone());
    let cred_repo = CredentialRepository::new(env.db);

    let u = user();
    user_repo.put(&u).await.unwrap();

    let c = credential(u.id.clone());
    cred_repo.put(&c).await.unwrap();

    let list = cred_repo.list_for_user(&u.id).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, c.id);
    assert_eq!(list[0].public_key, b"fake-passkey-bytes");
    assert_eq!(list[0].nickname.as_deref(), Some("YubiKey"));
}

#[tokio::test]
async fn list_for_user_returns_empty_when_none() {
    let env = common::TestEnv::new().await;
    let repo = CredentialRepository::new(env.db);

    let list = repo.list_for_user(&UserId::new()).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn update_sign_count() {
    let env = common::TestEnv::new().await;
    let user_repo = UserRepository::new(env.db.clone());
    let cred_repo = CredentialRepository::new(env.db);

    let u = user();
    user_repo.put(&u).await.unwrap();

    let c = credential(u.id.clone());
    cred_repo.put(&c).await.unwrap();

    cred_repo.update_sign_count(&u.id, &c.id, 5).await.unwrap();
    let list = cred_repo.list_for_user(&u.id).await.unwrap();
    assert_eq!(list[0].sign_count, 5);
}

#[tokio::test]
async fn sign_count_regression_is_tolerated() {
    // Counter going backwards is logged but does not return an error —
    // this tolerates eventual consistency lag in Global Tables.
    let env = common::TestEnv::new().await;
    let user_repo = UserRepository::new(env.db.clone());
    let cred_repo = CredentialRepository::new(env.db);

    let u = user();
    user_repo.put(&u).await.unwrap();

    let c = credential(u.id.clone());
    cred_repo.put(&c).await.unwrap();

    cred_repo.update_sign_count(&u.id, &c.id, 10).await.unwrap();
    // Sending a lower count should not fail.
    cred_repo.update_sign_count(&u.id, &c.id, 2).await.unwrap();

    // The stored count should remain 10 (condition_expression prevented the rollback).
    let list = cred_repo.list_for_user(&u.id).await.unwrap();
    assert_eq!(list[0].sign_count, 10);
}

#[tokio::test]
async fn delete_removes_credential() {
    let env = common::TestEnv::new().await;
    let user_repo = UserRepository::new(env.db.clone());
    let cred_repo = CredentialRepository::new(env.db);

    let u = user();
    user_repo.put(&u).await.unwrap();

    let c = credential(u.id.clone());
    cred_repo.put(&c).await.unwrap();
    cred_repo.delete(&u.id, &c.id).await.unwrap();

    let list = cred_repo.list_for_user(&u.id).await.unwrap();
    assert!(list.is_empty());
}
