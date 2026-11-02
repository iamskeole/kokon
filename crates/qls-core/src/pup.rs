//! The BFL prompt bundle of the FLUX.3 Image (Klein) prompt upsampler.
//!
//! The bundle is committed data: `assets/pup/` next to this module (the four upsampler
//! system prompts, the BFL service request-line formats, the chat template, the tokenizer
//! and preprocessor configs; Apache-2.0, source and file hashes in `assets/pup/LICENSE.md`).
//! A pipeline names it in its config (`promptBundle`, schema 13) and the service loads it
//! at runtime — BFL's prompts are contract assets, so their text is never echoed into
//! logs, tests, chat or docs: the loader and its tests deal in paths and hashes only.

use std::path::Path;

use crate::error::{AppError, ErrorCode};

/// The four BFL upsampler system prompts plus the two BFL service request-line formats
/// (the t2i canvas line and the image-scaffold): plain and layout-box variants for
/// text-to-image and for image-conditioned requests.
#[derive(Debug, Clone)]
pub struct PupBundle {
    pub t2i: String,
    pub i2i: String,
    pub t2i_bbox: String,
    pub i2i_bbox: String,
    pub t2i_canvas: String,
    pub i2i_scaffold: String,
}

/// Load the bundle from a bundle folder (its `system_prompts.json`).
///
/// Errors name the folder and the defect; they never carry a prompt byte.
pub fn load(dir: &Path) -> Result<PupBundle, AppError> {
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
    let pick = |key: &str| -> Result<String, AppError> {
        v.get(key)
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::InvalidConfig,
                    format!(
                        "the prompt bundle at {} is missing its '{key}' prompt",
                        dir.display()
                    ),
                )
            })
    };
    // The BFL file's four prompts are checked before the companion file, so a broken
    // contract bundle names its own defect first.
    let t2i = pick("t2i")?;
    let i2i = pick("i2i")?;
    let t2i_bbox = pick("t2i_bbox")?;
    let i2i_bbox = pick("i2i_bbox")?;
    // The request-line formats are Kokon's own companion file in the same folder.
    let fmt_path = dir.join("request_format.json");
    let fmt_raw = std::fs::read_to_string(&fmt_path).map_err(|e| {
        AppError::new(
            ErrorCode::InvalidConfig,
            format!("cannot read request_format.json from the prompt bundle at {}: {e}", dir.display()),
        )
    })?;
    let fv: serde_json::Value = serde_json::from_str(&fmt_raw).map_err(|e| {
        AppError::new(
            ErrorCode::InvalidConfig,
            format!(
                "cannot parse request_format.json from the prompt bundle at {}: {e}",
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
    Ok(PupBundle {
        t2i,
        i2i,
        t2i_bbox,
        i2i_bbox,
        t2i_canvas: pick_fmt("t2i_canvas")?,
        i2i_scaffold: pick_fmt("i2i_scaffold")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn asset_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/pup")
    }

    fn sha(s: &str) -> String {
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        hex::encode(h.finalize())
    }

    // The hashes are of the four prompt strings inside the committed
    // system_prompts.json (computed once, 2026-11-02, from the committed file;
    // the file-level hashes are in assets/pup/LICENSE.md). Comparing hashes
    // keeps the prompt text out of any failure output.
    #[test]
    fn the_loaded_bundle_matches_the_committed_assets() {
        let b = load(&asset_dir()).expect("the committed bundle loads");
        assert_eq!(sha(&b.t2i), "8b235a421bce2213d11ada01f5a1244443dd72303ffbb4cc9e5a0d8b0de5ab6f");
        assert_eq!(sha(&b.i2i), "95cec384d5b6d341caa9fb3f2c7ca45ee80c2364b3a95d4044aebf976f6dd3bf");
        assert_eq!(sha(&b.t2i_bbox), "5f513d8ce5838732919f9e645507aa20ff373e88bf93132c768e063c9db3151f");
        assert_eq!(sha(&b.i2i_bbox), "9ad10f947e401337cf155cac0518cc428159ef16f2f029f48aae2aaab8380a5e");
        assert_eq!(sha(&b.t2i_canvas), "4c469fc6507dfe235e9d2a7b56cdfefd0106421af540a66b17caf80d62cfc161");
        assert_eq!(sha(&b.i2i_scaffold), "3fcea082f035e21088d8e708d8731b74c48cfbe855d018586eeec1729845d26b");
        assert!(b.t2i.trim().len() > 100, "the t2i prompt is a full paragraph");
        assert!(b.i2i_bbox.contains("ref_image_0"), "the i2i bbox prompt references the images");
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
        // A minimal bundle with only two of the four prompts.
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
}
