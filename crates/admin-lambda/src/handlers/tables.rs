use askama::Template;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::response::Html;
use axum::routing::get;
use serde::Deserialize;

use crate::error::AppError;
use crate::handlers::NavUser;
use crate::handlers::util::{
    DdbItem, decode_browse_cursor, encode_browse_cursor, fmt_unix, title_case, trunc, val_bool,
    val_n, val_s,
};
use crate::middleware::AuthUser;
use crate::state::AppState;

const PAGE_SIZE: i32 = 25;

pub fn routes() -> Router<AppState> {
    Router::new().route("/admin/tables/{table}", get(table_page))
}

#[derive(Deserialize)]
struct TableBrowseQuery {
    cursor: Option<String>,
    page: Option<u32>,
}

struct TableCell {
    value: String,
    href: Option<String>,
}

impl TableCell {
    fn plain(s: impl Into<String>) -> Self { Self { value: s.into(), href: None } }
    fn linked(s: impl Into<String>, href: impl Into<String>) -> Self {
        Self { value: s.into(), href: Some(href.into()) }
    }
}

struct TableRow {
    cells: Vec<TableCell>,
}

#[derive(Template)]
#[template(path = "admin-table.html")]
#[allow(dead_code)]
struct AdminTableTemplate {
    nav: NavUser,
    table_name: String,
    table_slug: String,
    scope: &'static str,
    headers: Vec<&'static str>,
    rows: Vec<TableRow>,
    next_cursor: Option<String>,
    item_count: usize,
    current_page: u32,
    approx_total: i64,
    active_section: &'static str,
    active_table: String,
}

async fn table_page(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(slug): Path<String>,
    Query(q): Query<TableBrowseQuery>,
) -> Result<Html<String>, AppError> {
    let (ddb_table, scope, headers): (&str, &'static str, Vec<&'static str>) = match slug.as_str() {
        "refresh-tokens" => (
            &state.db.refresh_tokens_table,
            "Global",
            vec!["JTI", "User ID", "Expires", "Revoked"],
        ),
        "challenges" => (
            &state.db.challenges_table,
            "Regional",
            vec!["ID", "Type", "User ID", "Expires"],
        ),
        "email-tokens" => (
            &state.db.email_tokens_table,
            "Regional",
            vec!["ID", "Email", "Expires"],
        ),
        "oauth-devices" => (
            &state.db.oauth_devices_table,
            "Regional",
            vec!["User Code", "Status", "User ID", "Expires"],
        ),
        _ => return Err(AppError::NotFound),
    };

    let current_page = q.page.unwrap_or(1).max(1);

    let mut scan_req = state.db.inner
        .scan()
        .table_name(ddb_table)
        .limit(PAGE_SIZE);

    if let Some(ref cursor) = q.cursor {
        scan_req = scan_req.set_exclusive_start_key(Some(decode_browse_cursor(cursor)?));
    }

    let (scan_resp, describe_resp) = tokio::join!(
        scan_req.send(),
        state.db.inner.describe_table().table_name(ddb_table).send(),
    );

    let scan_resp = scan_resp.map_err(|e| AppError::Internal(anyhow::anyhow!(e)))?;

    let approx_total = describe_resp.ok()
        .and_then(|r| r.table)
        .and_then(|t| t.item_count)
        .unwrap_or(0);

    let next_cursor = scan_resp.last_evaluated_key
        .map(|k| encode_browse_cursor(&k))
        .transpose()
        .map_err(|e: anyhow::Error| AppError::Internal(e))?;

    let items = scan_resp.items.unwrap_or_default();
    let item_count = items.len();

    let rows: Vec<TableRow> = items.iter().map(|item| {
        TableRow {
            cells: match slug.as_str() {
                "refresh-tokens" => row_refresh_token(item),
                "challenges" => row_challenge(item),
                "email-tokens" => row_email_token(item),
                "oauth-devices" => row_oauth_device(item),
                _ => vec![],
            },
        }
    }).collect();

    Ok(Html(AdminTableTemplate {
        nav: NavUser { email: claims.email.clone(), is_admin: true },
        table_name: ddb_table.to_string(),
        active_table: slug.clone(),
        table_slug: slug,
        scope,
        headers,
        rows,
        next_cursor,
        item_count,
        current_page,
        approx_total,
        active_section: "tables",
    }.render().map_err(|e| anyhow::anyhow!(e))?))
}

// ── Per-table row mappers ─────────────────────────────────────────────────────

fn user_cell(user_id: &str) -> TableCell {
    if user_id == "—" || user_id.is_empty() {
        TableCell::plain(user_id)
    } else {
        TableCell::linked(trunc(user_id, 8), format!("/admin/users/{user_id}"))
    }
}

fn row_refresh_token(item: &DdbItem) -> Vec<TableCell> {
    let jti = val_s(item, "jti");
    let user_id = val_s(item, "user_id");
    let expires_n = val_n(item, "expires_at");
    vec![
        TableCell::plain(trunc(&jti, 8)),
        user_cell(&user_id),
        TableCell::plain(fmt_unix(&expires_n)),
        TableCell::plain(val_bool(item, "revoked")),
    ]
}

fn row_challenge(item: &DdbItem) -> Vec<TableCell> {
    let pk_val = val_s(item, "pk");
    let id = pk_val.strip_prefix("CHALLENGE#").unwrap_or(&pk_val);
    let user_id = val_s(item, "user_id");
    let expires_n = val_n(item, "expires_at");
    vec![
        TableCell::plain(trunc(id, 8)),
        TableCell::plain(title_case(&val_s(item, "challenge_type"))),
        user_cell(&user_id),
        TableCell::plain(fmt_unix(&expires_n)),
    ]
}

fn row_email_token(item: &DdbItem) -> Vec<TableCell> {
    let pk_val = val_s(item, "pk");
    let id = pk_val.strip_prefix("EMAIL_TOKEN#").unwrap_or(&pk_val);
    let expires_n = val_n(item, "expires_at");
    vec![
        TableCell::plain(trunc(id, 8)),
        TableCell::plain(val_s(item, "email")),
        TableCell::plain(fmt_unix(&expires_n)),
    ]
}

fn row_oauth_device(item: &DdbItem) -> Vec<TableCell> {
    let user_id = val_s(item, "user_id");
    let expires_n = val_n(item, "expires_at");
    vec![
        TableCell::plain(val_s(item, "user_code")),
        TableCell::plain(title_case(&val_s(item, "status"))),
        user_cell(&user_id),
        TableCell::plain(fmt_unix(&expires_n)),
    ]
}
