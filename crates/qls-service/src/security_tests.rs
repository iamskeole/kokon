//! Security tests for access from other devices, through the real router: who gets in on each
//! listener, the attempt limit, what a signed-in browser can and cannot reach, and sessions
//! ending with a new password or a logout. No GPU, no network port opened.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use axum::{
    Router,
    body::Body,
    extract::connect_info::MockConnectInfo,
    http::{HeaderMap, Request, StatusCode, header},
};
use qls_core::{config::AppConfig, engine::Engine};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::{AppState, api, remote::RemoteState};

const PASSWORD: &str = "home server 42";

/// Settings are saved under `QLS_CONFIG_DIR`: a temporary folder, never the user's.
fn isolate_settings() {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        // SAFETY: set once, before any test reads it (every test calls this first).
        unsafe { std::env::set_var("QLS_CONFIG_DIR", d.path()) };
        d
    });
}

struct Fixture {
    _dir: tempfile::TempDir,
    state: Arc<AppState>,
}

fn fixture(with_password: bool) -> Fixture {
    isolate_settings();
    let dir = tempfile::tempdir().unwrap();
    let state = start(dir.path());
    if with_password {
        state.remote.set_password(PASSWORD).unwrap();
    }
    Fixture { _dir: dir, state }
}

/// The service starting on `dir` (again, for a restart).
fn start(dir: &std::path::Path) -> Arc<AppState> {
    // The engine folder inside the test's own: the default is the user's (a matting model installed
    // there made "installed" true).
    let engine = Engine::new(AppConfig { sdcpp_dir: dir.join("sdcpp"), ..AppConfig::default() }, dir).unwrap();
    Arc::new(AppState {
        engine,
        token: "t".repeat(64),
        leases: Mutex::new(HashMap::new()),
        exit_deadline: Mutex::new(None),
        persistent: true,
        quick_exit: std::sync::atomic::AtomicBool::new(false),
        shutdown: tokio::sync::Notify::new(),
        started: Instant::now(),
        remote: RemoteState::new(dir, false),
    })
}

fn app(f: &Fixture, lan: bool, ip: &str) -> Router {
    let addr: SocketAddr = format!("{ip}:50000").parse().unwrap();
    api::router(f.state.clone(), lan).layer(MockConnectInfo(addr))
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

impl Reply {
    fn code(&self) -> &str {
        self.body["error"]["code"].as_str().unwrap_or("")
    }
}

async fn send(
    app: &Router,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<Value>,
) -> Reply {
    let mut req = Request::builder().method(method).uri(path);
    for (k, v) in headers {
        req = req.header(*k, *v);
    }
    let req = match body {
        Some(b) => req
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(b.to_string())),
        None => req.body(Body::empty()),
    }
    .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let bytes = axum::body::to_bytes(res.into_body(), 16 << 20)
        .await
        .unwrap();
    Reply {
        status,
        headers,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

const CSRF: (&str, &str) = ("x-kokon-csrf", "1");

async fn login(app: &Router, password: &str) -> Reply {
    send(
        app,
        "POST",
        "/api/v1/auth/login",
        &[CSRF],
        Some(json!({ "password": password })),
    )
    .await
}

/// The `Cookie` header value for a successful login.
async fn sign_in(app: &Router) -> String {
    let r = login(app, PASSWORD).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let set = r.headers[header::SET_COOKIE].to_str().unwrap();
    for attr in ["HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(set.contains(attr), "cookie lacks {attr}: {set}");
    }
    set.split(';').next().unwrap().to_string()
}

#[tokio::test]
async fn nothing_without_credentials() {
    let f = fixture(true);
    for lan in [false, true] {
        let a = app(&f, lan, "192.168.1.50");
        for (m, p) in [
            ("GET", "/api/v1/status"),
            ("GET", "/api/v1/gallery"),
            ("GET", "/api/v1/events"),
            ("POST", "/api/v1/jobs"),
        ] {
            let r = send(&a, m, p, &[], None).await;
            assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{m} {p} lan={lan}");
            assert_eq!(r.code(), "login_required");
        }
        let r = send(&a, "GET", "/api/v1/gallery/x/image?token=", &[], None).await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
        // A cookie that no login handed out.
        let r = send(
            &a,
            "GET",
            "/api/v1/status",
            &[("cookie", "kokon_session=0123456789abcdef")],
            None,
        )
        .await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    }
}

#[tokio::test]
async fn the_token_works_on_loopback_only() {
    let f = fixture(true);
    let auth = format!("Bearer {}", f.state.token);
    let local = app(&f, false, "127.0.0.1");
    assert_eq!(
        send(
            &local,
            "GET",
            "/api/v1/remote",
            &[("authorization", &auth)],
            None
        )
        .await
        .status,
        StatusCode::OK
    );
    let wrong = send(
        &local,
        "GET",
        "/api/v1/status",
        &[("authorization", "Bearer nope")],
        None,
    )
    .await;
    assert_eq!(
        (wrong.status, wrong.code()),
        (StatusCode::UNAUTHORIZED, "unauthorized")
    );
    // From a web page in a browser, even with the token.
    let page = send(
        &local,
        "GET",
        "/api/v1/status",
        &[("authorization", &auth), ("origin", "http://evil.example")],
        None,
    )
    .await;
    assert_eq!(page.status, StatusCode::UNAUTHORIZED);
    // The network listener never takes it.
    let lan = app(&f, true, "192.168.1.50");
    let r = send(
        &lan,
        "GET",
        "/api/v1/status",
        &[("authorization", &auth)],
        None,
    )
    .await;
    assert_eq!(
        (r.status, r.code()),
        (StatusCode::UNAUTHORIZED, "login_required")
    );
    let r = send(
        &lan,
        "GET",
        &format!("/api/v1/status?token={}", f.state.token),
        &[],
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn no_login_without_a_password() {
    let f = fixture(false);
    let a = app(&f, true, "192.168.1.50");
    let r = login(&a, "anything at all").await;
    assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "forbidden"));
    let s = send(&a, "GET", "/api/v1/auth/session", &[], None).await;
    assert_eq!(s.body, json!({ "signed_in": false, "user": null, "password_set": false, "multi_user": false }));
}

#[tokio::test]
async fn login_needs_the_csrf_header() {
    let f = fixture(true);
    let a = app(&f, true, "192.168.1.50");
    let r = send(
        &a,
        "POST",
        "/api/v1/auth/login",
        &[],
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    assert!(!r.headers.contains_key(header::SET_COOKIE));
}

#[tokio::test]
async fn wrong_passwords_lock_the_address() {
    let f = fixture(true);
    let a = app(&f, true, "192.168.1.66");
    for _ in 0..4 {
        let r = login(&a, "guess guess").await;
        assert_eq!(
            (r.status, r.code()),
            (StatusCode::UNAUTHORIZED, "login_required")
        );
    }
    let r = login(&a, "guess guess").await;
    assert_eq!(
        (r.status, r.code()),
        (StatusCode::TOO_MANY_REQUESTS, "too_many_attempts")
    );
    assert_eq!(r.body["retry_after_s"], 60);
    assert_eq!(r.headers[header::RETRY_AFTER], "60");
    // Locked: even the right password gets the same answer, and no cookie.
    let r = login(&a, PASSWORD).await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    assert!(!r.headers.contains_key(header::SET_COOKIE));
    // Another device on the network is not affected.
    sign_in(&app(&f, true, "192.168.1.67")).await;
}

#[tokio::test]
async fn a_browser_gets_the_ui_api_but_no_paths_on_the_server() {
    let f = fixture(true);
    let a = app(&f, true, "192.168.1.50");
    let cookie = sign_in(&a).await;
    let c = ("cookie", cookie.as_str());
    assert_eq!(
        send(&a, "GET", "/api/v1/status", &[c], None).await.status,
        StatusCode::OK
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/gallery", &[c], None).await.status,
        StatusCode::OK
    );
    let s = send(&a, "GET", "/api/v1/auth/session", &[c], None).await;
    assert_eq!(s.body, json!({ "signed_in": true, "user": null, "password_set": true, "multi_user": false }));
    for (m, p, body) in [
        (
            "POST",
            "/api/v1/inputs/import",
            json!({ "path": "/etc/passwd" }),
        ),
        ("POST", "/api/v1/models/scan", json!({ "dir": "/" })),
        (
            "POST",
            "/api/v1/models/inspect",
            json!({ "path": "/etc/shadow" }),
        ),
        (
            "POST",
            "/api/v1/gallery/x/export",
            json!({ "path": "/tmp/x.png" }),
        ),
        (
            "POST",
            "/api/v1/image-info",
            json!({ "path": "/etc/passwd" }),
        ),
        (
            "POST",
            "/api/v1/loras",
            json!({ "path": "/tmp/x.safetensors" }),
        ),
        (
            "POST",
            "/api/v1/luts/import",
            json!({ "path": "/tmp/x.cube" }),
        ),
        (
            "POST",
            "/api/v1/generate",
            json!({ "prompt": "x", "output_path": "/tmp/x.png" }),
        ),
        ("POST", "/api/v1/shutdown", json!({})),
        (
            "POST",
            "/api/v1/remote/password",
            json!({ "password": "taken over!" }),
        ),
        ("POST", "/api/v1/remote", json!({ "enabled": true })),
        ("GET", "/api/v1/remote", Value::Null),
    ] {
        let body = (!body.is_null()).then_some(body);
        let r = send(&a, m, p, &[c, CSRF], body).await;
        assert_eq!(
            (r.status, r.code()),
            (StatusCode::FORBIDDEN, "forbidden"),
            "{m} {p}"
        );
    }
    // A job may only use images already in Kokon, and saves to the gallery.
    for job in [
        json!({ "prompt": "x", "mode": "edit", "images": ["/etc/passwd"] }),
        json!({ "prompt": "x", "output_path": "/tmp/x.png" }),
    ] {
        let r = send(&a, "POST", "/api/v1/jobs", &[c, CSRF], Some(job)).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN);
        let r = send(
            &a,
            "POST",
            "/api/v1/plan",
            &[c, CSRF],
            Some(json!({ "prompt": "x", "mode": "edit", "images": ["C:\\Users\\me\\x.png"] })),
        )
        .await;
        assert_eq!(r.status, StatusCode::FORBIDDEN);
    }
    // The password is never in the settings the API returns.
    let cfg = send(&a, "GET", "/api/v1/config", &[c], None).await;
    assert!(!cfg.body.to_string().contains("argon2"));
}

#[tokio::test]
async fn browser_changes_need_the_csrf_header() {
    let f = fixture(true);
    let a = app(&f, true, "192.168.1.50");
    let cookie = sign_in(&a).await;
    let c = ("cookie", cookie.as_str());
    let beat = json!({ "client_id": "tab-1" });
    let r = send(
        &a,
        "POST",
        "/api/v1/session/heartbeat",
        &[c],
        Some(beat.clone()),
    )
    .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = send(
        &a,
        "POST",
        "/api/v1/session/heartbeat",
        &[c, CSRF],
        Some(beat),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    // A browser's lease is never the desktop window's, and never kept alive by a pid.
    let l = f.state.leases.lock().unwrap();
    assert_eq!((l["tab-1"].kind.as_str(), l["tab-1"].pid), ("web", None));
}

#[tokio::test]
async fn browser_settings_saves_keep_the_computers_settings() {
    let f = fixture(true);
    let a = app(&f, true, "192.168.1.50");
    let cookie = sign_in(&a).await;
    let c = ("cookie", cookie.as_str());
    let before = f.state.engine.config();
    let mut sent = serde_json::to_value(&before).unwrap();
    sent["sdcppDir"] = json!("/tmp/elsewhere");
    sent["extraServerArgs"] = json!(["--rpc", "evil:50052"]);
    sent["remoteAccess"] = json!(true);
    sent["flashAttention"] = json!(!before.flash_attention);
    let r = send(&a, "PUT", "/api/v1/config", &[c, CSRF], Some(sent)).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let after = f.state.engine.config();
    assert_eq!(after.sdcpp_dir, before.sdcpp_dir);
    assert_eq!(after.extra_server_args, before.extra_server_args);
    assert!(!after.remote_access);
    assert_eq!(
        after.flash_attention, !before.flash_attention,
        "an app setting does change"
    );
}

#[tokio::test]
async fn a_new_password_or_a_logout_ends_sessions() {
    let f = fixture(true);
    let a = app(&f, true, "192.168.1.50");
    let phone = sign_in(&a).await;
    let laptop = sign_in(&a).await;
    let r = send(
        &a,
        "POST",
        "/api/v1/auth/logout",
        &[("cookie", &laptop), CSRF],
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(
        r.headers[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/status", &[("cookie", &laptop)], None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/status", &[("cookie", &phone)], None)
            .await
            .status,
        StatusCode::OK
    );
    // Changed from the desktop app (token, loopback).
    let local = app(&f, false, "127.0.0.1");
    let auth = format!("Bearer {}", f.state.token);
    let r = send(
        &local,
        "POST",
        "/api/v1/remote/password",
        &[("authorization", &auth)],
        Some(json!({ "password": "a brand new one" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(
        send(&a, "GET", "/api/v1/status", &[("cookie", &phone)], None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(login(&a, PASSWORD).await.status, StatusCode::UNAUTHORIZED);
    assert_eq!(login(&a, "a brand new one").await.status, StatusCode::OK);
    let short = send(
        &local,
        "POST",
        "/api/v1/remote/password",
        &[("authorization", &auth)],
        Some(json!({ "password": "short" })),
    )
    .await;
    assert_eq!(short.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn a_sign_in_survives_a_restart_until_the_password_changes() {
    let mut f = fixture(true);
    let a = app(&f, true, "192.168.1.50");
    let phone = sign_in(&a).await;
    let kept = std::fs::read_to_string(f._dir.path().join("remote-sessions.json")).unwrap();
    assert!(
        !kept.contains(phone.split_once('=').unwrap().1),
        "only a hash of the cookie is kept"
    );
    f.state = start(f._dir.path());
    let a = app(&f, true, "192.168.1.50");
    assert_eq!(
        send(&a, "GET", "/api/v1/status", &[("cookie", &phone)], None)
            .await
            .status,
        StatusCode::OK
    );
    // Set from a terminal while the service was off.
    qls_core::remote::set_password(
        &qls_core::remote::password_file(f._dir.path()),
        "set while off",
    )
    .unwrap();
    f.state = start(f._dir.path());
    let a = app(&f, true, "192.168.1.50");
    assert_eq!(
        send(&a, "GET", "/api/v1/status", &[("cookie", &phone)], None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn the_network_stays_closed_without_a_password() {
    let f = fixture(false);
    let local = app(&f, false, "127.0.0.1");
    let auth = format!("Bearer {}", f.state.token);
    let r = send(
        &local,
        "POST",
        "/api/v1/remote",
        &[("authorization", &auth)],
        Some(json!({ "enabled": true })),
    )
    .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    let st = send(
        &local,
        "GET",
        "/api/v1/remote",
        &[("authorization", &auth)],
        None,
    )
    .await
    .body;
    assert_eq!(
        (
            st["enabled"].as_bool(),
            st["listening"].as_bool(),
            st["password_set"].as_bool()
        ),
        (Some(false), Some(false), Some(false))
    );
    // Removing the password turns access off.
    f.state
        .engine
        .set_remote(true, f.state.engine.config().remote_port)
        .unwrap();
    let r = send(
        &local,
        "DELETE",
        "/api/v1/remote/password",
        &[("authorization", &auth)],
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert!(!f.state.engine.config().remote_access);
    assert_eq!(r.body["listening"], false);
}

#[tokio::test]
async fn a_browser_opens_a_project_with_one_request_and_previews() {
    let f = fixture(true);
    let a = app(&f, true, "192.168.1.50");
    let cookie = sign_in(&a).await;
    let c = ("cookie", cookie.as_str());
    let img = image::RgbaImage::from_pixel(2000, 1000, image::Rgba([10, 20, 30, 255]));
    let png = qls_core::imaging::encode_png(&img, &[], None, true).unwrap();
    let input = f
        .state
        .engine
        .store
        .put_input(&png, Some("a.png"), None)
        .unwrap();
    let project = json!({ "meta": { "id": "p", "title": "t", "updatedAt": 1.0 }, "items": [
        { "key": "i", "kind": "input", "inputId": input.id },
        { "key": "r-gone", "kind": "gen", "genId": "gone" },
    ] });
    let r = send(
        &a,
        "PUT",
        "/api/v1/projects/p",
        &[c, CSRF],
        Some(json!({ "project": project })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    let r = send(&a, "GET", "/api/v1/projects/p/records", &[c], None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["inputs"][&input.id]["width"], 2000);
    assert_eq!(
        r.body["generations"],
        json!({}),
        "a deleted version is left out"
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/projects/nope/records", &[c], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let r = send(
        &a,
        "GET",
        &format!("/api/v1/inputs/{}/preview", input.id),
        &[c],
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.headers[header::CONTENT_TYPE], "image/jpeg");
    assert!(
        r.headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    assert!(
        f.state
            .engine
            .store
            .input_preview_path(&input.id)
            .with_extension("preview.jpg")
            .is_file()
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/gallery/gone/preview", &[c], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    // Signed out: nothing.
    assert_eq!(
        send(&a, "GET", "/api/v1/projects/p/records", &[], None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn fast_stickers_take_only_images_already_in_kokon() {
    let f = fixture(true);
    let a = app(&f, true, "192.168.1.50");
    let cookie = sign_in(&a).await;
    let c = ("cookie", cookie.as_str());
    let s = send(&a, "GET", "/api/v1/matte", &[c], None).await;
    assert_eq!((s.status, s.body["installed"].clone()), (StatusCode::OK, json!(false)));
    for image in ["/etc/passwd", r"C:\Users\me\x.png", "file:///etc/passwd"] {
        let r = send(&a, "POST", "/api/v1/matte", &[c, CSRF], Some(json!({ "image": image }))).await;
        assert_eq!((r.status, r.code()), (StatusCode::BAD_REQUEST, "invalid_input"), "{image}");
    }
    // Stop: signed in with the CSRF header only.
    let stop = json!({ "run": "r1" });
    let r = send(&a, "POST", "/api/v1/matte/cancel", &[c, CSRF], Some(stop.clone())).await;
    assert_eq!((r.status, r.body["stopped"].clone()), (StatusCode::OK, json!(false)));
    assert_ne!(send(&a, "POST", "/api/v1/matte/cancel", &[c], Some(stop.clone())).await.status, StatusCode::OK);
    assert_eq!(send(&a, "POST", "/api/v1/matte/cancel", &[CSRF], Some(stop)).await.status, StatusCode::UNAUTHORIZED);
}

/// Behind a reverse proxy on the private network (the one serving the public subdomain) the
/// lockout follows the address the proxy reports, so one client being locked out does not
/// lock out everyone behind the proxy.
#[tokio::test]
async fn a_reverse_proxy_reports_the_real_address() {
    let f = fixture(true);
    // The proxy's own address is where the socket comes from; the client is what it reports.
    let proxy = app(&f, true, "192.168.1.20");
    for _ in 0..4 {
        let r = send(
            &proxy,
            "POST",
            "/api/v1/auth/login",
            &[("x-forwarded-for", "203.0.113.7"), CSRF],
            Some(json!({ "password": "guess guess" })),
        )
        .await;
        assert_eq!((r.status, r.code()), (StatusCode::UNAUTHORIZED, "login_required"));
    }
    // The fifth wrong one locks the reported address (429, same answer for the right password).
    let r = send(
        &proxy,
        "POST",
        "/api/v1/auth/login",
        &[("x-forwarded-for", "203.0.113.7"), CSRF],
        Some(json!({ "password": "guess guess" })),
    )
    .await;
    assert_eq!((r.status, r.code()), (StatusCode::TOO_MANY_REQUESTS, "too_many_attempts"));
    let r = send(
        &proxy,
        "POST",
        "/api/v1/auth/login",
        &[("x-forwarded-for", "203.0.113.7"), CSRF],
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    // Another client through the same proxy is unaffected.
    let r = send(
        &proxy,
        "POST",
        "/api/v1/auth/login",
        &[("x-forwarded-for", "203.0.113.9"), CSRF],
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
}

#[tokio::test]
async fn a_direct_peers_own_forwarded_for_is_not_believed() {
    let f = fixture(true);
    // A peer off the private network (the firewall never lets one reach the port directly,
    // but the rule must hold): its header can rotate, so the lock follows the socket.
    let a = app(&f, true, "203.0.113.7");
    for _ in 0..4 {
        let r = send(
            &a,
            "POST",
            "/api/v1/auth/login",
            &[("x-forwarded-for", "10.9.9.9"), CSRF],
            Some(json!({ "password": "guess guess" })),
        )
        .await;
        assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    }
    let r = send(
        &a,
        "POST",
        "/api/v1/auth/login",
        &[("x-forwarded-for", "10.9.9.9"), CSRF],
        Some(json!({ "password": "guess guess" })),
    )
    .await;
    assert_eq!(
        (r.status, r.code()),
        (StatusCode::TOO_MANY_REQUESTS, "too_many_attempts")
    );
    // Rotating the header does not dodge the lock: it is keyed to the socket.
    let r = send(
        &a,
        "POST",
        "/api/v1/auth/login",
        &[("x-forwarded-for", "10.8.8.8"), CSRF],
        Some(json!({ "password": "anything else" })),
    )
    .await;
    assert_eq!(r.status, StatusCode::TOO_MANY_REQUESTS);
    // Nor does a direct peer's claim about the scheme: no Secure cookie for it.
    let r = send(
        &app(&f, true, "203.0.113.9"),
        "POST",
        "/api/v1/auth/login",
        &[("x-forwarded-proto", "https"), CSRF],
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let set = r.headers[header::SET_COOKIE].to_str().unwrap();
    assert!(!set.contains("Secure"), "{set}");
}

#[tokio::test]
async fn the_secure_cookie_matches_the_tls_the_proxy_says() {
    let f = fixture(true);
    // Through the proxy, over TLS: the cookie carries Secure.
    let proxy = app(&f, true, "192.168.1.20");
    let r = send(
        &proxy,
        "POST",
        "/api/v1/auth/login",
        &[
            ("x-forwarded-for", "203.0.113.7"),
            ("x-forwarded-proto", "https"),
            CSRF,
        ],
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let set = r.headers[header::SET_COOKIE].to_str().unwrap();
    assert!(set.contains("Secure"), "{set}");
    let cookie = set.split(';').next().unwrap().trim().to_string();
    // The clear mirrors it (a Secure cookie is only deleted by a Secure one).
    let r = send(
        &proxy,
        "POST",
        "/api/v1/auth/logout",
        &[
            ("cookie", cookie.as_str()),
            ("x-forwarded-proto", "https"),
            CSRF,
        ],
        None,
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let clear = r.headers[header::SET_COOKIE].to_str().unwrap();
    assert!(clear.contains("Secure") && clear.contains("Max-Age=0"), "{clear}");
    // And the browser is actually signed out: the (cleared) cookie no longer opens the API.
    assert_eq!(
        send(&proxy, "GET", "/api/v1/status", &[("cookie", cookie.as_str())], None)
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    // On plain HTTP (the LAN as it runs today) the cookie stays without Secure.
    let r = send(
        &proxy,
        "POST",
        "/api/v1/auth/login",
        &[("x-forwarded-for", "203.0.113.7"), CSRF],
        Some(json!({ "password": PASSWORD })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let set = r.headers[header::SET_COOKIE].to_str().unwrap();
    assert!(!set.contains("Secure"), "{set}");
}

// ---------------------------------------------------------------------------------------
// Multi-user (D-MU1..D-MU12): named accounts see only their own rows; the operator sees all

/// A named account's login: its name and its own password.
async fn named_sign_in(app: &Router, name: &str, password: &str) -> String {
    let r = send(
        app,
        "POST",
        "/api/v1/auth/login",
        &[CSRF],
        Some(json!({ "password": password, "username": name })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    r.headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

fn fake_row(id: &str, user: Option<&str>) -> qls_core::store::GenerationRecord {
    serde_json::from_value(json!({
        "id": id, "created_at": "t", "mode": "generate", "prompt": "p", "negative_prompt": "",
        "seed": 1, "width": 8, "height": 8, "working_width": 32, "working_height": 32,
        "model": "m", "sampler": "euler", "scheduler": null, "steps": 1, "cfg": 6.0, "strength": null,
        "quality": "fast", "resolution": "auto", "references": [], "mask_input_id": null, "outpaint": null,
        "file_path": format!("/outside-the-gallery/{id}.png"), "favorite": false, "parent_id": null,
        "duration_ms": 1, "notes": [], "app_version": "1", "backend_version": null, "client": null,
        "user": user.map(str::to_owned)
    }))
    .unwrap()
}

#[tokio::test]
async fn named_accounts_see_only_their_own_rows() {
    let f = fixture(true);
    let a = app(&f, true, "192.168.1.50");
    // The token only works on the loopback listener, where the desktop app and the scripts sit.
    let tl = app(&f, false, "127.0.0.1");
    let token = format!("Bearer {}", f.state.token);
    let t = ("authorization", token.as_str());

    // The operator (token) turns the gate on and creates two accounts.
    assert_eq!(
        send(&tl, "POST", "/api/v1/users/enable", &[t, CSRF], Some(json!({ "on": true }))).await.status,
        StatusCode::OK
    );
    for (name, pw) in [("alice", "alice's pass 1"), ("bob", "bob's pass 2")] {
        let r = send(
            &tl,
            "POST",
            "/api/v1/users",
            &[t, CSRF],
            Some(json!({ "name": name, "password": pw })),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
        assert_eq!(r.body["name"], name);
    }
    let bob_id = send(&tl, "GET", "/api/v1/users", &[t], None)
        .await
        .body["users"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["name"] == "bob")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Three rows: one per owner, one the operator's (no owner, as before accounts).
    let rows = [
        fake_row("row-alice", Some("alice")),
        fake_row("row-bob", Some("bob")),
        fake_row("row-mine", None),
    ];
    for r in &rows {
        f.state.engine.store.insert_generation(r).unwrap();
    }

    // Named logins: each gets a session that says who they are.
    let alice_cookie = named_sign_in(&a, "alice", "alice's pass 1").await;
    let alice = ("cookie", alice_cookie.as_str());
    let bob_cookie = named_sign_in(&a, "bob", "bob's pass 2").await;
    let bob = ("cookie", bob_cookie.as_str());
    let s = send(&a, "GET", "/api/v1/auth/session", &[alice], None).await;
    assert_eq!(
        s.body,
        json!({ "signed_in": true, "user": "alice", "password_set": true, "multi_user": true })
    );
    // The shared password still signs in the operator (no name), as before.
    let op_cookie = sign_in(&a).await;
    let op = ("cookie", op_cookie.as_str());
    let s = send(&a, "GET", "/api/v1/auth/session", &[op], None).await;
    assert_eq!(s.body["user"], json!(null));
    assert_eq!(s.body["multi_user"], json!(true));

    // Wrong name, wrong password: distinct messages, same 401 (D-MU4).
    let r = send(
        &a,
        "POST",
        "/api/v1/auth/login",
        &[CSRF],
        Some(json!({ "password": "nope", "username": "alice" })),
    )
    .await;
    assert_eq!((r.status, r.body["error"]["message"].as_str().unwrap()), (StatusCode::UNAUTHORIZED, "Wrong password."));
    let r = send(
        &a,
        "POST",
        "/api/v1/auth/login",
        &[CSRF],
        Some(json!({ "password": "nope", "username": "ghost" })),
    )
    .await;
    assert_eq!((r.status, r.body["error"]["message"].as_str().unwrap()), (StatusCode::UNAUTHORIZED, "Unknown name."));

    // Each account lists its own row only, whatever filter it asks for.
    let g = send(&a, "GET", "/api/v1/gallery", &[alice], None).await;
    assert_eq!(g.body["total"], 1);
    assert_eq!(g.body["items"][0]["id"], "row-alice");
    let g = send(&a, "GET", "/api/v1/gallery?user=bob", &[alice], None).await;
    assert_eq!(g.body["total"], 1, "a named account cannot widen its list");
    assert_eq!(g.body["items"][0]["id"], "row-alice");
    let g = send(&a, "GET", "/api/v1/gallery", &[bob], None).await;
    assert_eq!(g.body["items"][0]["id"], "row-bob");

    // Another account's row is unknown, like a deleted one (no existence leak).
    for (m, p, body) in [
        ("GET", "/api/v1/gallery/row-bob", None),
        ("PATCH", "/api/v1/gallery/row-bob", Some(json!({ "favorite": true }))),
        ("DELETE", "/api/v1/gallery/row-bob", None),
        ("GET", "/api/v1/gallery/row-bob/image", None),
        ("GET", "/api/v1/gallery/row-bob/thumb", None),
        ("GET", "/api/v1/gallery/row-bob/preview", None),
    ] {
        let r = send(&a, m, p, &[alice, CSRF], body).await;
        assert_eq!((r.status, r.code()), (StatusCode::NOT_FOUND, "not_found"), "{m} {p}");
    }
    // Its own row is fully readable.
    assert_eq!(
        send(&a, "GET", "/api/v1/gallery/row-alice", &[alice], None).await.status,
        StatusCode::OK
    );
    // A plan referring to another account's image: unknown source (the same rule `submit`
    // applies; a real job is checked by the external-engine harness, which has a model).
    let r = send(
        &a,
        "POST",
        "/api/v1/plan",
        &[alice, CSRF],
        Some(json!({ "prompt": "x", "mode": "edit", "images": ["gen:row-bob"] })),
    )
    .await;
    assert_eq!((r.status, r.code()), (StatusCode::NOT_FOUND, "not_found"), "{:?}", r.body);

    // The operator sees all three and can narrow.
    let g = send(&a, "GET", "/api/v1/gallery", &[op], None).await;
    assert_eq!(g.body["total"], 3);
    for (param, want) in [("user=alice", 1), ("user=bob", 1), ("user=me", 1), ("user=all", 3)] {
        let g = send(&a, "GET", &format!("/api/v1/gallery?{param}").as_str(), &[op], None).await;
        assert_eq!(g.body["total"], want, "{param}");
    }
    assert_eq!(
        send(&a, "GET", "/api/v1/gallery/row-bob", &[op], None).await.status,
        StatusCode::OK
    );

    // User management is the operator's (D-MU7): a named account gets a 403, not a list.
    let r = send(&a, "GET", "/api/v1/users", &[alice], None).await;
    assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "forbidden"));
    let r = send(
        &a,
        "POST",
        "/api/v1/users/enable",
        &[alice, CSRF],
        Some(json!({ "on": true })),
    )
    .await;
    assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "forbidden"));

    // Deleting bob takes his row and his session; alice and the operator are untouched.
    let r = send(&tl, "DELETE", &format!("/api/v1/users/{bob_id}").as_str(), &[t, CSRF], None).await;
    assert_eq!(r.body, json!({ "deleted": "bob", "rows_removed": 1, "projects_removed": 0 }));
    assert_eq!(
        send(&a, "GET", "/api/v1/auth/session", &[bob], None).await.body["signed_in"],
        false
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/gallery", &[op], None).await.body["total"],
        2
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/gallery", &[alice], None).await.body["total"],
        1
    );

    // The gate off: named logins are refused, named sessions end, the operator keeps its own.
    assert_eq!(
        send(&tl, "POST", "/api/v1/users/enable", &[t, CSRF], Some(json!({ "on": false }))).await.status,
        StatusCode::OK
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/auth/session", &[alice], None).await.body["signed_in"],
        false
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/auth/session", &[op], None).await.body["signed_in"],
        true
    );
    let r = send(
        &a,
        "POST",
        "/api/v1/auth/login",
        &[CSRF],
        Some(json!({ "password": "alice's pass 1", "username": "alice" })),
    )
    .await;
    assert_eq!((r.status, r.code()), (StatusCode::FORBIDDEN, "forbidden"));
}

/// A project as a browser sends it (the UI's shape).
fn proj(id: &str) -> Value {
    json!({
        "meta": { "id": id, "title": id, "updatedAt": 1.0, "cover": null, "count": 1 },
        "items": [ { "key": "k", "kind": "gen", "genId": "g-none" } ],
        "selected": "k", "rev": 0
    })
}

#[tokio::test]
async fn projects_are_per_user() {
    let f = fixture(true);
    let a = app(&f, true, "192.168.1.50");
    let tl = app(&f, false, "127.0.0.1");
    let token = format!("Bearer {}", f.state.token);
    let t = ("authorization", token.as_str());

    // The operator opens the gate and creates two accounts (the row test's setup).
    assert_eq!(
        send(&tl, "POST", "/api/v1/users/enable", &[t, CSRF], Some(json!({ "on": true }))).await.status,
        StatusCode::OK
    );
    for (name, pw) in [("alice", "alice's pass 1"), ("bob", "bob's pass 2")] {
        let r = send(
            &tl,
            "POST",
            "/api/v1/users",
            &[t, CSRF],
            Some(json!({ "name": name, "password": pw })),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    }
    let alice_cookie = named_sign_in(&a, "alice", "alice's pass 1").await;
    let alice = ("cookie", alice_cookie.as_str());
    let bob_cookie = named_sign_in(&a, "bob", "bob's pass 2").await;
    let bob = ("cookie", bob_cookie.as_str());
    let op_cookie = sign_in(&a).await;
    let op = ("cookie", op_cookie.as_str());

    // One shared project (the machine's list, as before accounts) and one per account.
    for (h, id) in [(t, "shared"), (alice, "alice-p"), (bob, "bob-p")] {
        let r = send(
            if h.0 == "authorization" { &tl } else { &a },
            "PUT",
            &format!("/api/v1/projects/{id}"),
            &[h, CSRF],
            Some(json!({ "project": proj(id), "known": [] })),
        )
        .await;
        assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    }

    // Each account lists exactly its own; the machine and the operator see all three.
    for (h, want) in [(alice, 1), (bob, 1), (op, 3)] {
        let g = send(&a, "GET", "/api/v1/projects", &[h], None).await;
        assert_eq!(g.body["projects"].as_array().unwrap().len(), want, "{h:?}");
    }
    let g = send(&a, "GET", "/api/v1/projects", &[alice], None).await;
    assert_eq!(g.body["projects"][0]["meta"]["id"], "alice-p");
    let g = send(&tl, "GET", "/api/v1/projects", &[t], None).await;
    assert_eq!(g.body["projects"].as_array().unwrap().len(), 3);

    // Shared, another's and absent are one and the same absence (no existence leak).
    for id in ["shared", "bob-p", "ghost"] {
        let r = send(&a, "GET", &format!("/api/v1/projects/{id}"), &[alice], None).await;
        assert_eq!((r.status, r.code()), (StatusCode::NOT_FOUND, "not_found"), "{id}");
    }
    assert_eq!(
        send(&a, "GET", "/api/v1/projects/alice-p", &[alice], None).await.status,
        StatusCode::OK
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/projects/shared", &[op], None).await.status,
        StatusCode::OK
    );

    // A named account writes and deletes nothing that is not its own; the ownership is
    // never in the request body to claim.
    for (m, pth) in [
        ("PUT", "/api/v1/projects/shared"),
        ("PUT", "/api/v1/projects/bob-p"),
        ("DELETE", "/api/v1/projects/shared"),
        ("DELETE", "/api/v1/projects/bob-p"),
    ] {
        let body = if m == "PUT" { Some(json!({ "project": proj("shared"), "known": [] })) } else { None };
        let r = send(&a, m, pth, &[alice, CSRF], body).await;
        assert_eq!((r.status, r.code()), (StatusCode::NOT_FOUND, "not_found"), "{m} {pth}");
    }
    // Its own it edits and deletes; the operator merges into it and the owner is kept.
    assert_eq!(
        send(&a, "PUT", "/api/v1/projects/alice-p", &[alice, CSRF], Some(json!({ "project": proj("alice-p"), "known": [] })))
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        send(&a, "PUT", "/api/v1/projects/alice-p", &[op, CSRF], Some(json!({ "project": proj("alice-p"), "known": [] })))
            .await
            .status,
        StatusCode::OK
    );
    let g = send(&a, "GET", "/api/v1/projects", &[alice], None).await;
    assert_eq!(g.body["projects"][0]["meta"]["id"], "alice-p", "the operator's merge keeps the owner");
    assert_eq!(
        send(&a, "DELETE", "/api/v1/projects/bob-p", &[bob, CSRF], None).await.status,
        StatusCode::OK
    );
    assert_eq!(
        send(&a, "GET", "/api/v1/projects/bob-p", &[bob], None).await.status,
        StatusCode::NOT_FOUND
    );

    // The first-launch import is namespaced: alice merges into her project and creates
    // her new one, and leaves the shared list and another's untouched.
    let r = send(
        &a,
        "POST",
        "/api/v1/projects/import",
        &[alice, CSRF],
        Some(json!({ "projects": [ proj("shared"), proj("bob-p"), proj("alice-new") ] })),
    )
    .await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    assert_eq!(r.body["changed"], 1, "{:?}", r.body);
    let g = send(&tl, "GET", "/api/v1/projects", &[t], None).await;
    let ids: Vec<&str> = g.body["projects"].as_array().unwrap().iter()
        .map(|x| x["meta"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert!(ids.contains(&"alice-new"));
    assert!(ids.contains(&"shared"));

    // Records: a named caller gets only the rows it owns — its items are a
    // client-controlled list of references; the operator gets all of them.
    for r in [fake_row("g-alice", Some("alice")), fake_row("g-shared", None)] {
        f.state.engine.store.insert_generation(&r).unwrap();
    }
    let recs = json!({
        "meta": { "id": "alice-rec", "title": "t", "updatedAt": 1.0, "cover": null, "count": 2 },
        "items": [
            { "key": "a", "kind": "gen", "genId": "g-alice" },
            { "key": "s", "kind": "gen", "genId": "g-shared" }
        ],
        "selected": "s", "rev": 0
    });
    assert_eq!(
        send(&a, "PUT", "/api/v1/projects/alice-rec", &[alice, CSRF], Some(json!({ "project": recs, "known": [] })))
            .await
            .status,
        StatusCode::OK
    );
    let r = send(&a, "GET", "/api/v1/projects/alice-rec/records", &[alice], None).await;
    assert_eq!(r.body["generations"].as_object().unwrap().len(), 1, "only her own row: {:?}", r.body);
    assert_eq!(r.body["generations"]["g-alice"]["id"], "g-alice");
    let r = send(&a, "GET", "/api/v1/projects/alice-rec/records", &[op], None).await;
    assert_eq!(r.body["generations"].as_object().unwrap().len(), 2, "the operator sees both");
    let r = send(&a, "GET", "/api/v1/projects/shared/records", &[alice], None).await;
    assert_eq!((r.status, r.code()), (StatusCode::NOT_FOUND, "not_found"));

    // The account's projects go with the account when the operator deletes it.
    let alice_id = send(&tl, "GET", "/api/v1/users", &[t], None)
        .await
        .body["users"]
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["name"] == "alice")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let r = send(&tl, "DELETE", &format!("/api/v1/users/{alice_id}"), &[t, CSRF], None).await;
    assert_eq!(r.body["deleted"], "alice");
    assert_eq!(r.body["projects_removed"], 3, "{:?}", r.body);
    let g = send(&tl, "GET", "/api/v1/projects", &[t], None).await;
    let ids: Vec<&str> = g
        .body["projects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["meta"]["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["shared"], "{ids:?}");
    assert_eq!(
        send(&a, "GET", "/api/v1/auth/session", &[alice], None).await.body["signed_in"],
        false
    );
}
