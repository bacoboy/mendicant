/// Datastar Server-Sent Events response builder.
///
/// Datastar consumes `text/event-stream` responses. Each event follows the
/// standard SSE format (`event:` + `data:` lines) with Datastar-specific event
/// names and data shapes.
use axum::body::Body;
use axum::http::{Response, StatusCode, header};
use axum::response::IntoResponse;

pub struct SseResponse {
    events: Vec<String>,
    cookies: Vec<String>,
}

impl SseResponse {
    pub fn new() -> Self {
        Self { events: vec![], cookies: vec![] }
    }

    /// Merge values into the Datastar signals store on the client.
    /// `signals` must be a valid JSON object string, e.g. `{"foo":"bar"}`.
    pub fn patch_signals(mut self, signals_json: &str) -> Self {
        self.events.push(format!("event: datastar-patch-signals\ndata: signals {signals_json}\n\n"));
        self
    }

    /// Execute a JavaScript expression on the client.
    pub fn execute_script(mut self, script: &str) -> Self {
        self.events.push(format!("event: datastar-execute-script\ndata: script {script}\n\n"));
        self
    }

    /// Redirect the browser after the SSE response is processed.
    pub fn redirect(self, url: &str) -> Self {
        self.execute_script(&format!("window.location.href='{url}'"))
    }
}

impl Default for SseResponse {
    fn default() -> Self {
        Self::new()
    }
}

impl IntoResponse for SseResponse {
    fn into_response(self) -> axum::response::Response {
        let body: String = self.events.concat();
        let mut builder = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header(header::CACHE_CONTROL, "no-cache")
            .header("X-Accel-Buffering", "no");

        for cookie in self.cookies {
            builder = builder.header(header::SET_COOKIE, cookie);
        }

        builder.body(Body::from(body)).unwrap()
    }
}
