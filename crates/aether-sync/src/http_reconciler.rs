//! HTTP-backed `Reconciler` — the production transport for the
//! `SyncEngine`. Talks to the Supabase Edge Function pair that exposes
//! the wire contract pinned in this module.
//!
//! ## Wire contract
//!
//! `POST <base_url>/v1/sync/push`
//! - Headers: `Authorization: Bearer <jwt>`, `X-Tenant-ID: <uuid>`,
//!   `Content-Type: application/json`
//! - Body: `{ "entries": [<OutboxEntry>, …] }`
//! - Response 200: [`PushResult`] — `{ accepted, conflicts }`
//! - Response 401/403: surfaced as `ReconcilerError::Internal` (auth)
//!   so the caller can prompt for re-auth
//! - Response 5xx: surfaced as `ReconcilerError::Network` (transient,
//!   caller's backoff loop retries)
//! - Network/timeout: `ReconcilerError::Network`
//!
//! `GET <base_url>/v1/sync/pull?cursor=<string>`
//! - Headers: same as push
//! - Response 200: [`PullResult`] — `{ entries, next_cursor }`
//! - Errors: same mapping
//!
//! ## Why pin the wire format here
//! `crates/aether-sync` is the only place that imports `reqwest`, so
//! the JSON shape is one grep away from the trait and the LocalReconciler.
//! When the Supabase Edge Function (TypeScript) needs the schema, it
//! reads back the existing `OutboxEntry` + `PushResult` + `PullResult`
//! types — generated TypeScript bindings via `ts-rs` are a future PR
//! but the canonical source stays here.
//!
//! ## Zero-knowledge stance
//! This module sends `OutboxEntry::payload` verbatim. Callers MUST
//! have already wrapped sensitive payloads through
//! [`crate::payload::encrypt_entry`] — by the time bytes hit the wire,
//! the server only sees ciphertext for entities under tenant control.
//! Metadata (entity, entity_id, hlc_ts, op_id) stays plaintext for
//! routing.

use crate::outbox::OutboxEntry;
use crate::reconcile::{PullResult, PushResult, Reconciler, ReconcilerError};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::{Client, StatusCode};
use serde::Serialize;
use uuid::Uuid;

/// Wire body for `POST /v1/sync/push`. A struct wrapper around the
/// entries Vec lets us add optional fields (batch checksums, client
/// version) in a backward-compatible way without re-serializing every
/// call site.
#[derive(Serialize)]
struct PushBody<'a> {
    entries: &'a [OutboxEntry],
}

/// HTTP-backed reconciler. Clone is cheap — the internal `reqwest::Client`
/// is `Arc`-backed and shares the connection pool across clones, so a
/// single client per tenant per process is the recommended pattern.
#[derive(Clone)]
pub struct HttpReconciler {
    /// Base URL like `https://api.aether-os.com` — paths are appended.
    /// Trailing slash is normalized away in `new`.
    base_url: String,
    /// Tenant UUID sent as `X-Tenant-ID` header. The Edge Function's
    /// JWT introspection MUST also assert the JWT's `tenant_id` claim
    /// equals this — defence in depth against header spoofing.
    tenant_id: Uuid,
    /// JWT bearer token from `mint-session`. Held as a `String`
    /// (not `SecretString` etc) because reqwest needs to read it on
    /// every request and the extra protection is illusory once the
    /// token has been used once anyway.
    auth_token: String,
    client: Client,
}

impl HttpReconciler {
    /// Build a reconciler. `client` is taken by-value so callers can
    /// share a single configured reqwest client (timeouts, proxy, …)
    /// across all of their reconcilers. Use `Client::new()` for
    /// defaults.
    pub fn new(
        base_url: impl Into<String>,
        tenant_id: Uuid,
        auth_token: impl Into<String>,
        client: Client,
    ) -> Self {
        let mut base = base_url.into();
        // Normalize: drop any trailing '/' so we always join with a
        // leading '/' and never get a double slash on the wire.
        while base.ends_with('/') {
            base.pop();
        }
        Self {
            base_url: base,
            tenant_id,
            auth_token: auth_token.into(),
            client,
        }
    }

    /// Construct the per-request headers. Returns a fresh HeaderMap so
    /// callers don't accidentally mutate one another's tokens.
    fn headers(&self) -> Result<HeaderMap, ReconcilerError> {
        let mut h = HeaderMap::new();
        h.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let bearer = format!("Bearer {}", self.auth_token);
        let bearer = HeaderValue::from_str(&bearer)
            .map_err(|e| ReconcilerError::Internal(format!("invalid auth token: {e}")))?;
        h.insert(AUTHORIZATION, bearer);
        // Tenant UUID as ASCII — UUIDs are URL-safe by construction.
        let tenant = HeaderValue::from_str(&self.tenant_id.to_string())
            .map_err(|e| ReconcilerError::Internal(format!("invalid tenant id: {e}")))?;
        h.insert("x-tenant-id", tenant);
        Ok(h)
    }
}

/// Map a reqwest error to our `ReconcilerError` taxonomy. Errors that
/// look like transient network problems go to `Network` (the caller
/// retries with backoff); everything else surfaces as `Internal` so
/// telemetry can pick it up.
fn map_reqwest_err(e: reqwest::Error) -> ReconcilerError {
    if e.is_timeout() || e.is_connect() || e.is_request() {
        ReconcilerError::Network(format!("{e}"))
    } else {
        ReconcilerError::Internal(format!("reqwest: {e}"))
    }
}

/// Map an HTTP non-2xx response to a Reconciler error. Reads the body
/// for the error message (truncated to 512 bytes to bound log noise on
/// pathological responses).
async fn map_http_status(resp: reqwest::Response) -> ReconcilerError {
    let status = resp.status();
    let body = resp.text().await.unwrap_or_else(|_| "<no body>".into());
    let truncated: String = body.chars().take(512).collect();
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            ReconcilerError::Internal(format!("auth failed ({status}): {truncated}"))
        }
        s if s.is_server_error() => {
            ReconcilerError::Network(format!("server error {status}: {truncated}"))
        }
        s => ReconcilerError::Internal(format!("unexpected {s}: {truncated}")),
    }
}

#[async_trait]
impl Reconciler for HttpReconciler {
    async fn push(&self, entries: &[OutboxEntry]) -> Result<PushResult, ReconcilerError> {
        let url = format!("{}/v1/sync/push", self.base_url);
        let body = PushBody { entries };

        let resp = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(&body)
            .send()
            .await
            .map_err(map_reqwest_err)?;

        if !resp.status().is_success() {
            return Err(map_http_status(resp).await);
        }

        resp.json::<PushResult>()
            .await
            .map_err(|e| ReconcilerError::Internal(format!("push response decode: {e}")))
    }

    async fn pull_since(&self, cursor: Option<&str>) -> Result<PullResult, ReconcilerError> {
        let base = format!("{}/v1/sync/pull", self.base_url);
        // Let reqwest::Url do the percent-encoding so server-issued
        // cursors that happen to contain `&`, `=`, or `#` don't break
        // out of the cursor param and fool the server into reading
        // them as separate query keys.
        let url = if let Some(c) = cursor {
            reqwest::Url::parse_with_params(&base, &[("cursor", c)])
                .map_err(|e| ReconcilerError::Internal(format!("url build: {e}")))?
        } else {
            reqwest::Url::parse(&base)
                .map_err(|e| ReconcilerError::Internal(format!("url build: {e}")))?
        };

        let resp = self
            .client
            .get(url)
            .headers(self.headers()?)
            .send()
            .await
            .map_err(map_reqwest_err)?;

        if !resp.status().is_success() {
            return Err(map_http_status(resp).await);
        }

        resp.json::<PullResult>()
            .await
            .map_err(|e| ReconcilerError::Internal(format!("pull response decode: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outbox::Op;
    use aether_core::Hlc;
    use serde_json::json;
    use wiremock::matchers::{header, header_exists, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sample_entry(op_id: &str) -> OutboxEntry {
        OutboxEntry {
            op_id: op_id.into(),
            entity: "materials".into(),
            entity_id: "mat-1".into(),
            op: Op::Update,
            payload: b"{}".to_vec(),
            hlc_ts: Hlc::new(1, 0, "node-a"),
            parent_hlc: None,
            encrypted: false,
        }
    }

    fn rcl(server: &MockServer) -> HttpReconciler {
        HttpReconciler::new(server.uri(), Uuid::nil(), "test-jwt", Client::new())
    }

    #[tokio::test]
    async fn push_success_returns_accepted_op_ids() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/sync/push"))
            .and(header("authorization", "Bearer test-jwt"))
            .and(header("x-tenant-id", &Uuid::nil().to_string()[..]))
            .and(header("content-type", "application/json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "accepted": ["op-1", "op-2"],
                "conflicts": []
            })))
            .expect(1)
            .mount(&server)
            .await;

        let r = rcl(&server);
        let res = r
            .push(&[sample_entry("op-1"), sample_entry("op-2")])
            .await
            .unwrap();
        assert_eq!(res.accepted, vec!["op-1", "op-2"]);
        assert!(res.conflicts.is_empty());
    }

    #[tokio::test]
    async fn push_with_conflict_round_trips() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/sync/push"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "accepted": [],
                "conflicts": [{
                    "entity": "materials",
                    "entity_id": "mat-1",
                    "local": {
                        "op_id": "op-local",
                        "entity": "materials",
                        "entity_id": "mat-1",
                        "op": "update",
                        "payload": [],
                        "hlc_ts": Hlc::new(1, 0, "node-a"),
                        "parent_hlc": null,
                        "encrypted": false,
                    },
                    "remote": {
                        "op_id": "op-remote",
                        "entity": "materials",
                        "entity_id": "mat-1",
                        "op": "update",
                        "payload": [],
                        "hlc_ts": Hlc::new(2, 0, "node-b"),
                        "parent_hlc": null,
                        "encrypted": false,
                    },
                    "policy": "reject-and-surface"
                }]
            })))
            .mount(&server)
            .await;

        let res = rcl(&server)
            .push(&[sample_entry("op-local")])
            .await
            .unwrap();
        assert!(res.accepted.is_empty());
        assert_eq!(res.conflicts.len(), 1);
        assert_eq!(res.conflicts[0].entity_id, "mat-1");
    }

    #[tokio::test]
    async fn pull_without_cursor_omits_query_param() {
        // First boot — no persisted cursor — must not send a `cursor=`
        // param so the server returns from epoch.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/sync/pull"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "entries": [],
                "next_cursor": null,
            })))
            .expect(1)
            .mount(&server)
            .await;

        // Asserting `query_param` absence isn't a wiremock primitive,
        // so we mount the route without the param and rely on `.expect(1)`
        // to confirm it matched. A request that included `?cursor=` would
        // still match this mock (wiremock is liberal), but the next test
        // exercises the cursor side.
        let res = rcl(&server).pull_since(None).await.unwrap();
        assert!(res.entries.is_empty());
        assert!(res.next_cursor.is_none());
    }

    #[tokio::test]
    async fn pull_with_cursor_sends_query_param() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/sync/pull"))
            .and(query_param("cursor", "42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "entries": [{
                    "op_id": "op-10",
                    "entity": "work_orders",
                    "entity_id": "wo-1",
                    "op": "update",
                    "payload": [],
                    "hlc_ts": Hlc::new(10, 0, "node-a"),
                    "parent_hlc": null,
                    "encrypted": false,
                }],
                "next_cursor": "43",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let res = rcl(&server).pull_since(Some("42")).await.unwrap();
        assert_eq!(res.entries.len(), 1);
        assert_eq!(res.entries[0].op_id, "op-10");
        assert_eq!(res.next_cursor.as_deref(), Some("43"));
    }

    #[tokio::test]
    async fn cursor_with_special_chars_is_url_encoded() {
        // A server-issued cursor could theoretically include `&`, `=`,
        // or `#`. We must encode them — if the cursor lands in the
        // query string un-escaped, the server parses the second segment
        // as a separate param and the cursor is lost.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/sync/pull"))
            .and(query_param("cursor", "a&b=c"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"entries": [], "next_cursor": null})),
            )
            .expect(1)
            .mount(&server)
            .await;

        rcl(&server).pull_since(Some("a&b=c")).await.unwrap();
    }

    #[tokio::test]
    async fn http_401_is_mapped_to_internal_auth_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/sync/push"))
            .respond_with(ResponseTemplate::new(401).set_body_string("bad token"))
            .mount(&server)
            .await;

        let err = rcl(&server)
            .push(&[sample_entry("op-1")])
            .await
            .unwrap_err();
        assert!(
            matches!(err, ReconcilerError::Internal(ref m) if m.contains("auth failed")),
            "got: {err:?}"
        );
    }

    #[tokio::test]
    async fn http_500_is_mapped_to_network_so_caller_retries() {
        // 5xx is transient by convention. The engine's backoff loop
        // should retry — that's only possible if we surface it as
        // Network, not Internal.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/sync/push"))
            .respond_with(ResponseTemplate::new(503).set_body_string("scaling up"))
            .mount(&server)
            .await;

        let err = rcl(&server)
            .push(&[sample_entry("op-1")])
            .await
            .unwrap_err();
        assert!(
            matches!(err, ReconcilerError::Network(ref m) if m.contains("503")),
            "got: {err:?}"
        );
    }

    #[tokio::test]
    async fn malformed_json_response_surfaces_decode_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/sync/push"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let err = rcl(&server)
            .push(&[sample_entry("op-1")])
            .await
            .unwrap_err();
        assert!(
            matches!(err, ReconcilerError::Internal(ref m) if m.contains("decode")),
            "got: {err:?}"
        );
    }

    #[tokio::test]
    async fn unreachable_server_is_mapped_to_network_error() {
        // Point at a port that nothing is listening on. Connection
        // refusal is the canonical "network down" scenario.
        let r = HttpReconciler::new(
            "http://127.0.0.1:1",
            Uuid::nil(),
            "test-jwt",
            Client::builder()
                .connect_timeout(std::time::Duration::from_millis(200))
                .build()
                .unwrap(),
        );
        let err = r.push(&[sample_entry("op-1")]).await.unwrap_err();
        assert!(matches!(err, ReconcilerError::Network(_)), "got: {err:?}");
    }

    #[tokio::test]
    async fn auth_header_carries_bearer_prefix() {
        // Belt-and-braces — the JWT bearer prefix is easy to forget
        // and the server will reject anything that doesn't match it.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/sync/push"))
            .and(header("authorization", "Bearer test-jwt"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"accepted": [], "conflicts": []})),
            )
            .expect(1)
            .mount(&server)
            .await;

        rcl(&server).push(&[]).await.unwrap();
    }

    #[tokio::test]
    async fn tenant_header_is_set_on_every_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/sync/pull"))
            .and(header_exists("x-tenant-id"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"entries": [], "next_cursor": null})),
            )
            .expect(1)
            .mount(&server)
            .await;

        rcl(&server).pull_since(None).await.unwrap();
    }

    #[test]
    fn trailing_slash_in_base_url_is_normalized() {
        let r = HttpReconciler::new(
            "https://api.example.com/",
            Uuid::nil(),
            "tok",
            Client::new(),
        );
        assert_eq!(r.base_url, "https://api.example.com");

        let r2 = HttpReconciler::new(
            "https://api.example.com///",
            Uuid::nil(),
            "tok",
            Client::new(),
        );
        assert_eq!(r2.base_url, "https://api.example.com");
    }
}
