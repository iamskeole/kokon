//! Prompt bundles for the pipeline VLMs, loaded at runtime from the deployment.
//!
//! Three families share this loader:
//!
//! - **BFL** (`assets/pup/`): the FLUX.3 Image (Klein) prompt upsampler's system
//!   prompts, the BFL service request-line formats, the chat template, the
//!   tokenizer and preprocessor configs (Apache-2.0, source and file hashes in
//!   `assets/pup/LICENSE.md`). BFL's prompts are contract assets: their text is
//!   never echoed into logs, tests, chat or docs — the loader and its tests deal
//!   in keys and hashes only. A BFL pipeline without a loaded bundle is a job-level
//!   configuration failure (the model is trained on the upsampler's captions).
//!
//! - **Qwen** (`assets/pup-qwen/`): Kokon's own enhancement system prompts for
//!   the Qwen-side VLM (t2i, edit, compose, per-reference label). Kokon's text,
//!   committed data; a missing or broken bundle falls back to the built-in
//!   defaults in [`crate::enhancer`] (the Qwen path never hard-fails on its
//!   bundle — see `assets/pup-qwen/README.md`).
//!
//! - **Qwen native** (deployment data-dir, e.g. `assets/pup-qwen-native/`): the
//!   Qwen-Image PE fine-tunes' own `system_prompt.txt` files — Qwen's text under
//!   the Qwen Research License, so a third-party contract asset exactly like the
//!   BFL set: the folder lives in the deployment's data dir with a `LICENSE` and
//!   a `manifest.json` (the system-prompt paths, the reference sampling rows,
//!   the compose style), and no byte of it is ever committed, echoed into logs,
//!   tests, chat or docs (keys and hashes only). These doors answer a JSON
//!   envelope instead of bare prompt text; the decode lives in [`crate::vlm`].
//!
//! A pipeline names its bundle in its config (`promptBundle`, schema 13,
//! [`crate::config::resolve_prompt_bundle`]) and the service loads it at runtime,
//! per job, so a replacement dropped into that folder applies to the next job.

use std::path::Path;

use serde::Deserialize;

use crate::error::{AppError, ErrorCode};
use crate::planner::ResolvedMode;

/// The prompt assets of one bundle folder, whichever family it is.
///
/// `t2i` is the only key a bundle must carry (both families have one). The BFL
/// family adds `i2i` + the two bbox variants from `system_prompts.json` and the
/// two request-line formats from its companion `request_format.json`; the Qwen
/// family adds `edit`, `compose` and `compose_label`. Missing keys are `None` —
/// callers decide what a gap means (the BFL branch errors, the Qwen branch
/// falls back to its built-in defaults).
#[derive(Debug, Clone, Default)]
pub struct PupBundle {
    /// Text-to-image system prompt (BFL's caption writer, Qwen's built-in, or —
    /// for the native family — the PE-T2I fine-tune's own prompt).
    pub t2i: Option<String>,
    // --- BFL family --------------------------------------------------------
    /// Image-conditioned (edit/combine) system prompt.
    pub i2i: Option<String>,
    /// t2i variant that maintains the layout-box table.
    pub t2i_bbox: Option<String>,
    /// i2i variant that maintains the layout-box table.
    pub i2i_bbox: Option<String>,
    /// The BFL service t2i request-line format (`request_format.json`).
    pub t2i_canvas: Option<String>,
    /// The BFL service image-scaffold request-line format (`request_format.json`).
    pub i2i_scaffold: Option<String>,
    // --- Qwen family -------------------------------------------------------
    /// Image-grounded edit/combine system prompt (built-in `EDIT_SYSTEM_PROMPT`).
    pub edit: Option<String>,
    /// Composition system prompt, step 2 of 2 (`COMPOSE_SYSTEM_PROMPT`).
    pub compose: Option<String>,
    /// Per-reference label system prompt, step 1 of 2 (`COMPOSE_LABEL_PROMPT`).
    pub compose_label: Option<String>,
    // --- Qwen native family -----------------------------------------------
    /// True for the native PE family (the bundle carries a `manifest.json`).
    pub native: bool,
    /// The PE-I2I fine-tune's own prompt: every image-grounded mode of the
    /// native family (edit, multi-reference, and the one-shot compose).
    pub native_i2i: Option<String>,
    /// The manifest's sampling row for t2i jobs (None = the reference row).
    pub native_sampling_t2i: Option<NativeSampling>,
    /// The manifest's sampling row for image-grounded jobs (None = the reference row).
    pub native_sampling_i2i: Option<NativeSampling>,
    /// The native compose style: one call to the I2I door with the references
    /// (the fine-tune's own task) instead of the two-step label-then-write.
    pub compose_one_shot: bool,
}

/// The six BFL prompt assets as owned strings: what the BFL request path needs.
/// A bundle the BFL branch cannot use (missing keys) is a clean configuration
/// error naming the key — never a silently partial caption contract.
#[derive(Debug, Clone)]
pub struct BflPrompts {
    pub t2i: String,
    pub i2i: String,
    pub t2i_bbox: String,
    pub i2i_bbox: String,
    pub t2i_canvas: String,
    pub i2i_scaffold: String,
}

/// The sampling row of a native PE door for one mode group: the fine-tune's
/// reference row (README: do_sample, temp 1.0, top_p 0.95, top_k 20) with the
/// per-mode output budget (`max_new_tokens` 16256 t2i / 24000 i2i), overridable
/// per bundle by its manifest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeSampling {
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub max_tokens: u32,
}

/// The reference row, as constant: what a manifest without an explicit row means.
pub fn native_reference_row(mode: crate::planner::ResolvedMode) -> NativeSampling {
    NativeSampling {
        temperature: 1.0,
        top_p: 0.95,
        top_k: 20,
        max_tokens: match mode {
            crate::planner::ResolvedMode::Generate => crate::enhancer::NATIVE_MAX_TOKENS_T2I,
            _ => crate::enhancer::NATIVE_MAX_TOKENS_I2I,
        },
    }
}

impl PupBundle {
    /// The BFL family, complete. `Ok` only when all six assets are present.
    pub fn bfl(&self) -> Result<BflPrompts, AppError> {
        let pick = |key: &str, v: &Option<String>| -> Result<String, AppError> {
            v.clone().ok_or_else(|| {
                AppError::new(
                    ErrorCode::InvalidConfig,
                    format!(
                        "the loaded prompt bundle is not a complete BFL bundle (missing its '{key}' prompt)"
                    ),
                )
            })
        };
        Ok(BflPrompts {
            t2i: pick("t2i", &self.t2i)?,
            i2i: pick("i2i", &self.i2i)?,
            t2i_bbox: pick("t2i_bbox", &self.t2i_bbox)?,
            i2i_bbox: pick("i2i_bbox", &self.i2i_bbox)?,
            t2i_canvas: pick("t2i_canvas", &self.t2i_canvas)?,
            i2i_scaffold: pick("i2i_scaffold", &self.i2i_scaffold)?,
        })
    }

    /// A Qwen-family system prompt for a mode, if the bundle carries it.
    ///
    /// Non-native (Kokon's own prompts): `t2i` for generation, `compose` for
    /// compositions, `edit` for every other image-grounded mode — the same
    /// mapping as [`crate::enhancer::system_prompt`].
    ///
    /// Native (the PE fine-tunes): the PE-T2I prompt for generation and the
    /// PE-I2I prompt for *every* image-grounded mode — compositions included
    /// (their one-shot form calls the I2I door with the references; the
    /// two-step label dance is the non-native path's compensation for a model
    /// that could not see them). Kokon's own area/Expand words never reach
    /// this function: those requests carry their own system prompt.
    pub fn qwen_system(&self, mode: ResolvedMode) -> Option<&str> {
        if self.native {
            return match mode {
                ResolvedMode::Generate => self.t2i.as_deref(),
                _ => self.native_i2i.as_deref(),
            };
        }
        match mode {
            ResolvedMode::Generate => self.t2i.as_deref(),
            ResolvedMode::Compose => self.compose.as_deref(),
            _ => self.edit.as_deref(),
        }
    }

    /// The native family's sampling row for a mode: the bundle's manifest row,
    /// else the reference row. None for the other families.
    pub fn native_sampling_for(&self, mode: ResolvedMode) -> Option<NativeSampling> {
        if !self.native {
            return None;
        }
        match mode {
            ResolvedMode::Generate => self.native_sampling_t2i.or(Some(native_reference_row(mode))),
            _ => self.native_sampling_i2i.or(Some(native_reference_row(mode))),
        }
    }

    /// The Qwen-family per-reference label prompt, if the bundle carries it.
    pub fn qwen_label(&self) -> Option<&str> {
        self.compose_label.as_deref()
    }
}

/// Load a bundle from a bundle folder.
///
/// The family is declared: a `manifest.json` makes it the native PE family
/// (the two `system_prompt.txt` files it names, its sampling rows, its compose
/// style, and a `LICENSE` in the folder — third-party text ships with its
/// license); otherwise it is the `system_prompts.json` shape (BFL family with
/// its BFL marker keys plus the companion `request_format.json`, or the Qwen
/// family with its optional keys).
///
/// Errors name the folder and the defect; they never carry a prompt byte.
pub fn load(dir: &Path) -> Result<PupBundle, AppError> {
    if dir.join("manifest.json").exists() {
        return load_native(dir);
    }
    load_prompt_json(dir)
}

/// The native PE family: a `manifest.json` plus the system-prompt files it
/// names and a `LICENSE` in the folder.
fn load_native(dir: &Path) -> Result<PupBundle, AppError> {
    let raw = std::fs::read_to_string(dir.join("manifest.json")).map_err(|e| {
        AppError::new(
            ErrorCode::InvalidConfig,
            format!("cannot read the prompt bundle manifest at {}: {e}", dir.display()),
        )
    })?;
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Row {
        temperature: f32,
        top_p: f32,
        top_k: u32,
        max_tokens: u32,
    }
    #[derive(Default, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Manifest {
        family: Option<String>,
        #[serde(default)]
        system_prompts: SystemPrompts,
        #[serde(default)]
        sampling: SamplingRows,
        #[serde(default)]
        compose_one_shot: bool,
    }
    #[derive(Default, Deserialize)]
    struct SystemPrompts {
        t2i: Option<String>,
        i2i: Option<String>,
    }
    #[derive(Default, Deserialize)]
    struct SamplingRows {
        t2i: Option<Row>,
        i2i: Option<Row>,
    }
    let m: Manifest = serde_json::from_str(&raw).map_err(|e| {
        AppError::new(
            ErrorCode::InvalidConfig,
            format!("cannot parse the prompt bundle manifest at {}: {e}", dir.display()),
        )
    })?;
    match m.family.as_deref() {
        // The only native family this loader knows; a manifest claiming another
        // is a different deployment's contract, not a misfiled Qwen bundle.
        Some("qwen-native") => {}
        other => {
            return Err(AppError::new(
                ErrorCode::InvalidConfig,
                format!(
                    "the prompt bundle at {} is a native manifest of family {:?}, not \"qwen-native\"",
                    dir.display(),
                    other
                ),
            ))
        }
    }
    let row_to_sampling = |r: &Row| NativeSampling {
        temperature: r.temperature,
        top_p: r.top_p,
        top_k: r.top_k,
        max_tokens: r.max_tokens,
    };
    // The system-prompt files: third-party contract text, read but never echoed.
    let t2i = native_prompt_file(dir, m.system_prompts.t2i.as_deref().unwrap_or("t2i/system_prompt.txt"), "t2i")?;
    let i2i = native_prompt_file(dir, m.system_prompts.i2i.as_deref().unwrap_or("i2i/system_prompt.txt"), "i2i")?;
    // Third-party text ships with its license: a native bundle without one is
    // incomplete (BFL discipline).
    if !dir.join("LICENSE").exists() {
        return Err(AppError::new(
            ErrorCode::InvalidConfig,
            format!(
                "the native prompt bundle at {} has no LICENSE (third-party prompt text ships with its license)",
                dir.display()
            ),
        ));
    }
    Ok(PupBundle {
        t2i: Some(t2i),
        native_i2i: Some(i2i),
        native: true,
        native_sampling_t2i: m.sampling.t2i.as_ref().map(row_to_sampling),
        native_sampling_i2i: m.sampling.i2i.as_ref().map(row_to_sampling),
        compose_one_shot: m.compose_one_shot,
        ..Default::default()
    })
}

/// One native system-prompt file: an error naming the file (never the bytes)
/// when it is absent, unreadable, or empty.
fn read_native_prompt(dir: &Path, rel: &str) -> Result<Option<String>, AppError> {
    let p = dir.join(rel);
    let s = match std::fs::read_to_string(&p) {
        Ok(s) => s,
        Err(_) if !p.exists() => return Ok(None),
        Err(e) => {
            return Err(AppError::new(
                ErrorCode::InvalidConfig,
                format!("cannot read the system prompt {} of the bundle at {}: {e}", rel, dir.display()),
            ))
        }
    };
    Ok(Some(s))
}

/// One native system prompt, refusing empty files (an empty prompt is the
/// absent prompt wearing a file extension).
fn native_prompt_file(dir: &Path, rel: &str, key: &str) -> Result<String, AppError> {
    let s = read_native_prompt(dir, rel)?
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::InvalidConfig,
                format!("the native prompt bundle at {} has no {key} system prompt", dir.display()),
            )
        })?;
    if s.trim().is_empty() {
        return Err(AppError::new(
            ErrorCode::InvalidConfig,
            format!("the native prompt bundle at {} has an empty {key} system prompt", dir.display()),
        ));
    }
    Ok(s)
}

/// The `system_prompts.json` families (BFL and Qwen), as before schema 14.
fn load_prompt_json(dir: &Path) -> Result<PupBundle, AppError> {
    let path = dir.join("system_prompts.json");
    let raw = std::fs::read_to_string(&path).map_err(|e| {
        AppError::new(
            ErrorCode::InvalidConfig,
            format!("cannot read the prompt bundle at {}: {e}", dir.display()),
        )
    })?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
        AppError::new(
            ErrorCode::InvalidConfig,
            format!("cannot parse the prompt bundle at {}: {e}", dir.display()),
        )
    })?;
    let pick = |key: &str| -> Option<String> {
        v.get(key).and_then(|x| x.as_str()).map(str::to_string)
    };
    // The only key both families must carry.
    let t2i = pick("t2i").ok_or_else(|| {
        AppError::new(
            ErrorCode::InvalidConfig,
            format!(
                "the prompt bundle at {} is missing its 't2i' prompt",
                dir.display()
            ),
        )
    })?;
    let i2i = pick("i2i");
    let t2i_bbox = pick("t2i_bbox");
    let i2i_bbox = pick("i2i_bbox");

    // Family: any BFL marker key makes it a BFL bundle, whose remaining keys
    // and companion request-format file are then mandatory (the BFL captions
    // are part of the model's training contract — a gap is a configuration
    // failure, not a fallback). Otherwise it is a Qwen bundle, whose other
    // keys are optional (the runtime falls back to its built-in defaults).
    let bfl_family = i2i.is_some() || t2i_bbox.is_some() || i2i_bbox.is_some();
    let mut bundle = PupBundle {
        t2i: Some(t2i),
        i2i,
        t2i_bbox,
        i2i_bbox,
        ..Default::default()
    };
    if bfl_family {
        for (key, slot) in [
            ("i2i", &mut bundle.i2i),
            ("t2i_bbox", &mut bundle.t2i_bbox),
            ("i2i_bbox", &mut bundle.i2i_bbox),
        ] {
            if slot.is_none() {
                return Err(AppError::new(
                    ErrorCode::InvalidConfig,
                    format!(
                        "the prompt bundle at {} is missing its '{key}' prompt",
                        dir.display()
                    ),
                ));
            }
        }
        // The request-line formats are Kokon's own companion file in the same
        // folder (committed with the BFL set, not part of BFL's file).
        let fmt_path = dir.join("request_format.json");
        let fmt_raw = std::fs::read_to_string(&fmt_path).map_err(|e| {
            AppError::new(
                ErrorCode::InvalidConfig,
                format!(
                    "cannot read request_format.json from the prompt bundle at {}: {e}",
                    dir.display()
                ),
            )
        })?;
        let fv: serde_json::Value = serde_json::from_str(&fmt_raw).map_err(|e| {
            AppError::new(
                ErrorCode::InvalidConfig,
                format!(
                    "cannot parse request_format.json in the prompt bundle at {}: {e}",
                    dir.display()
                ),
            )
        })?;
        let pick_fmt = |key: &str| -> Result<String, AppError> {
            fv.get(key)
                .and_then(|x| x.as_str())
                .map(str::to_string)
                .ok_or_else(|| {
                    AppError::new(
                        ErrorCode::InvalidConfig,
                        format!(
                            "request_format.json in the prompt bundle at {} is missing its '{key}' line",
                            dir.display()
                        ),
                    )
                })
        };
        bundle.t2i_canvas = Some(pick_fmt("t2i_canvas")?);
        bundle.i2i_scaffold = Some(pick_fmt("i2i_scaffold")?);
    } else {
        bundle.edit = pick("edit");
        bundle.compose = pick("compose");
        bundle.compose_label = pick("compose_label");
    }
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn asset_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/pup")
    }

    fn qwen_asset_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/pup-qwen")
    }

    fn sha(s: &str) -> String {
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        hex::encode(h.finalize())
    }

    // The hashes are of the four prompt strings inside the committed
    // system_prompts.json (computed once, 2026-11-02, from the committed file;
    // the file-level hashes are in assets/pup/LICENSE.md). Comparing hashes
    // keeps the BFL prompt text out of any failure output.
    #[test]
    fn the_loaded_bundle_matches_the_committed_assets() {
        let b = load(&asset_dir()).expect("the committed bundle loads");
        let bfl = b.bfl().expect("the committed BFL bundle is complete");
        assert_eq!(sha(&bfl.t2i), "8b235a421bce2213d11ada01f5a1244443dd72303ffbb4cc9e5a0d8b0de5ab6f");
        assert_eq!(sha(&bfl.i2i), "95cec384d5b6d341caa9fb3f2c7ca45ee80c2364b3a95d4044aebf976f6dd3bf");
        assert_eq!(sha(&bfl.t2i_bbox), "5f513d8ce5838732919f9e645507aa20ff373e88bf93132c768e063c9db3151f");
        assert_eq!(sha(&bfl.i2i_bbox), "9ad10f947e401337cf155cac0518cc428159ef16f2f029f48aae2aaab8380a5e");
        assert_eq!(sha(&bfl.t2i_canvas), "4c469fc6507dfe235e9d2a7b56cdfefd0106421af540a66b17caf80d62cfc161");
        assert_eq!(sha(&bfl.i2i_scaffold), "3fcea082f035e21088d8e708d8731b74c48cfbe855d018586eeec1729845d26b");
        assert!(bfl.t2i.trim().len() > 100, "the t2i prompt is a full paragraph");
        assert!(bfl.i2i_bbox.contains("ref_image_0"), "the i2i bbox prompt references the images");
    }

    #[test]
    fn a_bundle_without_its_request_formats_is_a_clean_error_naming_the_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("system_prompts.json"),
            r#"{"t2i":"one","i2i":"two","t2i_bbox":"three","i2i_bbox":"four"}"#,
        )
        .unwrap();
        let e = load(dir.path()).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidConfig);
        assert!(e.message.contains("request_format.json"), "it names the missing companion file");
        assert!(!e.message.contains("upsampler"));
    }

    #[test]
    fn a_missing_bundle_folder_is_a_clean_error_without_prompt_bytes() {
        let dir = std::path::Path::new("/nonexistent-bundle-dir");
        let e = load(dir).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidConfig);
        assert!(e.message.contains("cannot read the prompt bundle"));
        assert!(e.message.contains("/nonexistent-bundle-dir"));
    }

    #[test]
    fn a_bundle_missing_a_prompt_is_a_clean_error_naming_the_key() {
        let dir = tempfile::tempdir().unwrap();
        // A minimal bundle with only two of the four BFL prompts.
        std::fs::write(
            dir.path().join("system_prompts.json"),
            r#"{"t2i":"one","i2i":"two"}"#,
        )
        .unwrap();
        let e = load(dir.path()).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidConfig);
        assert!(e.message.contains("t2i_bbox"), "it names the missing key");
    }

    #[test]
    fn unparseable_bundle_json_is_a_clean_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("system_prompts.json"), "{not json").unwrap();
        let e = load(dir.path()).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidConfig);
        assert!(e.message.contains("cannot parse"));
    }

    // --- Qwen family --------------------------------------------------------

    #[test]
    fn the_committed_qwen_bundle_matches_the_built_in_defaults() {
        // The asset is a verbatim extraction of the built-in prompts
        // (assets/pup-qwen/README.md): pin both sides so a refresh lands in
        // one commit, and the bundle (data) and the fallback (code) can never
        // silently drift apart. Kokon's own text — asserted by equality,
        // unlike the BFL set.
        let b = load(&qwen_asset_dir()).expect("the committed qwen bundle loads");
        use crate::enhancer::{
            COMPOSE_LABEL_PROMPT, COMPOSE_SYSTEM_PROMPT, EDIT_SYSTEM_PROMPT, T2I_SYSTEM_PROMPT,
        };
        assert_eq!(b.t2i.as_deref(), Some(T2I_SYSTEM_PROMPT));
        assert_eq!(b.edit.as_deref(), Some(EDIT_SYSTEM_PROMPT));
        assert_eq!(b.compose.as_deref(), Some(COMPOSE_SYSTEM_PROMPT));
        assert_eq!(b.compose_label.as_deref(), Some(COMPOSE_LABEL_PROMPT));
        // No BFL assets in the file.
        assert!(b.i2i.is_none());
        assert!(b.t2i_canvas.is_none());
        assert!(b.i2i_scaffold.is_none());
    }

    #[test]
    fn a_qwen_bundle_needs_no_request_format_companion() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("system_prompts.json"),
            r#"{"t2i":"a","edit":"b","compose":"c","compose_label":"d"}"#,
        )
        .unwrap();
        let b = load(dir.path()).expect("a qwen bundle loads without a companion file");
        assert_eq!(b.t2i.as_deref(), Some("a"));
        assert_eq!(b.compose_label.as_deref(), Some("d"));
        assert!(b.t2i_canvas.is_none());
        assert!(b.i2i_scaffold.is_none());
        assert!(b.bfl().is_err(), "a qwen bundle is not a BFL bundle");
    }

    #[test]
    fn a_bundle_with_only_t2i_is_a_valid_qwen_bundle() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("system_prompts.json"),
            r#"{"t2i":"only the t2i prompt"}"#,
        )
        .unwrap();
        let b = load(dir.path()).expect("t2i alone is a loadable qwen bundle");
        assert_eq!(b.t2i.as_deref(), Some("only the t2i prompt"));
        assert!(b.edit.is_none(), "missing qwen keys are None, not errors");
        assert!(b.qwen_system(ResolvedMode::Edit).is_none());
        assert_eq!(b.qwen_system(ResolvedMode::Generate), Some("only the t2i prompt"));
    }

    #[test]
    fn a_bundle_without_any_t2i_is_a_clean_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("system_prompts.json"),
            r#"{"edit":"only an edit prompt"}"#,
        )
        .unwrap();
        let e = load(dir.path()).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidConfig);
        assert!(e.message.contains("'t2i'"), "it names the one mandatory key");
    }

    // --- Native family (the PE fine-tunes, schema 14) ------------------------

    const FAKE_T2I: &str = "FAKE-T2I-SP (test stand-in, never the contract text)";
    const FAKE_I2I: &str = "FAKE-I2I-SP (test stand-in, never the contract text)";

    fn write_native_bundle(dir: &std::path::Path, with_sampling: bool, with_license: bool) {
        let manifest = if with_sampling {
            r#"{"family":"qwen-native",
              "systemPrompts":{"t2i":"t2i/system_prompt.txt","i2i":"i2i/system_prompt.txt"},
              "sampling":{"t2i":{"temperature":1.0,"topP":0.95,"topK":20,"maxTokens":16256},
                          "i2i":{"temperature":1.0,"topP":0.95,"topK":20,"maxTokens":24000}},
              "composeOneShot":true}"#
        } else {
            r#"{"family":"qwen-native",
              "systemPrompts":{"t2i":"t2i/system_prompt.txt","i2i":"i2i/system_prompt.txt"},
              "composeOneShot":false}"#
        };
        std::fs::write(dir.join("manifest.json"), manifest).unwrap();
        std::fs::create_dir_all(dir.join("t2i")).unwrap();
        std::fs::create_dir_all(dir.join("i2i")).unwrap();
        std::fs::write(dir.join("t2i").join("system_prompt.txt"), FAKE_T2I).unwrap();
        std::fs::write(dir.join("i2i").join("system_prompt.txt"), FAKE_I2I).unwrap();
        if with_license {
            std::fs::write(dir.join("LICENSE"), "(test) license file").unwrap();
        }
    }

    #[test]
    fn a_native_bundle_loads_and_dispatches_its_prompts_by_mode() {
        let dir = tempfile::tempdir().unwrap();
        write_native_bundle(dir.path(), true, true);
        let b = load(dir.path()).expect("the native bundle loads");
        assert!(b.native, "the manifest declares the native family");
        assert!(b.bfl().is_err(), "a native bundle is not the BFL family");
        // Dispatch: generation the T2I prompt, every image-grounded mode (compose
        // included) the I2I prompt.
        assert_eq!(b.qwen_system(ResolvedMode::Generate), Some(FAKE_T2I));
        for m in [
            ResolvedMode::Edit,
            ResolvedMode::Inpaint,
            ResolvedMode::Outpaint,
            ResolvedMode::MultiReference,
            ResolvedMode::Compose,
        ] {
            assert_eq!(b.qwen_system(m), Some(FAKE_I2I), "{m:?} takes the i2i prompt");
        }
        // The prompt text never leaks into an error: hash-pin it instead.
        assert_eq!(sha(FAKE_T2I), sha(&b.t2i.as_deref().unwrap()));
        // Sampling: the manifest rows are used as written.
        let t = b.native_sampling_for(ResolvedMode::Generate).unwrap();
        assert_eq!((t.temperature, t.top_p, t.top_k, t.max_tokens), (1.0, 0.95, 20, 16256));
        let i = b.native_sampling_for(ResolvedMode::Edit).unwrap();
        assert_eq!((i.temperature, i.top_p, i.top_k, i.max_tokens), (1.0, 0.95, 20, 24000));
        assert_eq!(i.max_tokens, t.max_tokens + 7744, "the i2i row carries the image budget");
        assert!(b.compose_one_shot, "the manifest's one-shot compose style");
    }

    #[test]
    fn a_native_bundle_without_manifest_rows_uses_the_reference_row() {
        let dir = tempfile::tempdir().unwrap();
        write_native_bundle(dir.path(), false, true);
        let b = load(dir.path()).unwrap();
        assert!(!b.compose_one_shot);
        for m in [ResolvedMode::Generate, ResolvedMode::Edit, ResolvedMode::Compose] {
            let s = b.native_sampling_for(m).unwrap();
            assert_eq!((s.temperature, s.top_p, s.top_k), (1.0, 0.95, 20), "{m:?} reference row");
        }
        assert!(
            b.native_sampling_for(ResolvedMode::Edit).unwrap().max_tokens
                > b.native_sampling_for(ResolvedMode::Generate).unwrap().max_tokens,
            "the reference i2i row keeps its image-token headroom"
        );
    }

    #[test]
    fn a_native_bundle_without_a_license_is_a_clean_error_naming_the_file() {
        let dir = tempfile::tempdir().unwrap();
        write_native_bundle(dir.path(), true, false);
        let e = load(dir.path()).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidConfig);
        assert!(e.message.contains("LICENSE"), "it names the missing license file");
    }

    #[test]
    fn a_native_manifest_of_another_family_is_a_clean_error() {
        let dir = tempfile::tempdir().unwrap();
        write_native_bundle(dir.path(), true, true);
        std::fs::write(dir.path().join("manifest.json"), r#"{"family":"other"}"#).unwrap();
        let e = load(dir.path()).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidConfig);
        assert!(e.message.contains("\"other\""), "it names the foreign family: {}", e.message);
        assert!(!e.message.contains(FAKE_T2I), "no prompt bytes in the error");
    }

    #[test]
    fn a_native_bundle_with_an_empty_prompt_file_is_a_clean_error_naming_the_key() {
        let dir = tempfile::tempdir().unwrap();
        write_native_bundle(dir.path(), true, true);
        std::fs::write(dir.path().join("t2i").join("system_prompt.txt"), "  ").unwrap();
        let e = load(dir.path()).unwrap_err();
        assert_eq!(e.code, ErrorCode::InvalidConfig);
        assert!(e.message.contains("t2i"), "it names the empty prompt");
    }

    #[test]
    fn a_manifest_bundle_is_not_treated_as_the_json_families() {
        // The family is decided by the manifest's presence, not by which
        // companion file happens to sit next to it.
        let dir = tempfile::tempdir().unwrap();
        write_native_bundle(dir.path(), true, true);
        std::fs::write(
            dir.path().join("system_prompts.json"),
            r#"{"t2i":"should not be read"}"#,
        )
        .unwrap();
        let b = load(dir.path()).unwrap();
        assert_eq!(b.qwen_system(ResolvedMode::Generate), Some(FAKE_T2I));
    }
}
