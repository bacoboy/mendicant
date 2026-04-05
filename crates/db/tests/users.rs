mod common;

use db::error::DbError;
use db::users::UserRepository;
use domain::user::{Role, User, UserId, UserStatus};

// ── helpers ───────────────────────────────────────────────────────────────────

fn alice() -> User {
    User::new("alice@example.com".into(), "Alice".into())
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn put_and_get_by_id() {
    let env = common::TestEnv::new().await;
    let repo = UserRepository::new(env.db);
    let user = alice();

    repo.put(&user).await.unwrap();
    let fetched = repo.get(&user.id).await.unwrap();

    assert_eq!(fetched.id, user.id);
    assert_eq!(fetched.email, user.email);
    assert_eq!(fetched.display_name, user.display_name);
    assert_eq!(fetched.role, Role::Free);
    assert_eq!(fetched.status, UserStatus::PendingVerification);
}

#[tokio::test]
async fn get_missing_user_returns_not_found() {
    let env = common::TestEnv::new().await;
    let repo = UserRepository::new(env.db);

    let err = repo.get(&UserId::new()).await.unwrap_err();
    assert!(matches!(err, DbError::NotFound));
}

#[tokio::test]
async fn get_by_email() {
    let env = common::TestEnv::new().await;
    let repo = UserRepository::new(env.db);
    let user = alice();

    repo.put(&user).await.unwrap();
    let fetched = repo.get_by_email(&user.email).await.unwrap();

    assert_eq!(fetched.id, user.id);
    assert_eq!(fetched.email, "alice@example.com");
}

#[tokio::test]
async fn get_by_email_missing_returns_not_found() {
    let env = common::TestEnv::new().await;
    let repo = UserRepository::new(env.db);

    let err = repo.get_by_email("nobody@example.com").await.unwrap_err();
    assert!(matches!(err, DbError::NotFound));
}

#[tokio::test]
async fn update_role() {
    let env = common::TestEnv::new().await;
    let repo = UserRepository::new(env.db);
    let user = alice();

    repo.put(&user).await.unwrap();
    repo.update_role(&user.id, &Role::Member).await.unwrap();

    let fetched = repo.get(&user.id).await.unwrap();
    assert_eq!(fetched.role, Role::Member);
}

#[tokio::test]
async fn update_status() {
    let env = common::TestEnv::new().await;
    let repo = UserRepository::new(env.db);
    let user = alice();

    repo.put(&user).await.unwrap();
    repo.update_status(&user.id, &UserStatus::Suspended).await.unwrap();

    let fetched = repo.get(&user.id).await.unwrap();
    assert_eq!(fetched.status, UserStatus::Suspended);
}

#[tokio::test]
async fn list_returns_stored_users() {
    let env = common::TestEnv::new().await;
    let repo = UserRepository::new(env.db);

    let u1 = User::new("user1@example.com".into(), "User1".into());
    let u2 = User::new("user2@example.com".into(), "User2".into());
    repo.put(&u1).await.unwrap();
    repo.put(&u2).await.unwrap();

    let (users, cursor) = repo.list(10, None).await.unwrap();
    assert_eq!(users.len(), 2);
    assert!(cursor.is_none());
}

#[tokio::test]
async fn list_pagination() {
    let env = common::TestEnv::new().await;
    let repo = UserRepository::new(env.db);

    for i in 0..5u8 {
        repo.put(&User::new(format!("user{i}@example.com"), format!("User{i}")))
            .await
            .unwrap();
    }

    let (page1, cursor) = repo.list(3, None).await.unwrap();
    assert_eq!(page1.len(), 3);
    assert!(cursor.is_some(), "expected a next-page cursor");

    let (page2, cursor2) = repo.list(3, cursor).await.unwrap();
    assert_eq!(page2.len(), 2);
    assert!(cursor2.is_none());

    // no duplicates across pages
    let all_ids: std::collections::HashSet<_> =
        page1.iter().chain(page2.iter()).map(|u| &u.id).collect();
    assert_eq!(all_ids.len(), 5);
}
