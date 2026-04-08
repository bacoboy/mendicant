/// Local API Gateway proxy for Lambda development.
///
/// Translates incoming HTTP requests into API Gateway v2 HTTP API payload format
/// and invokes the Lambda via the Lambda Runtime Interface Emulator (RIE).
/// Returns the Lambda's response as a plain HTTP response.
///
/// This simulates the AWS API Gateway → Lambda invocation model locally so that:
/// - Each HTTP request becomes one discrete Lambda invocation
/// - Lambda REPORT lines appear in the Lambda container logs per request
/// - The Lambda binary never needs to act as a long-running HTTP server
use axum::{Router, body::Body, extract::{Request, State}, response::Response, routing::any};
use axum::body::to_bytes;
use base64::Engine as _;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

const MAX_BODY_SIZE: usize = 10 * 1024 * 1024; // 10 MB

#[derive(Clone)]
struct AppState {
    client: Client,
    lambda_endpoint: String,
}

#[derive(Serialize)]
struct ApiGwV2Request<'a> {
    version: &'static str,
    #[serde(rename = "routeKey")]
    route_key: &'static str,
    #[serde(rename = "rawPath")]
    raw_path: &'a str,
    #[serde(rename = "rawQueryString")]
    raw_query_string: &'a str,
    cookies: Vec<String>,
    headers: HashMap<String, String>,
    #[serde(rename = "queryStringParameters")]
    query_string_parameters: HashMap<String, String>,
    #[serde(rename = "requestContext")]
    request_context: RequestContext<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(rename = "isBase64Encoded")]
    is_base64_encoded: bool,
}

#[derive(Serialize)]
struct RequestContext<'a> {
    #[serde(rename = "accountId")]
    account_id: &'static str,
    #[serde(rename = "apiId")]
    api_id: &'static str,
    #[serde(rename = "domainName")]
    domain_name: &'static str,
    #[serde(rename = "domainPrefix")]
    domain_prefix: &'static str,
    http: HttpContext<'a>,
    #[serde(rename = "requestId")]
    request_id: String,
    #[serde(rename = "routeKey")]
    route_key: &'static str,
    stage: &'static str,
    time: String,
    #[serde(rename = "timeEpoch")]
    time_epoch: u128,
}

#[derive(Serialize)]
struct HttpContext<'a> {
    method: String,
    path: &'a str,
    protocol: &'static str,
    #[serde(rename = "sourceIp")]
    source_ip: String,
    #[serde(rename = "userAgent")]
    user_agent: String,
}

#[derive(Deserialize, Debug)]
struct ApiGwV2Response {
    #[serde(rename = "statusCode")]
    status_code: u16,
    #[serde(default)]
    headers: HashMap<String, String>,
    #[serde(rename = "multiValueHeaders", default)]
    multi_value_headers: HashMap<String, Vec<String>>,
    #[serde(default)]
    cookies: Vec<String>,
    body: Option<String>,
    #[serde(rename = "isBase64Encoded", default)]
    is_base64_encoded: bool,
}

async fn proxy(State(state): State<AppState>, req: Request) -> Response {
    let method = req.method().to_string();
    let uri = req.uri().clone();
    let headers = req.headers().clone();

    let path = uri.path();
    let query = uri.query().unwrap_or("");

    let source_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("127.0.0.1")
        .to_string();

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Collect headers (lowercase names). API GW v2 uses comma-joined multi-values.
    let mut header_map: HashMap<String, String> = HashMap::new();
    for (name, value) in headers.iter() {
        let key = name.as_str().to_lowercase();
        if let Ok(val) = value.to_str() {
            header_map
                .entry(key)
                .and_modify(|e| {
                    e.push(',');
                    e.push_str(val);
                })
                .or_insert_with(|| val.to_string());
        }
    }

    // Extract cookies from the Cookie header.
    let cookies: Vec<String> = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(';').map(|c| c.trim().to_string()).collect())
        .unwrap_or_default();

    // Parse query string parameters (last value wins for duplicates, matching API GW behavior).
    let query_string_parameters: HashMap<String, String> = if query.is_empty() {
        HashMap::new()
    } else {
        url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect()
    };

    // Read the request body.
    let body_bytes = match to_bytes(req.into_body(), MAX_BODY_SIZE).await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("failed to read request body: {e}");
            return error_response(400, "Bad Request: body too large or unreadable");
        }
    };

    let body = if body_bytes.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&body_bytes).into_owned())
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let time_epoch_ms = now.as_millis();

    let event = ApiGwV2Request {
        version: "2.0",
        route_key: "$default",
        raw_path: path,
        raw_query_string: query,
        cookies,
        headers: header_map,
        query_string_parameters,
        request_context: RequestContext {
            account_id: "123456789012",
            api_id: "local",
            domain_name: "localhost",
            domain_prefix: "localhost",
            http: HttpContext {
                method,
                path,
                protocol: "HTTP/1.1",
                source_ip,
                user_agent,
            },
            request_id: Uuid::new_v4().to_string(),
            route_key: "$default",
            stage: "$default",
            time: format!("{time_epoch_ms}"),
            time_epoch: time_epoch_ms,
        },
        body,
        is_base64_encoded: false,
    };

    let lambda_res = match state
        .client
        .post(&state.lambda_endpoint)
        .json(&event)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Lambda invocation failed: {e}");
            return error_response(502, "Bad Gateway: Lambda unreachable");
        }
    };

    let body_bytes = match lambda_res.bytes().await {
        Ok(b) => b,
        Err(e) => {
            tracing::error!("Failed to read Lambda response: {e}");
            return error_response(502, "Bad Gateway: failed to read Lambda response");
        }
    };

    let lambda_response: ApiGwV2Response = match serde_json::from_slice(&body_bytes) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Failed to parse Lambda response: {e}");
            tracing::error!("Raw response: {}", String::from_utf8_lossy(&body_bytes));
            return error_response(502, "Bad Gateway: invalid Lambda response");
        }
    };

    let mut builder = Response::builder().status(lambda_response.status_code);

    // Single-value headers first, then multi-value headers override.
    for (name, value) in &lambda_response.headers {
        builder = builder.header(name, value);
    }
    for (name, values) in &lambda_response.multi_value_headers {
        for value in values {
            builder = builder.header(name, value);
        }
    }
    for cookie in &lambda_response.cookies {
        builder = builder.header("set-cookie", cookie);
    }

    let body_bytes = match lambda_response.body {
        None => vec![],
        Some(b) if lambda_response.is_base64_encoded => {
            base64::engine::general_purpose::STANDARD.decode(&b).unwrap_or_default()
        }
        Some(b) => b.into_bytes(),
    };

    builder.body(Body::from(body_bytes)).unwrap_or_else(|_| error_response(500, "Internal error"))
}

fn error_response(status: u16, msg: &'static str) -> Response {
    Response::builder()
        .status(status)
        .body(Body::from(msg))
        .unwrap()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "local_apigw=info".into()),
        )
        .init();

    let lambda_endpoint = std::env::var("LAMBDA_ENDPOINT").unwrap_or_else(|_| {
        "http://localhost:9000/2015-03-31/functions/function/invocations".into()
    });

    tracing::info!("Local API Gateway proxy");
    tracing::info!("Forwarding to Lambda: {lambda_endpoint}");

    let state = AppState {
        client: Client::new(),
        lambda_endpoint,
    };

    let app = Router::new()
        .fallback(any(proxy))
        .with_state(state);

    let addr = "0.0.0.0:3000";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tracing::info!("Listening on {addr}");
    axum::serve(listener, app).await.unwrap();
}
