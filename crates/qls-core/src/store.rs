//! Local persistence: SQLite gallery of generations, content-addressed input images,
//! and the metadata written into PNG files. Nothing leaves the machine.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::RefineMode;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::geometry::{Expansion, Size};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReferenceRecord {
    pub index: usize,
    /// Content-addressed copy of the reference (always available for reopening).
    pub input_id: String,
    #[serde(default)]
    pub original_path: Option<String>,
    pub width: u32,
    pub height: u32,
}

/// How the pixels were produced beyond a single diffusion pass (Details panel).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct PipelineRecord {
    /// First pass of a two-pass job (the refine ran at `working_*`).
    pub base: Option<Size>,
    pub refine: Option<RefineMode>,
    pub refine_steps: Option<u32>,
    pub refine_denoise: Option<f32>,
    /// Upscaler between the passes ("RealESRGAN_x4plus" or "Lanczos").
    pub hires_upscaler: Option<String>,
    /// EasyCache threshold used for sampling.
    pub cache_threshold: Option<f32>,
    /// How the result was enlarged to its final size, when it was ("RealESRGAN_x4plus" or "Lanczos").
    pub enlarged_with: Option<String>,
    /// Size the content had before the final enlargement.
    pub enlarged_from: Option<Size>,
    /// The diffusion output before that enlargement (PNG in the store's `originals`), kept
    /// so the result can be compared with it or saved without the upscaler.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GenerationRecord {
    pub id: String,
    pub created_at: String,
    pub mode: String,
    /// The prompt the image was made from (the prompt enhancer's when it ran).
    pub prompt: String,
    /// The user's own prompt, when the prompt enhancer rewrote it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_prompt: Option<String>,
    /// LoRAs the image was made with.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loras: Vec<crate::lora::AppliedLora>,
    pub negative_prompt: String,
    pub seed: i64,
    pub width: u32,
    pub height: u32,
    pub working_width: u32,
    pub working_height: u32,
    pub model: String,
    /// In external mode: the gateway tenant the job was sent to (the configured model id,
    /// e.g. "kokon-flux-image-3-klein"). The engine's own reported name above can be
    /// ambiguous - different tenants may serve a model file with the same name - so this
    /// is the identity the picker label and the details panel resolve to. Absent in stock
    /// mode (and on older records), where the engine name is the only one there is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_model: Option<String>,
    pub sampler: String,
    pub scheduler: Option<String>,
    pub steps: u32,
    pub cfg: f32,
    pub strength: Option<f32>,
    pub quality: String,
    pub resolution: String,
    pub references: Vec<ReferenceRecord>,
    pub mask_input_id: Option<String>,
    pub outpaint: Option<Expansion>,
    pub file_path: String,
    #[serde(default)]
    pub extra_paths: Vec<String>,
    pub favorite: bool,
    pub parent_id: Option<String>,
    pub duration_ms: u64,
    pub notes: Vec<String>,
    pub app_version: String,
    pub backend_version: Option<String>,
    pub client: Option<String>,
    /// Full original request, for exact "reopen".
    #[serde(default)]
    pub request: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<PipelineRecord>,
    /// Left out of the gallery list (a step of the layers lab: only the image the user saves from
    /// it is shown). The project still opens it by id.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// The owner (multi-user): the name from the `users` table; absent = the operator and the
    /// rows from before accounts existed. The database's `user` column is the source of
    /// ownership; this copy is for display and PNG provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// The IPTC Digital Source Type when it is not the one of its mode (provenance.rs): an upscale
    /// or a LUT of an imported image that was already AI-made stays marked as AI-made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
}

/// A named account (multi-user): `name` is the identity — what a browser signs in with and the
/// folder the user's results are kept in — so it is unique (case-insensitively) and
/// filesystem-safe; the password lives only as an Argon2id hash.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserRecord {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

/// A user name: 1–20 letters, digits or ` .-_` (a browser signs in with it; the gallery keeps the
/// user's results in a folder of that name, so it must stay filesystem-safe).
pub fn validate_user_name(name: &str) -> AppResult<String> {
    let t = name.trim();
    let ok = (1..=20).contains(&t.chars().count())
        && t.chars().any(|c| c.is_ascii_alphanumeric())
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_'));
    if !ok {
        return Err(AppError::invalid(
            "The name is 1 to 20 letters, digits or space, dot, dash or underscore, with at least one letter or digit.",
        ));
    }
    Ok(t.to_string())
}

impl GenerationRecord {
    /// AUTOMATIC1111-style "parameters" text understood by many image tools.
    pub fn parameters_text(&self) -> String {
        if self.mode == "lut" {
            let lut = &self.request["lut"];
            let pct = (lut["intensity"].as_f64().unwrap_or(1.0) * 100.0).round();
            return format!("Colour LUT: {}, Intensity: {pct}%, Size: {}x{}, Version: Kokon {}", lut["name"].as_str().unwrap_or("?"), self.width, self.height, self.app_version);
        }
        let mut s = self.prompt.clone();
        if !self.negative_prompt.is_empty() {
            s.push_str(&format!("\nNegative prompt: {}", self.negative_prompt));
        }
        let model_line = match &self.external_model {
            Some(e) if !e.trim().is_empty() => format!("{} ({e})", self.model),
            _ => self.model.clone(),
        };
        s.push_str(&format!(
            "\nSteps: {}, Sampler: {}, Schedule type: {}, CFG scale: {}, Seed: {}, Size: {}x{}, Model: {}",
            self.steps,
            self.sampler,
            self.scheduler.as_deref().unwrap_or("default"),
            self.cfg,
            self.seed,
            self.width,
            self.height,
            model_line
        ));
        if let Some(st) = self.strength {
            s.push_str(&format!(", Denoising strength: {st}"));
        }
        if !self.loras.is_empty() {
            let list: Vec<String> = self.loras.iter().map(|l| format!("{} ({})", l.name.replace([',', ':'], " "), l.weight)).collect();
            s.push_str(&format!(", LoRA: {}", list.join(" + ")));
        }
        if let Some(p) = &self.pipeline
            && let Some(base) = p.base
        {
            let factor = self.working_width as f64 / base.width.max(1) as f64;
            s.push_str(&format!(
                ", Hires upscale: {factor:.2}, Hires steps: {}, Hires upscaler: {}",
                p.refine_steps.unwrap_or(0),
                p.hires_upscaler.as_deref().unwrap_or("Lanczos")
            ));
            if let Some(d) = p.refine_denoise {
                s.push_str(&format!(", Hires denoising strength: {d}"));
            }
        }
        s.push_str(&format!(", Mode: {}, Version: Kokon {}", self.mode, self.app_version));
        s
    }

    pub fn png_text(&self) -> Vec<(&'static str, String)> {
        let mut slim = self.clone();
        slim.request = serde_json::Value::Null;
        if let Some(p) = slim.pipeline.as_mut() {
            p.original_path = None;
        }
        vec![
            ("parameters", self.parameters_text()),
            ("qls:generation", serde_json::to_string(&slim).unwrap_or_default()),
            ("Software", format!("Kokon {}", self.app_version)),
        ]
    }

    pub fn from_png_text(text: &[(String, String)]) -> Option<Self> {
        text.iter().find(|(k, _)| k == "qls:generation").and_then(|(_, v)| serde_json::from_str(v).ok())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputRecord {
    pub id: String,
    pub path: String,
    pub original_name: Option<String>,
    pub original_path: Option<String>,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub has_alpha: bool,
    pub created_at: String,
    /// HEIC with an ISO 21496-1 HDR gain map.
    #[serde(default)]
    pub hdr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub favorites: bool,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
    /// "recent" (default) or "oldest".
    #[serde(default)]
    pub sort: Option<String>,
    /// Whose rows (the operator's filter, D-MU5): "all" (default) or "me" (the ownerless
    /// rows) or a user's name. A named account's own list ignores it (its rows only).
    #[serde(default)]
    pub user: Option<String>,
}

pub struct Store {
    conn: Mutex<Connection>,
    pub data_dir: PathBuf,
}

/// Which rows the caller may see (multi-user). `All` is the stock query (the token, the
/// operator, and single-user mode): the ownership column plays no part, so the result is
/// exactly what it was before accounts existed.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum OwnerFilter {
    #[default]
    All,
    /// Only the rows with no owner (the operator's and the pre-account ones).
    Own,
    /// Only this user's rows (the `users` table's name).
    User(String),
}

fn db_err(e: rusqlite::Error) -> AppError {
    AppError::new(ErrorCode::Internal, "The local gallery database reported an error.").detail(e.to_string())
}

pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    // Time-ordered prefix keeps file listings chronological.
    format!("{}{}", chrono::Utc::now().format("%Y%m%d%H%M%S"), &uuid::Uuid::new_v4().simple().to_string()[..8])
}

impl Store {
    pub fn open(data_dir: &Path) -> AppResult<Self> {
        std::fs::create_dir_all(data_dir.join("inputs")).map_err(|e| AppError::io("create the data folder", data_dir, e))?;
        std::fs::create_dir_all(data_dir.join("thumbs")).map_err(|e| AppError::io("create the data folder", data_dir, e))?;
        let conn = Connection::open(data_dir.join("gallery.db")).map_err(db_err)?;
        Self::init(conn, data_dir)
    }

    pub fn open_in_memory(data_dir: &Path) -> AppResult<Self> {
        Self::init(Connection::open_in_memory().map_err(db_err)?, data_dir)
    }

    fn init(conn: Connection, data_dir: &Path) -> AppResult<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;
             CREATE TABLE IF NOT EXISTS generations (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                prompt TEXT NOT NULL,
                mode TEXT NOT NULL,
                favorite INTEGER NOT NULL DEFAULT 0,
                file_path TEXT NOT NULL,
                record TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS gen_created ON generations(created_at);
             CREATE INDEX IF NOT EXISTS gen_fav ON generations(favorite);
             CREATE TABLE IF NOT EXISTS inputs (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                record TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL
             );
             CREATE UNIQUE INDEX IF NOT EXISTS users_name ON users(lower(name));",
        )
        .map_err(db_err)?;
        // 0.12: `hidden` (layers lab steps). Added when missing; an older app ignores the column
        // and its inserts get the default.
        let has_hidden = conn
            .prepare("SELECT 1 FROM pragma_table_info('generations') WHERE name = 'hidden'")
            .and_then(|mut st| st.exists([]))
            .map_err(db_err)?;
        if !has_hidden {
            conn.execute_batch("ALTER TABLE generations ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;").map_err(db_err)?;
        }
        // Multi-user: the row's owner (the `users` table's name); NULL = the operator and the
        // rows made before accounts existed. Older apps ignore the column.
        let has_user = conn
            .prepare("SELECT 1 FROM pragma_table_info('generations') WHERE name = 'user'")
            .and_then(|mut st| st.exists([]))
            .map_err(db_err)?;
        if !has_user {
            conn.execute_batch("ALTER TABLE generations ADD COLUMN user TEXT;").map_err(db_err)?;
        }
        conn.pragma_update(None, "user_version", 2).map_err(db_err)?;
        Ok(Self { conn: Mutex::new(conn), data_dir: data_dir.to_path_buf() })
    }

    /// Forgets every record (used for the temporary private-mode store).
    pub fn clear(&self) -> AppResult<()> {
        self.conn.lock().unwrap().execute_batch("DELETE FROM generations; DELETE FROM inputs;").map_err(db_err)
    }

    pub fn thumb_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("thumbs").join(format!("{id}.jpg"))
    }

    /// Where the browser's screen-sized copy of result `id` is kept (see [`preview`]).
    pub fn preview_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("thumbs").join(format!("{id}.preview"))
    }

    /// The bundled example projects' images ([`crate::examples`]).
    pub fn examples_dir(&self) -> PathBuf {
        self.data_dir.join("examples")
    }

    /// Where a result's pre-enlargement image is kept ([`PipelineRecord::original_path`]).
    pub fn original_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("originals").join(format!("{id}.png"))
    }

    pub fn insert_generation(&self, r: &GenerationRecord) -> AppResult<()> {
        let json = serde_json::to_string(r).map_err(AppError::internal)?;
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT OR REPLACE INTO generations (id, created_at, prompt, mode, favorite, file_path, record, hidden, user) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![r.id, r.created_at, r.prompt, r.mode, r.favorite as i32, r.file_path, json, r.hidden as i32, r.user],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn get_generation(&self, id: &str) -> AppResult<GenerationRecord> {
        let Some((json, fav, user)) = self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT record, favorite, user FROM generations WHERE id = ?1",
                [id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?, r.get::<_, Option<String>>(2)?)),
            )
            .optional()
            .map_err(db_err)?
        else {
            return Err(AppError::new(ErrorCode::NotFound, "That image is no longer in the gallery."));
        };
        let mut rec: GenerationRecord = serde_json::from_str(&json).map_err(AppError::internal)?;
        rec.favorite = fav != 0;
        // The column is the source of ownership (the record JSON is a copy).
        rec.user = user;
        Ok(rec)
    }

    /// The seed of the result saved at `file_path`, if the gallery has one there (exact path).
    /// For JPEG and WebP results, which carry no Kokon metadata in the file.
    pub fn seed_of_file(&self, file_path: &str) -> AppResult<Option<i64>> {
        let json: Option<String> = self
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT record FROM generations WHERE file_path = ?1 LIMIT 1", [file_path], |r| r.get(0))
            .optional()
            .map_err(db_err)?;
        Ok(json.and_then(|j| serde_json::from_str::<GenerationRecord>(&j).ok()).map(|r| r.seed))
    }

    pub fn list_generations(&self, q: &ListQuery, owner: &OwnerFilter) -> AppResult<(Vec<GenerationRecord>, u64)> {
        let limit = q.limit.unwrap_or(60).clamp(1, 500);
        let offset = q.offset.unwrap_or(0);
        let order = if q.sort.as_deref() == Some("oldest") { "ASC" } else { "DESC" };
        let pattern = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(|s| {
            let esc = s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_");
            format!("%{esc}%")
        });
        // `All` adds no ownership clause at all (the stock query); `Own` only the ownerless
        // rows; `User` that user's.
        let (owner_sql, owner_param) = match owner {
            OwnerFilter::All => (String::new(), None),
            OwnerFilter::Own => (String::from(" AND user IS NULL"), None),
            OwnerFilter::User(n) => (String::from(" AND user = ?3"), Some(n.clone())),
        };
        let where_ = format!(
            "WHERE (?1 IS NULL OR prompt LIKE ?1 ESCAPE '\\' OR mode LIKE ?1 ESCAPE '\\') AND (?2 = 0 OR favorite = 1) AND hidden = 0{owner_sql}"
        );
        // The parameters shift when the ownership clause takes ?3: build a list in order.
        // The COUNT query has no LIMIT/OFFSET, so it gets the same list without them.
        let params = |with_limits: bool| {
            let mut v: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(pattern.clone()), Box::new(q.favorites as i32)];
            if let Some(o) = &owner_param {
                v.push(Box::new(o.clone()));
            }
            if with_limits {
                v.push(Box::new(limit));
                v.push(Box::new(offset));
            }
            v
        };
        let n = if owner_param.is_some() { 3 } else { 2 };
        let conn = self.conn.lock().unwrap();
        let total: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM generations {where_}"),
                rusqlite::params_from_iter(params(false)),
                |r| r.get(0),
            )
            .map_err(db_err)?;
        let mut stmt = conn
            .prepare(&format!(
                "SELECT record, favorite, user FROM generations {where_} ORDER BY created_at {order}, id {order} LIMIT ?{} OFFSET ?{}",
                n + 1,
                n + 2
            ))
            .map_err(db_err)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params(true)), |r| Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?, r.get::<_, Option<String>>(2)?)))
            .map_err(db_err)?;
        let mut out = Vec::new();
        for row in rows {
            let (json, fav, user) = row.map_err(db_err)?;
            if let Ok(mut rec) = serde_json::from_str::<GenerationRecord>(&json) {
                rec.favorite = fav != 0;
                rec.user = user;
                out.push(rec);
            }
        }
        Ok((out, total.max(0) as u64))
    }

    /// Shows or hides result `id` in the gallery list (see [`GenerationRecord::hidden`]).
    pub fn set_hidden(&self, id: &str, hidden: bool) -> AppResult<()> {
        let mut rec = self.get_generation(id)?;
        rec.hidden = hidden;
        let json = serde_json::to_string(&rec).map_err(AppError::internal)?;
        self.conn.lock().unwrap().execute("UPDATE generations SET hidden = ?2, record = ?3 WHERE id = ?1", params![id, hidden as i32, json]).map_err(db_err)?;
        Ok(())
    }

    pub fn set_favorite(&self, id: &str, fav: bool) -> AppResult<()> {
        let n = self.conn.lock().unwrap().execute("UPDATE generations SET favorite = ?2 WHERE id = ?1", params![id, fav as i32]).map_err(db_err)?;
        if n == 0 {
            return Err(AppError::new(ErrorCode::NotFound, "That image is no longer in the gallery."));
        }
        Ok(())
    }

    /// Removes the record, its image file (only if inside the app output folder, or an
    /// example's), thumbnail and pre-upscale original.
    pub fn delete_generation(&self, id: &str, output_root: &Path) -> AppResult<()> {
        let rec = self.get_generation(id)?;
        self.conn.lock().unwrap().execute("DELETE FROM generations WHERE id = ?1", [id]).map_err(db_err)?;
        let f = PathBuf::from(&rec.file_path);
        if crate::paths::is_within(&f, &[output_root.to_path_buf(), self.examples_dir()]) {
            let _ = std::fs::remove_file(&f);
        }
        let _ = std::fs::remove_file(self.thumb_path(id));
        let preview = self.preview_path(id);
        let _ = std::fs::remove_file(preview.with_extension("preview.jpg"));
        let _ = std::fs::remove_file(preview.with_extension("preview.png"));
        let _ = std::fs::remove_file(self.original_path(id));
        Ok(())
    }

    // ---- inputs -----------------------------------------------------------------------

    /// Stores input bytes content-addressed (deduplicated). Original bytes are kept so
    /// EXIF/ICC survive for reopening.
    pub fn put_input(&self, bytes: &[u8], original_name: Option<&str>, original_path: Option<&str>) -> AppResult<InputRecord> {
        let img = crate::imaging::decode_image(bytes)?;
        let hash = hex::encode(Sha256::digest(bytes));
        let id = hash[..24].to_string();
        if let Ok(existing) = self.get_input(&id)
            && Path::new(&existing.path).is_file()
        {
            return Ok(existing);
        }
        let ext = match img.format {
            "jpeg" => "jpg",
            f => f,
        };
        let path = self.data_dir.join("inputs").join(format!("{id}.{ext}"));
        std::fs::write(&path, bytes).map_err(|e| AppError::io("store the image", &path, e))?;
        let thumb = crate::imaging::thumbnail(&img.pixels, 384);
        if let Ok(j) = crate::imaging::encode_jpeg(&thumb, 85) {
            let _ = std::fs::write(self.data_dir.join("inputs").join(format!("{id}.thumb.jpg")), j);
        }
        if img.format == "heic" {
            // The UI's WebView cannot show HEIC: keep a full-size JPEG to display instead.
            let j = crate::imaging::encode_jpeg(&img.pixels, 92)?;
            std::fs::write(self.input_display_path(&id), j).map_err(|e| AppError::io("store the image", &path, e))?;
        }
        let rec = InputRecord {
            id: id.clone(),
            path: path.to_string_lossy().into_owned(),
            original_name: original_name.map(str::to_owned),
            original_path: original_path.map(str::to_owned),
            width: img.pixels.width(),
            height: img.pixels.height(),
            format: img.format.to_string(),
            has_alpha: img.has_transparency,
            created_at: now_rfc3339(),
            hdr: img.format == "heic" && crate::heif::gain_map(bytes).is_some(),
        };
        let json = serde_json::to_string(&rec).map_err(AppError::internal)?;
        self.conn
            .lock()
            .unwrap()
            .execute("INSERT OR REPLACE INTO inputs (id, created_at, record) VALUES (?1,?2,?3)", params![id, rec.created_at, json])
            .map_err(db_err)?;
        Ok(rec)
    }

    pub fn get_input(&self, id: &str) -> AppResult<InputRecord> {
        if !id.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AppError::new(ErrorCode::NotFound, "Unknown input image."));
        }
        let json: Option<String> = self
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT record FROM inputs WHERE id = ?1", [id], |r| r.get(0))
            .optional()
            .map_err(db_err)?;
        let json = json.ok_or_else(|| AppError::new(ErrorCode::NotFound, "Unknown input image."))?;
        let mut rec: InputRecord = serde_json::from_str(&json).map_err(AppError::internal)?;
        // Records keep absolute paths; after the data folder moved (app renamed, user moved
        // it) the file is found by name in this store's inputs folder.
        if !Path::new(&rec.path).is_file()
            && let Some(name) = Path::new(&rec.path).file_name()
        {
            let here = self.data_dir.join("inputs").join(name);
            if here.is_file() {
                rec.path = here.to_string_lossy().into_owned();
            }
        }
        Ok(rec)
    }

    pub fn input_thumb_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("inputs").join(format!("{id}.thumb.jpg"))
    }

    /// Where the browser's screen-sized copy of input `id` is kept (see [`preview`]).
    pub fn input_preview_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("inputs").join(format!("{id}.preview"))
    }

    /// Full-size JPEG shown in the UI for inputs the WebView cannot display (HEIC).
    pub fn input_display_path(&self, id: &str) -> PathBuf {
        self.data_dir.join("inputs").join(format!("{id}.display.jpg"))
    }

    // ---- users (multi-user) ------------------------------------------------------------

    /// Creates a named account. The password is stored only as an Argon2id hash; the name is
    /// unique case-insensitively (the database enforces it on `lower(name)`).
    pub fn create_user(&self, name: &str, password: &str) -> AppResult<UserRecord> {
        let name = validate_user_name(name)?;
        let hash = crate::remote::hash_password(password)?;
        let id = format!("user_{}", &uuid::Uuid::new_v4().simple().to_string()[..12]);
        let created_at = now_rfc3339();
        self.conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO users (id, name, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
                params![id, name, hash, created_at],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(f, _) if f.code == rusqlite::ErrorCode::ConstraintViolation => {
                    AppError::new(ErrorCode::InvalidInput, "A user with that name already exists.")
                }
                e => db_err(e),
            })?;
        Ok(UserRecord { id, name, created_at })
    }

    pub fn list_users(&self) -> AppResult<Vec<UserRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, name, created_at FROM users ORDER BY lower(name)")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| Ok(UserRecord { id: r.get(0)?, name: r.get(1)?, created_at: r.get(2)? }))
            .map_err(db_err)?;
        Ok(rows.collect::<Result<Vec<_>, _>>().map_err(db_err)?)
    }

    pub fn user_by_name(&self, name: &str) -> AppResult<Option<UserRecord>> {
        let r: Option<(String, String, String)> = self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id, name, created_at FROM users WHERE lower(name) = lower(?1)",
                [name.trim()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(db_err)?;
        Ok(r.map(|(id, name, created_at)| UserRecord { id, name, created_at }))
    }

    pub fn user_by_id(&self, id: &str) -> AppResult<Option<UserRecord>> {
        let r: Option<(String, String, String)> = self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT id, name, created_at FROM users WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(db_err)?;
        Ok(r.map(|(id, name, created_at)| UserRecord { id, name, created_at }))
    }

    /// Verifies a password against the named user. Slow (Argon2id): call it off the async
    /// runtime. The service tells a missing user from a wrong password with [`user_by_name`].
    pub fn verify_user_password(&self, name: &str, password: &str) -> bool {
        let hash: Option<String> = self
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT password_hash FROM users WHERE lower(name) = lower(?1)", [name.trim()], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        hash.is_some_and(|h| crate::remote::verify(&h, password))
    }

    /// Replaces the user's password hash (their sessions end; the caller does that).
    pub fn set_user_password(&self, id: &str, password: &str) -> AppResult<()> {
        let hash = crate::remote::hash_password(password)?;
        let n = self
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE users SET password_hash = ?2 WHERE id = ?1", params![id, hash])
            .map_err(db_err)?;
        if n == 0 {
            return Err(AppError::new(ErrorCode::NotFound, "Unknown user."));
        }
        Ok(())
    }

    /// Deletes the account row (the caller removes the user's gallery rows first,
    /// [`delete_user_rows`], and ends their sessions).
    pub fn delete_user(&self, id: &str) -> AppResult<UserRecord> {
        let u = self.user_by_id(id)?.ok_or_else(|| AppError::new(ErrorCode::NotFound, "Unknown user."))?;
        self.conn
            .lock()
            .unwrap()
            .execute("DELETE FROM users WHERE id = ?1", [id])
            .map_err(db_err)?;
        Ok(u)
    }

    /// Deletes every gallery row owned by `user` (including the hidden ones), with their files
    /// when they are inside `output_root` (as [`delete_generation`]).
    pub fn delete_user_rows(&self, user: &str, output_root: &Path) -> AppResult<usize> {
        let mut removed = 0;
        loop {
            let ids: Vec<String> = {
                let conn = self.conn.lock().unwrap();
                let mut stmt = conn
                    .prepare("SELECT id FROM generations WHERE user = ?1 LIMIT 500")
                    .map_err(db_err)?;
                stmt.query_map([user], |r| r.get(0))
                    .map_err(db_err)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(db_err)?
            };
            if ids.is_empty() {
                break;
            }
            let n = ids.len();
            for id in ids {
                self.delete_generation(&id, output_root)?;
            }
            removed += n;
        }
        Ok(removed)
    }
}

/// Long side of the screen-sized copy a browser shows on the stage. A phone's stage is about
/// 1170 device px wide (iPhone: 390 CSS px x 3). Measured 10-24 on 3 images: 140-370 kB as JPEG
/// q85 against 6-7 MB for a 2048x2048 PNG, about 40 ms to make.
pub const PREVIEW_SIDE: u32 = 1280;

/// The screen-sized copy of the image at `src` (at most [`PREVIEW_SIDE`] on its long side; a
/// smaller image keeps its size), made on first request and kept at `stem` + ".jpg", or + ".png"
/// for an image with transparency, which JPEG would lose. Returns the file and its type.
pub fn preview(src: &Path, stem: &Path) -> AppResult<(PathBuf, &'static str)> {
    let jpg = stem.with_extension("preview.jpg");
    let png = stem.with_extension("preview.png");
    if jpg.is_file() {
        return Ok((jpg, "image/jpeg"));
    }
    if png.is_file() {
        return Ok((png, "image/png"));
    }
    let bytes = std::fs::read(src).map_err(|e| AppError::io("read the image", src, e))?;
    let img = crate::imaging::decode_image(&bytes)?;
    let small = crate::imaging::thumbnail(&img.pixels, PREVIEW_SIDE);
    let (out, data, ct) = if img.has_transparency {
        (png, crate::imaging::encode_png(&small, &[], None, true)?, "image/png")
    } else {
        (jpg, crate::imaging::encode_jpeg(&small, 85)?, "image/jpeg")
    };
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Two browsers may ask at once: each writes its own file, the rename is atomic.
    let tmp = out.with_extension(format!("{}.part", uuid::Uuid::new_v4().simple()));
    std::fs::write(&tmp, &data).map_err(|e| AppError::io("save the preview", &tmp, e))?;
    if std::fs::rename(&tmp, &out).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    Ok((out, ct))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: &str, prompt: &str, created: &str) -> GenerationRecord {
        GenerationRecord {
            id: id.into(),
            created_at: created.into(),
            mode: "generate".into(),
            original_prompt: None,
            loras: vec![],
            prompt: prompt.into(),
            negative_prompt: "blurry".into(),
            seed: 1234,
            width: 1024,
            height: 1024,
            working_width: 1024,
            working_height: 1024,
            model: "qwen-image-2.1-UC-Q4_K_M".into(),
            external_model: None,
            sampler: "euler".into(),
            scheduler: None,
            steps: 30,
            cfg: 6.0,
            strength: None,
            quality: "balanced".into(),
            resolution: "auto".into(),
            references: vec![ReferenceRecord { index: 0, input_id: "abc".into(), original_path: Some(r"C:\a.png".into()), width: 10, height: 20 }],
            mask_input_id: None,
            outpaint: Some(Expansion { left: 1, right: 2, top: 3, bottom: 4 }),
            file_path: r"C:\out\x.png".into(),
            extra_paths: vec![],
            favorite: false,
            parent_id: None,
            duration_ms: 5000,
            notes: vec!["n".into()],
            app_version: "1.0.0".into(),
            backend_version: Some("commit 3f8527a".into()),
            client: Some("ui".into()),
            request: serde_json::json!({"prompt": prompt}),
            pipeline: None,
            hidden: false,
            user: None,
            source_type: None,
        }
    }

    #[test]
    fn the_record_names_the_gateway_tenant_it_was_sent_to() {
        // Older records predate the field: they read fine, stay stock-shaped, and their
        // parameters text is unchanged.
        let mut old = rec("i", "p", "t");
        old.model = "model-Q8_0.gguf".into();
        assert_eq!(old.external_model, None);
        assert!(serde_json::to_value(&old).unwrap().get("external_model").is_none());
        assert!(!old.parameters_text().contains("kokon-"));

        // The same reported name with a tenant names the tenant: two box engines both
        // report "model-Q8_0.gguf", and it is the id, not the name, that tells them apart.
        old.external_model = Some("kokon-flux-image-3-klein".into());
        let text = old.parameters_text();
        assert!(text.contains("Model: model-Q8_0.gguf (kokon-flux-image-3-klein)"), "{text}");
        let rt: GenerationRecord = serde_json::from_value(serde_json::to_value(&old).unwrap()).unwrap();
        assert_eq!(rt.external_model.as_deref(), Some("kokon-flux-image-3-klein"));

        // A record deserialized from a pre-field JSON gains the absent field as None.
        let mut v = serde_json::to_value(&old).unwrap();
        v.as_object_mut().unwrap().remove("external_model");
        let from_old: GenerationRecord = serde_json::from_value(v).unwrap();
        assert_eq!(from_old.external_model, None);
    }

    #[test]
    fn pipeline_metadata_roundtrip_and_parameters() {
        let mut r = rec("g2", "a cabin", "2026-10-06T10:00:00.000Z");
        r.working_width = 2048;
        r.working_height = 1152;
        r.pipeline = Some(PipelineRecord {
            base: Some(Size::new(1376, 768)),
            refine: Some(RefineMode::Fast),
            refine_steps: Some(6),
            refine_denoise: Some(0.35),
            hires_upscaler: Some("RealESRGAN_x4plus".into()),
            cache_threshold: Some(0.2),
            enlarged_with: Some("RealESRGAN_x4plus".into()),
            enlarged_from: Some(Size::new(2048, 1152)),
            original_path: None,
        });
        let text = r.png_text();
        let params_ = &text.iter().find(|(k, _)| *k == "parameters").unwrap().1;
        assert!(params_.contains("Hires upscale: 1.49, Hires steps: 6, Hires upscaler: RealESRGAN_x4plus, Hires denoising strength: 0.35"), "{params_}");
        let owned: Vec<(String, String)> = text.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        let back = GenerationRecord::from_png_text(&owned).unwrap();
        assert_eq!(back.pipeline, r.pipeline);
        // Records written before this field existed still load.
        let mut v = serde_json::to_value(rec("g3", "x", "t")).unwrap();
        v.as_object_mut().unwrap().remove("pipeline");
        assert_eq!(serde_json::from_value::<GenerationRecord>(v).unwrap().pipeline, None);
    }

    #[test]
    fn metadata_serialization_roundtrip_via_png() {
        let r = rec("g1", "a red fox in snow", "2026-10-06T10:00:00.000Z");
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255]));
        let text = r.png_text();
        let pairs: Vec<(&str, &str)> = text.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let png = crate::imaging::encode_png(&img, &pairs, None, true).unwrap();
        let back = crate::imaging::decode_image(&png).unwrap();
        let parsed = GenerationRecord::from_png_text(&back.text).unwrap();
        let mut expected = r.clone();
        expected.request = serde_json::Value::Null;
        assert_eq!(parsed, expected);
        let params_ = back.text.iter().find(|(k, _)| k == "parameters").unwrap().1.clone();
        assert!(params_.starts_with("a red fox in snow\nNegative prompt: blurry\nSteps: 30, Sampler: euler"));
        assert!(params_.contains("Seed: 1234, Size: 1024x1024"));
    }

    #[test]
    fn gallery_crud_search_favorites_sort() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path()).unwrap();
        s.insert_generation(&rec("a", "a red fox", "2026-10-06T10:00:00.000Z")).unwrap();
        s.insert_generation(&rec("b", "blue whale 100%", "2026-10-06T11:00:00.000Z")).unwrap();
        s.insert_generation(&rec("c", "red car", "2026-10-06T12:00:00.000Z")).unwrap();
        let (all, total) = s.list_generations(&ListQuery::default(), &OwnerFilter::All).unwrap();
        assert_eq!(total, 3);
        assert_eq!(all.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(), vec!["c", "b", "a"]);
        let (red, _) = s.list_generations(&ListQuery { q: Some("red".into()), ..Default::default() }, &OwnerFilter::All).unwrap();
        assert_eq!(red.len(), 2);
        let (pct, _) = s.list_generations(&ListQuery { q: Some("100%".into()), ..Default::default() }, &OwnerFilter::All).unwrap();
        assert_eq!(pct.len(), 1, "LIKE wildcards are escaped");
        s.set_favorite("a", true).unwrap();
        let (fav, n) = s.list_generations(&ListQuery { favorites: true, ..Default::default() }, &OwnerFilter::All).unwrap();
        assert_eq!((fav.len(), n, fav[0].favorite), (1, 1, true));
        let (old, _) = s.list_generations(&ListQuery { sort: Some("oldest".into()), limit: Some(1), ..Default::default() }, &OwnerFilter::All).unwrap();
        assert_eq!(old[0].id, "a");
        assert!(s.get_generation("a").unwrap().favorite);
        s.delete_generation("a", dir.path()).unwrap();
        assert_eq!(s.get_generation("a").unwrap_err().code, ErrorCode::NotFound);
        assert!(s.set_favorite("zzz", true).is_err());
        // Reopen persists.
        drop(s);
        let s = Store::open(dir.path()).unwrap();
        assert_eq!(s.list_generations(&ListQuery::default(), &OwnerFilter::All).unwrap().1, 2);
        assert_eq!(s.get_generation("c").unwrap().references[0].input_id, "abc");
    }

    #[test]
    fn hidden_results_leave_the_list_only() {
        let dir = tempfile::tempdir().unwrap();
        // A gallery made before `hidden`: the column is added on open.
        {
            let c = Connection::open(dir.path().join("gallery.db")).unwrap();
            c.execute_batch("CREATE TABLE generations (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, prompt TEXT NOT NULL, mode TEXT NOT NULL, favorite INTEGER NOT NULL DEFAULT 0, file_path TEXT NOT NULL, record TEXT NOT NULL);").unwrap();
            let r = rec("old", "kept", "2026-10-06T09:00:00.000Z");
            c.execute("INSERT INTO generations (id, created_at, prompt, mode, favorite, file_path, record) VALUES (?1,?2,?3,?4,0,?5,?6)", params![r.id, r.created_at, r.prompt, r.mode, r.file_path, serde_json::to_string(&r).unwrap()]).unwrap();
        }
        let s = Store::open(dir.path()).unwrap();
        s.insert_generation(&rec("a", "a step", "2026-10-06T10:00:00.000Z")).unwrap();
        s.insert_generation(&GenerationRecord { hidden: true, ..rec("b", "a lab step", "2026-10-06T11:00:00.000Z") }).unwrap();
        let ids = |s: &Store| s.list_generations(&ListQuery::default(), &OwnerFilter::All).unwrap().0.into_iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(ids(&s), vec!["a", "old"]);
        assert!(s.get_generation("b").unwrap().hidden, "a project still opens it");
        s.set_hidden("a", true).unwrap();
        s.set_hidden("b", false).unwrap();
        assert_eq!(ids(&s), vec!["b", "old"]);
        assert_eq!(s.list_generations(&ListQuery::default(), &OwnerFilter::All).unwrap().1, 2);
        assert!(s.set_hidden("zzz", true).is_err());
        // The record leaves `hidden` out unless set (older apps read the same JSON).
        assert!(serde_json::to_value(rec("x", "p", "t")).unwrap().get("hidden").is_none());
    }

    #[test]
    fn inputs_are_deduplicated() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path()).unwrap();
        let img = image::RgbaImage::from_pixel(30, 20, image::Rgba([5, 6, 7, 255]));
        let png = crate::imaging::encode_png(&img, &[], None, true).unwrap();
        let a = s.put_input(&png, Some("x.png"), None).unwrap();
        let b = s.put_input(&png, Some("y.png"), None).unwrap();
        assert_eq!(a.id, b.id);
        assert_eq!((a.width, a.height), (30, 20));
        assert!(Path::new(&a.path).is_file());
        assert!(s.input_thumb_path(&a.id).is_file());
        assert!(s.get_input("../../etc").is_err());
        assert!(s.put_input(b"garbage", None, None).is_err());
    }

    #[test]
    fn previews_are_screen_sized_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path()).unwrap();
        let big = dir.path().join("big.png");
        std::fs::write(&big, crate::imaging::encode_png(&image::RgbaImage::from_pixel(2048, 1024, image::Rgba([9, 99, 199, 255])), &[], None, true).unwrap()).unwrap();
        let stem = s.preview_path("g1");
        let (p, ct) = preview(&big, &stem).unwrap();
        assert_eq!(ct, "image/jpeg");
        assert_eq!(p, dir.path().join("thumbs").join("g1.preview.jpg"));
        let img = crate::imaging::decode_image(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!((img.pixels.width(), img.pixels.height()), (PREVIEW_SIDE, PREVIEW_SIDE / 2));
        // Made once: later requests read the kept file.
        std::fs::remove_file(&big).unwrap();
        assert_eq!(preview(&big, &stem).unwrap().0, p);
        assert!(preview(&big, &s.preview_path("g2")).is_err(), "no source, no preview");
        // A smaller image keeps its size; transparency stays (PNG).
        let small = dir.path().join("small.png");
        std::fs::write(&small, crate::imaging::encode_png(&image::RgbaImage::from_pixel(300, 200, image::Rgba([1, 2, 3, 255])), &[], None, true).unwrap()).unwrap();
        let (p, ct) = preview(&small, &s.input_preview_path("abc")).unwrap();
        assert_eq!(ct, "image/jpeg");
        assert_eq!(crate::imaging::decode_image(&std::fs::read(&p).unwrap()).unwrap().pixels.dimensions(), (300, 200));
        let clear = dir.path().join("clear.png");
        std::fs::write(&clear, crate::imaging::encode_png(&image::RgbaImage::from_pixel(1600, 1600, image::Rgba([1, 2, 3, 0])), &[], None, true).unwrap()).unwrap();
        let (p, ct) = preview(&clear, &s.preview_path("g3")).unwrap();
        assert_eq!(ct, "image/png");
        let img = crate::imaging::decode_image(&std::fs::read(&p).unwrap()).unwrap();
        assert!(img.has_transparency);
        assert_eq!(img.pixels.dimensions(), (PREVIEW_SIDE, PREVIEW_SIDE));
        // Removing the result removes its preview.
        let mut r = rec("g1", "p", "2026-10-24T10:00:00.000Z");
        r.file_path = dir.path().join("out").join("g1.png").to_string_lossy().into_owned();
        s.insert_generation(&r).unwrap();
        s.delete_generation("g1", &dir.path().join("out")).unwrap();
        assert!(!dir.path().join("thumbs").join("g1.preview.jpg").exists());
    }

    #[test]
    fn users_are_stored_as_hashes_and_unique_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path()).unwrap();
        assert!(s.list_users().unwrap().is_empty());
        let a = s.create_user("Anna", "correct horse").unwrap();
        assert_eq!(a.name, "Anna");
        assert!(a.id.starts_with("user_"));
        // The password never lands in the file. The row may sit in the WAL until a
        // checkpoint, so read the database and its WAL.
        let db = dir.path().join("gallery.db");
        let mut text = String::new();
        for f in [db.clone(), db.with_extension("db-wal")] {
            if f.is_file() {
                text.push_str(&String::from_utf8_lossy(&std::fs::read(&f).unwrap()));
            }
        }
        assert!(!text.contains("correct horse"));
        assert!(text.contains("$argon2id$"));
        // The same name is taken, any casing, and only once.
        assert_eq!(
            s.create_user("anna", "another door").unwrap_err().code,
            ErrorCode::InvalidInput
        );
        assert_eq!(s.user_by_name("ANNA").unwrap().unwrap().id, a.id);
        // The name rules: a filesystem-safe word, 1 to 20 characters.
        let too_long = "x".repeat(21);
        for bad in ["", "-", too_long.as_str(), "a b/c", "a\nb", "..", "日本語"] {
            assert!(validate_user_name(bad).is_err(), "{bad:?}");
        }
        let max_len = "x".repeat(20);
        for good in ["a", max_len.as_str(), "Anna B", "a.b-c_d 12"] {
            assert_eq!(validate_user_name(good).unwrap(), good.trim(), "{good:?}");
        }
        // The password keeps the shared one's minimum length.
        assert!(s.create_user("Ben", "short").is_err());
        let b = s.create_user("ben", "a long enough password").unwrap();
        assert!(s.verify_user_password("BEN", "a long enough password"));
        assert!(!s.verify_user_password("BEN", "nope"));
        assert!(!s.verify_user_password("nobody", "a long enough password"));
        assert_eq!(
            s.list_users().unwrap().iter().map(|u| u.name.as_str()).collect::<Vec<_>>(),
            vec!["Anna", "ben"],
            "the list is case-insensitively sorted"
        );
        s.set_user_password(&b.id, "a fresh password").unwrap();
        assert!(s.verify_user_password("ben", "a fresh password"));
        s.delete_user(&b.id).unwrap();
        assert_eq!(s.user_by_id(&b.id).unwrap(), None);
        assert_eq!(s.delete_user("zzz").unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn gallery_rows_are_scoped_by_their_owner() {
        let dir = tempfile::tempdir().unwrap();
        let s = Store::open(dir.path()).unwrap();
        let mut anna = rec("a1", "a red fox", "2026-11-01T10:00:00.000Z");
        anna.user = Some("Anna".into());
        let mut ben = rec("b1", "a blue whale", "2026-11-01T11:00:00.000Z");
        ben.user = Some("ben".into());
        let op = rec("o1", "an operator image", "2026-11-01T12:00:00.000Z");
        for r in [&anna, &ben, &op] {
            s.insert_generation(r).unwrap();
        }
        let ids = |f: &OwnerFilter| s.list_generations(&ListQuery::default(), f).unwrap().0.into_iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(ids(&OwnerFilter::All), vec!["o1", "b1", "a1"]);
        assert_eq!(ids(&OwnerFilter::Own), vec!["o1"], "ownerless = the operator and the legacy rows");
        assert_eq!(ids(&OwnerFilter::User("Anna".into())), vec!["a1"]);
        assert_eq!(ids(&OwnerFilter::User("ben".into())), vec!["b1"]);
        assert!(ids(&OwnerFilter::User("nobody".into())).is_empty());
        // The owner survives the round-trip: the column is the source, the record copies it.
        let mut anna2 = s.get_generation("a1").unwrap();
        assert_eq!(anna2.user.as_deref(), Some("Anna"));
        assert_eq!(s.get_generation("o1").unwrap().user, None);
        // The owner filter composes with search, favourites and the hidden rows.
        let (red, total) = s
            .list_generations(&ListQuery { q: Some("red".into()), ..Default::default() }, &OwnerFilter::User("Anna".into()))
            .unwrap();
        assert_eq!((red.len(), total), (1, 1));
        // A record written before the field existed reads with no owner.
        let v = serde_json::to_value(rec("old", "x", "t")).unwrap();
        assert!(v.get("user").is_none(), "the stock JSON keeps its shape");
    }

    #[test]
    fn an_old_gallery_db_gains_the_user_column_and_the_users_table() {
        let dir = tempfile::tempdir().unwrap();
        // The 0.12 table (with `hidden`, without `user`) and a row already in it.
        {
            let c = Connection::open(dir.path().join("gallery.db")).unwrap();
            c.execute_batch(
                "CREATE TABLE generations (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, prompt TEXT NOT NULL, mode TEXT NOT NULL, favorite INTEGER NOT NULL DEFAULT 0, file_path TEXT NOT NULL, record TEXT NOT NULL, hidden INTEGER NOT NULL DEFAULT 0);
                 CREATE TABLE inputs (id TEXT PRIMARY KEY, created_at TEXT NOT NULL, record TEXT NOT NULL);",
            )
            .unwrap();
            let r = rec("old", "kept", "2026-11-01T09:00:00.000Z");
            c.execute(
                "INSERT INTO generations (id, created_at, prompt, mode, favorite, file_path, record, hidden) VALUES (?1,?2,?3,?4,0,?5,?6,0)",
                params![r.id, r.created_at, r.prompt, r.mode, r.file_path, serde_json::to_string(&r).unwrap()],
            )
            .unwrap();
        }
        let s = Store::open(dir.path()).unwrap();
        assert_eq!(
            s.list_generations(&ListQuery::default(), &OwnerFilter::All).unwrap().0.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            vec!["old"]
        );
        assert!(s.get_generation("old").unwrap().user.is_none(), "the legacy row stays the operator's");
        // And the users table is there for the first account.
        let u = s.create_user("anna", "a long enough password").unwrap();
        assert_eq!(s.user_by_id(&u.id).unwrap().unwrap().name, "anna");
    }

    #[test]
    fn deleting_a_user_removes_only_their_rows_and_files() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let s = Store::open(dir.path()).unwrap();
        let u = s.create_user("anna", "a long enough password").unwrap();
        let mut anna = rec("a1", "a red fox", "2026-11-01T10:00:00.000Z");
        anna.user = Some(u.name.clone());
        anna.file_path = out.join("Anna").join("2026-11-01").join("a1.png").to_string_lossy().into_owned();
        let mut hidden = rec("a2", "a lab step", "2026-11-01T10:30:00.000Z");
        hidden.user = Some(u.name.clone());
        hidden.hidden = true;
        hidden.file_path = out.join("Anna").join("2026-11-01").join("a2.png").to_string_lossy().into_owned();
        let mut ben = rec("b1", "a blue whale", "2026-11-01T11:00:00.000Z");
        ben.file_path = out.join("2026-11-01").join("b1.png").to_string_lossy().into_owned();
        let mut op = rec("o1", "an operator image", "2026-11-01T12:00:00.000Z");
        op.file_path = out.join("2026-11-01").join("o1.png").to_string_lossy().into_owned();
        let anna_file = anna.file_path.clone();
        let hidden_file = hidden.file_path.clone();
        for r in [&anna, &hidden, &ben, &op] {
            std::fs::create_dir_all(Path::new(&r.file_path).parent().unwrap()).unwrap();
            std::fs::write(&r.file_path, b"png").unwrap();
            s.insert_generation(r).unwrap();
        }
        // The hidden row goes too (it is not in the list, so it is counted straight from the table).
        assert_eq!(s.delete_user_rows(&u.name, &out).unwrap(), 2);
        assert!(!Path::new(&anna_file).exists() && !Path::new(&hidden_file).exists());
        s.delete_user(&u.id).unwrap();
        assert_eq!(s.user_by_id(&u.id).unwrap(), None);
        // Ben's and the operator's rows and files are untouched.
        let left = s.list_generations(&ListQuery::default(), &OwnerFilter::All).unwrap().0.into_iter().map(|r| r.id).collect::<Vec<_>>();
        assert_eq!(left, vec!["o1", "b1"]);
        assert!(Path::new(&ben.file_path).exists() && Path::new(&op.file_path).exists());
    }
}
