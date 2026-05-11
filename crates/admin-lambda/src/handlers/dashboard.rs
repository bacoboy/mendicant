use askama::Template;
use axum::Router;
use axum::extract::State;
use axum::response::Html;
use axum::routing::get;

use crate::error::AppError;
use crate::handlers::NavUser;
use crate::handlers::util::{format_bytes, format_number};
use crate::middleware::AuthUser;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/admin", get(admin_page))
}

pub struct TableInfo {
    pub slug: &'static str,
    pub name: String,
    pub scope: &'static str,
    pub status: String,
    pub item_count: String,
    pub size: String,
    pub billing_mode: String,
    pub href: String,
}

#[derive(Template)]
#[template(path = "admin.html")]
#[allow(dead_code)]
struct AdminPage {
    nav: NavUser,
    tables: Vec<TableInfo>,
    active_section: &'static str,
    active_table: &'static str,
}

async fn admin_page(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Html<String>, AppError> {
    // (table_name, slug, scope, browse_href)
    let tables_config: &[(&str, &'static str, &'static str, &'static str)] = &[
        (&state.db.users_table, "users", "Global", "/admin/users"),
        (&state.db.credentials_table, "credentials", "Global", "/admin/users"),
        (&state.db.refresh_tokens_table, "refresh-tokens", "Global", "/admin/tables/refresh-tokens"),
        (&state.db.challenges_table, "challenges", "Regional", "/admin/tables/challenges"),
        (&state.db.email_tokens_table, "email-tokens", "Regional", "/admin/tables/email-tokens"),
        (&state.db.oauth_devices_table, "oauth-devices", "Regional", "/admin/tables/oauth-devices"),
    ];

    let mut tables = Vec::with_capacity(tables_config.len());
    for (table_name, slug, scope, browse_href) in tables_config {
        let info = match state.db.inner.describe_table().table_name(*table_name).send().await {
            Ok(resp) => {
                let td = resp.table();
                let item_count = td.and_then(|t| t.item_count()).unwrap_or(0);
                let size_bytes = td.and_then(|t| t.table_size_bytes()).unwrap_or(0);
                let status = td
                    .and_then(|t| t.table_status())
                    .map(|s| s.as_str().to_string())
                    .unwrap_or_else(|| "unknown".into());
                let billing_mode = td
                    .and_then(|t| t.billing_mode_summary())
                    .and_then(|b| b.billing_mode())
                    .map(|m| match m.as_str() {
                        "PAY_PER_REQUEST" => "On-demand".into(),
                        "PROVISIONED" => "Provisioned".into(),
                        other => other.to_string(),
                    })
                    .unwrap_or_else(|| "unknown".into());

                TableInfo {
                    slug,
                    name: table_name.to_string(),
                    scope,
                    status,
                    item_count: format_number(item_count),
                    size: format_bytes(size_bytes),
                    billing_mode,
                    href: browse_href.to_string(),
                }
            }
            Err(e) => {
                tracing::error!("describe_table failed for {}: {}", table_name, e);
                TableInfo {
                    slug,
                    name: table_name.to_string(),
                    scope,
                    status: "error".into(),
                    item_count: "—".into(),
                    size: "—".into(),
                    billing_mode: "—".into(),
                    href: browse_href.to_string(),
                }
            }
        };
        tables.push(info);
    }

    Ok(Html(
        AdminPage {
            nav: NavUser { email: claims.email.clone(), is_admin: true },
            tables,
            active_section: "dashboard",
            active_table: "",
        }.render().map_err(|e| anyhow::anyhow!(e))?,
    ))
}
