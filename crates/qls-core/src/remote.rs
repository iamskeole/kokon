//! Access from other devices: the browser UI on the local network (or through a VPN such as
//! Tailscale), behind a password. The rules live here so they can be tested without HTTP:
//!
//! - the password is stored only as an Argon2id hash (`remote-auth.json` in the data folder,
//!   never in `config.json`, which the API returns);
//! - a login opens a session (a random cookie value); `remote-sessions.json` keeps only the
//!   SHA-256 of each value, so browsers stay signed in across a restart (an app update, a
//!   reboot); changing the password ends every session;
//! - 5 wrong passwords in a row from one address lock it for 1 minute, doubling up to 15;
//!   behind a reverse proxy on the private network the address is what the proxy reports in
//!   `X-Forwarded-For`;
//! - a browser session may use only the endpoints the UI needs, and never one that takes a
//!   path on the server or changes what the computer runs (see [`browser_allowed`]).

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult, ErrorCode};
use crate::planner::GenerationRequest;

pub const MIN_PASSWORD_CHARS: usize = 8;
/// Long enough that a phone or laptop rarely asks again; a new password ends it sooner.
pub const SESSION_TTL: Duration = Duration::from_secs(90 * 24 * 3600);
pub const SESSION_COOKIE: &str = "kokon_session";
/// Wrong passwords in a row before an address waits.
pub const FREE_ATTEMPTS: u32 = 5;
const FIRST_LOCK: Duration = Duration::from_secs(60);
const MAX_LOCK: Duration = Duration::from_secs(15 * 60);
/// Sessions kept at most (oldest dropped): one per browser that logged in.
const MAX_SESSIONS: usize = 200;

#[derive(Serialize, Deserialize)]
struct AuthFile {
    password_hash: String,
}

/// `<data dir>/remote-auth.json`.
pub fn password_file(data_dir: &Path) -> PathBuf {
    data_dir.join("remote-auth.json")
}

/// The stored hash, if a password was set (and the file is readable and well-formed).
pub fn load_hash(file: &Path) -> Option<String> {
    let f: AuthFile = serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()?;
    PasswordHash::new(&f.password_hash).ok()?;
    Some(f.password_hash)
}

/// Hashes `password` and writes it to `file` (readable by the owner only on Unix).
pub fn set_password(file: &Path, password: &str) -> AppResult<String> {
    let hash = hash_password(password)?;
    let body = serde_json::to_string_pretty(&AuthFile {
        password_hash: hash.clone(),
    })
    .map_err(AppError::internal)?;
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, body).map_err(|e| AppError::io("save the password", &tmp, e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&tmp, file).map_err(|e| AppError::io("save the password", file, e))?;
    Ok(hash)
}

/// Argon2id of a password, refusing the short ones (the shared password and the named
/// accounts use the same minimum).
pub fn hash_password(password: &str) -> AppResult<String> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(AppError::invalid(format!(
            "Use at least {MIN_PASSWORD_CHARS} characters."
        )));
    }
    Ok(
        Argon2::default()
            .hash_password(password.as_bytes())
            .map_err(AppError::internal)?
            .to_string(),
    )
}

/// Removes the password (access from other devices then stays closed).
pub fn clear_password(file: &Path) -> AppResult<()> {
    match std::fs::remove_file(file) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(AppError::io("remove the password", file, e))
        }
        _ => Ok(()),
    }
}

/// Slow on purpose (Argon2): call it off the async runtime.
pub fn verify(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}

struct Failures {
    count: u32,
    locked_until: Option<Instant>,
}

/// Wrong passwords per address.
#[derive(Default)]
pub struct Limiter {
    by_ip: Mutex<HashMap<IpAddr, Failures>>,
}

impl Limiter {
    /// `Err(seconds)` while `ip` has to wait before trying again.
    pub fn check(&self, ip: IpAddr, now: Instant) -> Result<(), u64> {
        match self
            .by_ip
            .lock()
            .unwrap()
            .get(&ip)
            .and_then(|f| f.locked_until)
        {
            Some(t) if t > now => Err((t - now).as_secs().max(1)),
            _ => Ok(()),
        }
    }

    /// A wrong password; returns how long `ip` now waits (0 = it can try again at once).
    pub fn fail(&self, ip: IpAddr, now: Instant) -> u64 {
        let mut m = self.by_ip.lock().unwrap();
        if m.len() > 10_000 {
            m.retain(|_, f| f.locked_until.is_some_and(|t| t > now));
        }
        let f = m.entry(ip).or_insert(Failures {
            count: 0,
            locked_until: None,
        });
        f.count += 1;
        if f.count < FREE_ATTEMPTS {
            return 0;
        }
        let lock = FIRST_LOCK
            .saturating_mul(1 << (f.count - FREE_ATTEMPTS).min(10))
            .min(MAX_LOCK);
        f.locked_until = Some(now + lock);
        lock.as_secs()
    }

    pub fn succeed(&self, ip: IpAddr) {
        self.by_ip.lock().unwrap().remove(&ip);
    }
}

/// `<data dir>/remote-sessions.json`.
pub fn sessions_file(data_dir: &Path) -> PathBuf {
    data_dir.join("remote-sessions.json")
}

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

fn unix_secs(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// What `remote-sessions.json` holds: never a cookie value, only its SHA-256 (a copy of the file
/// signs no one in), and the SHA-256 of the password hash the sessions were opened under.
#[derive(Serialize, Deserialize, Default)]
struct SessionsFile {
    password: String,
    /// SHA-256 of the cookie value → who it is and when it expires (seconds since 1970).
    sessions: HashMap<String, SessionEntry>,
}

/// One stored session. The operator (the shared password) is a bare expiry — the exact shape
/// the file had before named accounts, so an existing file reads without migration. A named
/// account (multi-user) adds its name; an older service that met such a file would drop the
/// whole file (no sessions) rather than sign anyone in, which is the safe direction.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum SessionEntry {
    Op(u64),
    User { user: String, exp: u64 },
}

impl SessionEntry {
    fn exp(&self) -> u64 {
        match self {
            SessionEntry::Op(t) => *t,
            SessionEntry::User { exp, .. } => *exp,
        }
    }
}

/// Who a signed-in browser is (multi-user): the operator (the shared password) or a named
/// account (its name, the `users` table's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionOwner {
    Operator,
    Named(String),
}

/// Signed-in browsers. Wall-clock expiry, so it means the same after a restart.
#[derive(Default)]
pub struct Sessions {
    /// `None`: in memory only.
    file: Option<PathBuf>,
    state: Mutex<SessionsFile>,
}

impl Sessions {
    /// Loads the sessions kept in `file`, minus the expired ones; all of them end if the password
    /// changed while the service was off (`--set-password` from a terminal).
    pub fn open(file: PathBuf, password_hash: Option<&str>, now: SystemTime) -> Self {
        let password = password_hash.map(sha256_hex).unwrap_or_default();
        let loaded: Option<SessionsFile> = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok());
        let mut st = SessionsFile {
            password,
            sessions: HashMap::new(),
        };
        // A file that is unreadable, from another password or with nothing left goes.
        let mut changed = file.exists();
        if let Some(f) = loaded
            && f.password == st.password
            && password_hash.is_some()
        {
            let n = f.sessions.len();
            let t = unix_secs(now);
            st.sessions = f
                .sessions
                .into_iter()
                .filter(|(_, e)| e.exp() > t)
                .collect();
            if st.sessions.len() > MAX_SESSIONS {
                let mut by_exp: Vec<u64> = st.sessions.values().map(|e| e.exp()).collect();
                by_exp.sort_unstable();
                let keep_from = by_exp[by_exp.len() - MAX_SESSIONS];
                st.sessions.retain(|_, e| e.exp() >= keep_from);
            }
            changed = st.sessions.len() != n || st.sessions.is_empty();
        }
        let s = Self {
            file: Some(file),
            state: Mutex::new(st),
        };
        if changed {
            let st = s.state.lock().unwrap();
            if st.sessions.is_empty() {
                s.remove_file()
            } else {
                s.save(&st)
            }
        }
        s
    }

    /// Written whole to a temporary file, then renamed over the old one (readable by the owner
    /// only on Unix). A failure only costs a sign-in after the next restart.
    fn save(&self, st: &SessionsFile) {
        let Some(file) = &self.file else { return };
        let Ok(body) = serde_json::to_string(st) else {
            return;
        };
        let tmp = file.with_extension("json.tmp");
        let res = std::fs::write(&tmp, body).and_then(|_| {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
            }
            std::fs::rename(&tmp, file)
        });
        if let Err(e) = res {
            let _ = std::fs::remove_file(&tmp);
            tracing::warn!("cannot save browser sessions to {}: {e}", file.display());
        }
    }

    fn remove_file(&self) {
        if let Some(file) = &self.file
            && let Err(e) = std::fs::remove_file(file)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("cannot remove {}: {e}", file.display());
        }
    }

    pub fn create(&self, now: SystemTime) -> String {
        self.create_for(None, now)
    }

    /// Opens a session for `user` (the `users` table's name; `None` = the operator).
    pub fn create_for(&self, user: Option<&str>, now: SystemTime) -> String {
        let id = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let t = unix_secs(now);
        let entry = match user {
            Some(u) => SessionEntry::User {
                user: u.to_string(),
                exp: t + SESSION_TTL.as_secs(),
            },
            None => SessionEntry::Op(t + SESSION_TTL.as_secs()),
        };
        let mut st = self.state.lock().unwrap();
        st.sessions.retain(|_, e| e.exp() > t);
        if st.sessions.len() >= MAX_SESSIONS
            && let Some(oldest) = st
                .sessions
                .iter()
                .min_by_key(|(_, e)| e.exp())
                .map(|(k, _)| k.clone())
        {
            st.sessions.remove(&oldest);
        }
        st.sessions.insert(sha256_hex(&id), entry);
        self.save(&st);
        id
    }

    pub fn valid(&self, id: &str, now: SystemTime) -> bool {
        self.owner_of(id, now).is_some()
    }

    /// Who this session belongs to, or none (unknown or expired — the two must not be told
    /// apart: either way the browser signs in again).
    pub fn owner_of(&self, id: &str, now: SystemTime) -> Option<SessionOwner> {
        let t = unix_secs(now);
        match self.state.lock().unwrap().sessions.get(&sha256_hex(id)) {
            Some(SessionEntry::Op(exp)) if *exp > t => Some(SessionOwner::Operator),
            Some(SessionEntry::User { user, exp }) if *exp > t => Some(SessionOwner::Named(user.clone())),
            _ => None,
        }
    }

    pub fn remove(&self, id: &str) {
        let mut st = self.state.lock().unwrap();
        if st.sessions.remove(&sha256_hex(id)).is_some() {
            self.save(&st);
        }
    }

    /// Ends every named user's session (multi-user turned off); the operator keeps theirs.
    pub fn clear_named(&self) {
        let mut st = self.state.lock().unwrap();
        let n = st.sessions.len();
        st.sessions.retain(|_, e| matches!(e, SessionEntry::Op(_)));
        if st.sessions.len() != n {
            self.save(&st);
        }
    }

    /// Ends every session of one named user (their password changed or the account went);
    /// the other owners keep theirs.
    pub fn remove_user(&self, name: &str) {
        let mut st = self.state.lock().unwrap();
        let n = st.sessions.len();
        st.sessions
            .retain(|_, e| !matches!(e, SessionEntry::User { user, .. } if user == name));
        if st.sessions.len() != n {
            self.save(&st);
        }
    }

    /// The password changed or was removed (`password_hash`: the one now in use): every
    /// browser signs in again, after a restart too.
    pub fn clear(&self, password_hash: Option<&str>) {
        let mut st = self.state.lock().unwrap();
        st.sessions.clear();
        st.password = password_hash.map(sha256_hex).unwrap_or_default();
        self.remove_file();
    }
}

/// What a browser session may call, by method and path under `/api/v1`. Everything the UI
/// needs, minus what takes a path on the server (`/inputs/import`, `/models/scan`, exports,
/// `/loras` add, `/luts/import`, `/image-info`), the scripting endpoints that accept paths
/// (`/generate`, `/edit`...), `/shutdown` and the network settings (`/remote`). Jobs and plans
/// are also checked by [`stored_sources_only`]; config saves by `AppConfig::keep_computer_settings`.
#[allow(clippy::match_like_matches_macro)]
pub fn browser_allowed(method: &str, path: &str) -> bool {
    let seg: Vec<&str> = path.trim_matches('/').split('/').collect();
    match (method, seg.as_slice()) {
        (
            "GET",
            [
                "status" | "setup" | "models" | "upscalers" | "config" | "diagnostics" | "logs"
                | "jobs" | "events" | "gallery" | "loras" | "luts" | "projects",
            ],
        ) => true,
        ("GET", ["setup", "hardware" | "install" | "legacy-upscalers"]) => true,
        ("GET", ["enhancer", "defaults"]) => true,
        ("GET", ["jobs", _] | ["jobs", _, "wait"]) => true,
        ("GET", ["inputs", _] | ["inputs", _, "image" | "thumb" | "preview"]) => true,
        ("GET", ["gallery", _] | ["gallery", _, "image" | "thumb" | "preview" | "original"]) => {
            true
        }
        ("GET", ["projects", _, "records"]) => true,
        ("GET", ["luts", _, "preview"]) => true,
        ("GET" | "POST", ["matte"]) | ("POST", ["matte", "download" | "cancel"]) => true,
        ("GET" | "PUT" | "DELETE", ["projects", _]) => true,
        ("PUT", ["config"]) => true,
        ("POST", ["setup", "install" | "gpu"] | ["setup", "install", "cancel"]) => true,
        ("POST", ["models", "use" | "vae" | "dit" | "switch"]) => true,
        ("POST", ["upscalers", _, "download"]) => true,
        ("POST", ["backend", "start" | "stop" | "restart"]) => true,
        ("POST", ["plan" | "jobs" | "inputs" | "examples"] | ["projects", "import"]) => true,
        // The people list (the operator's Settings): the named accounts may not touch it
        // (the handler refuses them with a 403).
        ("GET", ["users"]) => true,
        ("POST", ["users"] | ["users", "enable"] | ["users", _, "password"]) => true,
        ("DELETE", ["users", _]) => true,
        ("POST", ["jobs", _, "cancel"]) => true,
        ("POST", ["private", "clear"]) => true,
        ("POST", ["luts", _, "apply"]) => true,
        ("POST", ["session", "heartbeat" | "release"]) => true,
        ("PATCH", ["gallery", _] | ["loras", _]) => true,
        ("DELETE", ["gallery", _] | ["loras", _] | ["luts", _] | ["setup", "legacy-upscalers"]) => {
            true
        }
        _ => false,
    }
}

/// A browser's job or plan may use only images already in Kokon (uploads and results), never
/// a path on the server, and saves to the gallery only.
pub fn stored_sources_only(req: &GenerationRequest, workspace: Option<&str>) -> AppResult<()> {
    let stored = |s: &str| s.starts_with("input:") || s.starts_with("gen:");
    if !req.images.iter().all(|s| stored(s)) || !req.mask.as_deref().is_none_or(stored) {
        return Err(AppError::new(
            ErrorCode::Forbidden,
            "From a browser, upload the image first.",
        ));
    }
    if req.output_path.is_some() || workspace.is_some_and(|w| !w.trim().is_empty()) {
        return Err(AppError::new(
            ErrorCode::Forbidden,
            "From a browser, results go to the gallery.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_is_stored_as_a_hash_only() {
        let dir = tempfile::tempdir().unwrap();
        let f = password_file(dir.path());
        assert!(load_hash(&f).is_none());
        assert!(
            set_password(&f, "short").is_err(),
            "fewer than 8 characters"
        );
        set_password(&f, "correct horse").unwrap();
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(!text.contains("correct horse"));
        let hash = load_hash(&f).unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify(&hash, "correct horse"));
        assert!(!verify(&hash, "correct horsE"));
        assert!(!verify("not a hash", "correct horse"));
        clear_password(&f).unwrap();
        assert!(load_hash(&f).is_none());
        clear_password(&f).unwrap();
    }

    #[test]
    fn a_broken_password_file_counts_as_no_password() {
        let dir = tempfile::tempdir().unwrap();
        let f = password_file(dir.path());
        std::fs::write(&f, r#"{"password_hash": "plain text"}"#).unwrap();
        assert!(load_hash(&f).is_none());
    }

    #[test]
    fn five_wrong_passwords_lock_the_address_for_longer_each_time() {
        let l = Limiter::default();
        let ip: IpAddr = "192.168.1.30".parse().unwrap();
        let other: IpAddr = "192.168.1.31".parse().unwrap();
        let t = Instant::now();
        for _ in 0..FREE_ATTEMPTS - 1 {
            assert_eq!(l.fail(ip, t), 0);
            assert!(l.check(ip, t).is_ok());
        }
        assert_eq!(l.fail(ip, t), 60);
        assert_eq!(l.check(ip, t + Duration::from_secs(10)), Err(50));
        assert!(l.check(other, t).is_ok(), "other devices are not locked");
        assert!(l.check(ip, t + Duration::from_secs(61)).is_ok());
        assert_eq!(l.fail(ip, t + Duration::from_secs(61)), 120);
        let mut wait = 0;
        for _ in 0..10 {
            wait = l.fail(ip, t);
        }
        assert_eq!(wait, 900, "at most 15 minutes");
        l.succeed(ip);
        assert!(l.check(ip, t).is_ok());
        assert_eq!(l.fail(ip, t), 0, "a login starts the count again");
    }

    #[test]
    fn sessions_expire_and_end_with_a_new_password() {
        let s = Sessions::default();
        let t = SystemTime::now();
        let a = s.create(t);
        let b = s.create(t);
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
        assert!(s.valid(&a, t) && s.valid(&b, t));
        assert!(!s.valid("nope", t));
        assert!(!s.valid(&a, t + SESSION_TTL + Duration::from_secs(1)));
        s.remove(&a);
        assert!(!s.valid(&a, t) && s.valid(&b, t));
        s.clear(None);
        assert!(!s.valid(&b, t));
    }

    #[test]
    fn sessions_survive_a_restart_as_hashes_only() {
        let dir = tempfile::tempdir().unwrap();
        let f = sessions_file(dir.path());
        let t = SystemTime::now();
        let s = Sessions::open(f.clone(), Some("hash-1"), t);
        let a = s.create(t);
        let b = s.create(t);
        s.remove(&b);
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(!text.contains(&a), "the cookie value is never written");
        assert!(!text.contains("hash-1"));
        drop(s);
        let s = Sessions::open(f.clone(), Some("hash-1"), t + Duration::from_secs(3600));
        assert!(s.valid(&a, t) && !s.valid(&b, t));
        // A password set while the service was off ends them, and so does none at all.
        let s = Sessions::open(f.clone(), Some("hash-2"), t);
        assert!(!s.valid(&a, t));
        assert!(!f.exists());
        let c = s.create(t);
        assert!(!Sessions::open(f.clone(), None, t).valid(&c, t));
    }

    #[test]
    fn expired_sessions_are_dropped_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let f = sessions_file(dir.path());
        let t = SystemTime::now();
        let s = Sessions::open(f.clone(), Some("h"), t);
        let old = s.create(t);
        let new = s.create(t + Duration::from_secs(86_400));
        let later = t + SESSION_TTL + Duration::from_secs(60);
        let s = Sessions::open(f.clone(), Some("h"), later);
        assert!(!s.valid(&old, later) && s.valid(&new, later));
        let kept: SessionsFile =
            serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(
            kept.sessions.len(),
            1,
            "the file is rewritten without the expired one"
        );
        let all_gone = Sessions::open(f.clone(), Some("h"), later + SESSION_TTL);
        assert!(!all_gone.valid(&new, later) && !f.exists());
    }

    #[test]
    fn clear_empties_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let f = sessions_file(dir.path());
        let t = SystemTime::now();
        let s = Sessions::open(f.clone(), Some("h"), t);
        let a = s.create(t);
        assert!(f.exists());
        s.clear(Some("new"));
        assert!(!f.exists() && !s.valid(&a, t));
        assert!(
            !Sessions::open(f.clone(), Some("h"), t).valid(&a, t),
            "not back after a restart"
        );
        // New sign-ins are kept under the new password.
        let b = s.create(t);
        assert!(Sessions::open(f.clone(), Some("new"), t).valid(&b, t));
        std::fs::write(&f, "not json").unwrap();
        assert!(
            !Sessions::open(f.clone(), Some("new"), t).valid(&b, t),
            "a broken file counts as none"
        );
        assert!(!f.exists());
    }

    #[test]
    fn sessions_carry_their_owner_and_read_old_files()
    {
        let dir = tempfile::tempdir().unwrap();
        let f = sessions_file(dir.path());
        let t = SystemTime::now();
        let s = Sessions::open(f.clone(), Some("h"), t);
        let op = s.create(t);
        let anna = s.create_for(Some("anna"), t);
        assert_eq!(s.owner_of(&op, t), Some(SessionOwner::Operator));
        assert_eq!(s.owner_of(&anna, t), Some(SessionOwner::Named("anna".into())));
        assert_eq!(s.owner_of("nope", t), None);
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(text.contains("anna"));
        assert!(!text.contains(&op), "the cookie value is still never written");
        drop(s);
        // Restart: both owners come back. A file written before named accounts (bare numbers)
        // reads as the operator's.
        let s = Sessions::open(f.clone(), Some("h"), t + Duration::from_secs(3600));
        assert_eq!(s.owner_of(&op, t), Some(SessionOwner::Operator));
        assert_eq!(s.owner_of(&anna, t + Duration::from_secs(3600)), Some(SessionOwner::Named("anna".into())));
        let old = (0..3).map(|i| (sha256_hex(&format!("{i}")), SessionEntry::Op(1234))).collect();
        std::fs::write(
            &f,
            serde_json::to_string(&SessionsFile { password: sha256_hex("h"), sessions: old }).unwrap(),
        )
        .unwrap();
        let s = Sessions::open(f.clone(), Some("h"), SystemTime::UNIX_EPOCH);
        assert_eq!(s.owner_of("0", SystemTime::UNIX_EPOCH), Some(SessionOwner::Operator), "a pre-accounts file is the operator's");
        // A new password ends every session, owner or not (D-MU9).
        let s = Sessions::open(f.clone(), Some("changed"), t);
        assert_eq!(s.owner_of(&op, t), None);
        assert_eq!(s.owner_of(&anna, t), None);
    }

    #[test]
    fn at_most_max_sessions_are_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let f = sessions_file(dir.path());
        let t = SystemTime::now();
        let sessions = (0..MAX_SESSIONS as u64 + 5)
            .map(|i| (format!("{i}"), SessionEntry::Op(unix_secs(t) + 1000 + i)))
            .collect();
        std::fs::write(
            &f,
            serde_json::to_string(&SessionsFile {
                password: sha256_hex("h"),
                sessions,
            })
            .unwrap(),
        )
        .unwrap();
        let s = Sessions::open(f.clone(), Some("h"), t);
        let st = s.state.lock().unwrap();
        assert_eq!(st.sessions.len(), MAX_SESSIONS);
        assert!(!st.sessions.contains_key("0"), "the oldest go first");
    }

    #[test]
    fn browsers_cannot_reach_paths_on_the_server() {
        for (m, p) in [
            ("GET", "/status"),
            ("GET", "/gallery/abc/image"),
            ("GET", "/gallery/abc/preview"),
            ("GET", "/inputs/abc/preview"),
            ("GET", "/projects/abc/records"),
            ("GET", "/events"),
            ("POST", "/jobs"),
            ("POST", "/models/switch"),
            ("POST", "/inputs"),
            ("PUT", "/config"),
            ("POST", "/session/heartbeat"),
            ("DELETE", "/gallery/abc"),
            ("GET", "/projects"),
            ("PUT", "/projects/abc"),
            ("POST", "/projects/import"),
        ] {
            assert!(browser_allowed(m, p), "{m} {p}");
        }
        for (m, p) in [
            ("POST", "/inputs/import"),
            ("POST", "/models/scan"),
            ("POST", "/models/inspect"),
            ("POST", "/models/user"),
            ("POST", "/gallery/abc/export"),
            ("POST", "/image-info"),
            ("POST", "/loras"),
            ("POST", "/luts/import"),
            ("POST", "/generate"),
            ("POST", "/edit"),
            ("POST", "/shutdown"),
            ("GET", "/remote"),
            ("POST", "/remote"),
            ("POST", "/remote/password"),
            ("DELETE", "/jobs"),
            ("GET", "/gallery/a/b/c"),
            ("POST", "/gallery/abc/preview"),
            ("PUT", "/projects/abc/records"),
        ] {
            assert!(!browser_allowed(m, p), "{m} {p}");
        }
    }

    #[test]
    fn browser_jobs_use_stored_images_only() {
        let ok = GenerationRequest {
            images: vec!["input:a".into(), "gen:b".into()],
            mask: Some("input:m".into()),
            ..Default::default()
        };
        assert!(stored_sources_only(&ok, None).is_ok());
        let path = GenerationRequest {
            images: vec!["/etc/passwd".into()],
            ..Default::default()
        };
        assert_eq!(
            stored_sources_only(&path, None).unwrap_err().code,
            ErrorCode::Forbidden
        );
        let mask = GenerationRequest {
            mask: Some("C:\\secret.png".into()),
            ..Default::default()
        };
        assert!(stored_sources_only(&mask, None).is_err());
        let out = GenerationRequest {
            output_path: Some("/tmp/x.png".into()),
            ..Default::default()
        };
        assert!(stored_sources_only(&out, None).is_err());
        assert!(stored_sources_only(&GenerationRequest::default(), Some("/home")).is_err());
    }
}
