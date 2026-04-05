mod common;

use db::error::DbError;
use db::oauth_devices::OAuthDeviceRepository;
use domain::oauth::{DeviceGrant, DeviceGrantStatus};

fn grant() -> DeviceGrant {
    DeviceGrant::new(9999999999)
}

#[tokio::test]
async fn put_and_get_by_device_code() {
    let env = common::TestEnv::new().await;
    let repo = OAuthDeviceRepository::new(env.db.clone());
    let g = grant();

    repo.put(&g).await.unwrap();
    let fetched = repo.get_by_device_code(&g.device_code).await.unwrap();

    assert_eq!(fetched.device_code, g.device_code);
    assert_eq!(fetched.user_code, g.user_code);
    assert_eq!(fetched.status, DeviceGrantStatus::Pending);
    assert!(fetched.user_id.is_none());
}

#[tokio::test]
async fn get_by_user_code() {
    let env = common::TestEnv::new().await;
    let repo = OAuthDeviceRepository::new(env.db.clone());
    let g = grant();

    repo.put(&g).await.unwrap();
    let fetched = repo.get_by_user_code(&g.user_code).await.unwrap();

    assert_eq!(fetched.device_code, g.device_code);
}

#[tokio::test]
async fn duplicate_put_fails() {
    let env = common::TestEnv::new().await;
    let repo = OAuthDeviceRepository::new(env.db.clone());
    let g = grant();

    repo.put(&g).await.unwrap();
    let err = repo.put(&g).await.unwrap_err();
    assert!(matches!(err, DbError::ConditionalCheckFailed));
}

#[tokio::test]
async fn approve_sets_status_and_user_id() {
    let env = common::TestEnv::new().await;
    let repo = OAuthDeviceRepository::new(env.db.clone());
    let g = grant();

    repo.put(&g).await.unwrap();
    repo.approve(&g.device_code, "user-xyz").await.unwrap();

    let fetched = repo.get_by_device_code(&g.device_code).await.unwrap();
    assert_eq!(fetched.status, DeviceGrantStatus::Approved);
    assert_eq!(fetched.user_id.as_deref(), Some("user-xyz"));
}

#[tokio::test]
async fn approve_already_approved_fails() {
    let env = common::TestEnv::new().await;
    let repo = OAuthDeviceRepository::new(env.db.clone());
    let g = grant();

    repo.put(&g).await.unwrap();
    repo.approve(&g.device_code, "user-xyz").await.unwrap();

    // Second approve violates the condition_expression (status must be pending).
    let err = repo.approve(&g.device_code, "user-xyz").await.unwrap_err();
    assert!(matches!(err, DbError::ConditionalCheckFailed));
}

#[tokio::test]
async fn deny_sets_status() {
    let env = common::TestEnv::new().await;
    let repo = OAuthDeviceRepository::new(env.db.clone());
    let g = grant();

    repo.put(&g).await.unwrap();
    repo.deny(&g.device_code).await.unwrap();

    let fetched = repo.get_by_device_code(&g.device_code).await.unwrap();
    assert_eq!(fetched.status, DeviceGrantStatus::Denied);
}
