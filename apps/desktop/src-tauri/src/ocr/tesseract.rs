//! Tesseract 5, invoked as a bundled sidecar process (`docs/SPRINT-2.5-ASSESSMENT.md`, section E).
//!
//! Every rule below exists to keep patient text off disk and off the network:
//!
//! - `tesseract - - -l <lang> tsv` reads the image from stdin and writes to stdout. Never a file
//!   path on either side: the CLI's default form writes a `.txt` beside the input, which would
//!   put recognised text in an unmanaged file outside the index.
//! - The startup probe runs once, at construction, not per page. A missing sidecar or a missing
//!   language model degrades to Sprint 2a behaviour - report the page empty - rather than failing
//!   the index or the application launch.
//! - No `#[cfg(windows)]` anywhere in this file. Which prebuilt binary is bundled is a bundler
//!   and sidecar-discovery concern; the invocation is identical on both platforms.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::{ImageFormat, OcrError, OcrInput, OcrPage, OcrProvider, OcrStatus};

/// Below this mean word confidence, recognised text is kept internally but the page is treated
/// as unread by retrieval (`OcrStatus::BelowThreshold`, `docs/SPRINT-2.5-ASSESSMENT.md` section G).
pub const CONFIDENCE_FLOOR: f32 = 0.6;

/// A page rarely needs more than a few seconds. Past this, something is wrong with the image or
/// the sidecar, and the caller must not hang the index over one scan.
pub const PER_PAGE_TIMEOUT: Duration = Duration::from_secs(20);

/// BCP 47 from settings to Tesseract's own language vocabulary. A lookup table, never a literal
/// at the call site, and never a French word: `docs/LANGUAGE-AND-LOCALE.md`.
fn engine_locale(locale: &str) -> Result<&'static str, OcrError> {
    match locale {
        "fr-FR" => Ok("fra"),
        "en-US" => Ok("eng"),
        other => Err(OcrError::LanguageUnavailable {
            locale: other.to_string(),
        }),
    }
}

/// One bundled Tesseract sidecar, probed once at construction.
pub struct TesseractProvider {
    binary_path: PathBuf,
    tessdata_dir: PathBuf,
    version: String,
}

impl TesseractProvider {
    /// `binary_path` and `tessdata_dir` are resolved by the caller from the platform resource
    /// directory - never a literal here. Fails with `OcrError::EngineUnavailable` if the sidecar
    /// does not start; the caller degrades to Sprint 2a behaviour rather than failing launch.
    pub fn new(binary_path: PathBuf, tessdata_dir: PathBuf) -> Result<Self, OcrError> {
        let mut command = Command::new(&binary_path);
        apply_dll_search_path(&mut command, &binary_path, &tessdata_dir);
        let output = command
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|_| OcrError::EngineUnavailable)?;
        if !output.status.success() {
            return Err(OcrError::EngineUnavailable);
        }
        let banner = String::from_utf8_lossy(&output.stdout);
        let version = banner
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().last())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            binary_path,
            tessdata_dir,
            version,
        })
    }
}

impl OcrProvider for TesseractProvider {
    fn id(&self) -> &str {
        "tesseract"
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn supports(&self, format: ImageFormat) -> bool {
        matches!(format, ImageFormat::Jpeg | ImageFormat::Png)
    }

    fn recognise(&self, input: &OcrInput<'_>) -> Result<OcrPage, OcrError> {
        let language = engine_locale(input.locale)?;

        let mut child_cmd = Command::new(&self.binary_path);
        apply_dll_search_path(&mut child_cmd, &self.binary_path, &self.tessdata_dir);
        let mut child = child_cmd
            .arg("-")
            .arg("-")
            .arg("-l")
            .arg(language)
            .arg("--tessdata-dir")
            .arg(&self.tessdata_dir)
            // A bare "tsv" argument asks Tesseract to load the *config file* named "tsv" from
            // `<tessdata-dir>/configs/tsv` (or `TESSDATA_PREFIX/configs`), which we do not bundle
            // - only `fra.traineddata` ships in `resources/tessdata` (`resources/README.md`). The
            // config file's entire content is one variable assignment, so setting it directly with
            // `-c` produces the same TSV stdout without needing that extra bundled resource.
            .arg("-c")
            .arg("tessedit_create_tsv=1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| OcrError::EngineUnavailable)?;

        let mut stdin = child.stdin.take().ok_or(OcrError::EngineUnavailable)?;
        stdin
            .write_all(input.image)
            .map_err(|_| OcrError::UnreadableImage)?;
        drop(stdin);

        let output = wait_with_timeout(child, PER_PAGE_TIMEOUT)?;
        if !output.status.success() {
            return Err(OcrError::UnreadableImage);
        }

        let tsv = String::from_utf8_lossy(&output.stdout);
        let (text, confidence) = parse_tsv(&tsv);

        let status = if text.trim().is_empty() {
            OcrStatus::NoTextFound
        } else if confidence.is_some_and(|value| value < CONFIDENCE_FLOOR) {
            OcrStatus::BelowThreshold
        } else {
            OcrStatus::Recognised
        };

        Ok(OcrPage {
            relative_path: input.relative_path.to_string(),
            page_number: input.page_number,
            text,
            confidence,
            engine: self.id().to_string(),
            engine_version: self.version.clone(),
            status,
        })
    }
}

/// Windows loads Tesseract's DLLs from PATH and from the directory of the executable.
/// Sidecar discovery may find `tesseract.exe` a few folders away from those DLLs during
/// `tauri dev`, so each invocation prepends the binary directory and `resources/tesseract`.
/// `std::env::join_paths` keeps this platform-agnostic: no `cfg` in this module.
fn apply_dll_search_path(command: &mut Command, binary_path: &Path, tessdata_dir: &Path) {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(parent) = binary_path.parent() {
        dirs.push(parent.to_path_buf());
        if let Some(crate_root) = parent.parent() {
            dirs.push(crate_root.join("resources").join("tesseract"));
        }
    }
    if let Some(tess_parent) = tessdata_dir.parent() {
        dirs.push(tess_parent.join("tesseract"));
        dirs.push(tess_parent.to_path_buf());
    }
    let mut parts: Vec<PathBuf> = dirs.into_iter().filter(|dir| dir.is_dir()).collect();
    if let Some(existing) = std::env::var_os("PATH") {
        parts.extend(std::env::split_paths(&existing));
    }
    if let Ok(joined) = std::env::join_paths(parts) {
        command.env("PATH", joined);
    }
}

/// Polls the child rather than blocking forever, so one unreadable image cannot hang the index.
/// `std::process::Child` has no built-in timeout; this is the smallest correct substitute.
fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Result<std::process::Output, OcrError> {
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                return child.wait_with_output().map_err(|_| OcrError::UnreadableImage);
            }
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(OcrError::Timeout);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => return Err(OcrError::UnreadableImage),
        }
    }
}

/// Tesseract's TSV output: one header line, then one row per detected element. Level 5 rows are
/// words; anything above that (block, paragraph, line) carries no text and a confidence of -1.
/// Reconstructs the page's text by joining word rows with the whitespace Tesseract's own line and
/// block boundaries imply, and returns the mean word confidence in `0.0..=1.0`.
fn parse_tsv(tsv: &str) -> (String, Option<f32>) {
    const WORD_LEVEL: &str = "5";
    let mut words: Vec<(i64, i64, String)> = Vec::new();
    let mut confidences: Vec<f32> = Vec::new();

    for line in tsv.lines().skip(1) {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 12 || fields[0] != WORD_LEVEL {
            continue;
        }
        let text = fields[11].trim();
        if text.is_empty() {
            continue;
        }
        let line_num = fields[4].parse::<i64>().unwrap_or(0);
        let word_num = fields[5].parse::<i64>().unwrap_or(0);
        words.push((line_num, word_num, text.to_string()));
        if let Ok(confidence) = fields[10].parse::<f32>() {
            if confidence >= 0.0 {
                confidences.push(confidence / 100.0);
            }
        }
    }

    let mut current_line = None;
    let mut rendered = String::new();
    for (line_num, _, text) in words {
        if current_line.is_some() && current_line != Some(line_num) {
            rendered.push('\n');
        } else if current_line.is_some() {
            rendered.push(' ');
        }
        rendered.push_str(&text);
        current_line = Some(line_num);
    }

    let confidence = if confidences.is_empty() {
        None
    } else {
        Some(confidences.iter().sum::<f32>() / confidences.len() as f32)
    };

    (rendered, confidence)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_french_locale_maps_to_tesseracts_own_code() {
        assert_eq!(engine_locale("fr-FR"), Ok("fra"));
    }

    #[test]
    fn the_english_locale_maps_to_tesseracts_own_code() {
        assert_eq!(engine_locale("en-US"), Ok("eng"));
    }

    #[test]
    fn an_unmapped_locale_is_reported_rather_than_guessed() {
        assert_eq!(
            engine_locale("de-DE"),
            Err(OcrError::LanguageUnavailable {
                locale: "de-DE".to_string()
            })
        );
    }

    #[test]
    fn tsv_words_on_the_same_line_are_joined_with_spaces() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   5\t1\t1\t1\t1\t1\t0\t0\t10\t10\t95.0\tBonjour\n\
                   5\t1\t1\t1\t1\t2\t20\t0\t10\t10\t90.0\tCamille\n";

        let (text, confidence) = parse_tsv(tsv);

        assert_eq!(text, "Bonjour Camille");
        assert!((confidence.unwrap() - 0.925).abs() < 0.001);
    }

    #[test]
    fn tsv_words_on_different_lines_are_separated_by_a_newline() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   5\t1\t1\t1\t1\t1\t0\t0\t10\t10\t80.0\tBonjour\n\
                   5\t1\t1\t2\t2\t1\t0\t20\t10\t10\t70.0\tCamille\n";

        let (text, _) = parse_tsv(tsv);

        assert_eq!(text, "Bonjour\nCamille");
    }

    #[test]
    fn non_word_rows_carry_no_text_and_are_ignored() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   1\t1\t0\t0\t0\t0\t0\t0\t100\t100\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t0\t0\t10\t10\t95.0\tBonjour\n";

        let (text, confidence) = parse_tsv(tsv);

        assert_eq!(text, "Bonjour");
        assert_eq!(confidence, Some(0.95));
    }

    /// Exercises the real bundled sidecar rather than `FakeOcrProvider`. Ignored by default
    /// because `resources/tessdata/fra.traineddata` and `binaries/tesseract-*` are gitignored
    /// (`resources/README.md`, `binaries/README.md`) and only exist once
    /// `scripts/fetch-ocr-resources.ps1` has run - never on CI. Run it by hand after that script
    /// with `cargo test --lib -- --ignored recognises_a_real_bundled_prescription_scan`.
    #[test]
    #[ignore]
    fn recognises_a_real_bundled_prescription_scan() {
        let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let binary = crate_root
            .join("binaries")
            .join("tesseract-x86_64-pc-windows-msvc.exe");
        let tessdata = crate_root.join("resources").join("tessdata");
        let image_path = crate_root
            .join("..")
            .join("..")
            .join("..")
            .join("fixtures")
            .join("gp-sandbox")
            .join("inbox")
            .join("2026-03-26_ordonnance-scan.png");
        if !binary.exists() || !tessdata.join("fra.traineddata").exists() {
            eprintln!("skipping: OCR resources not staged, run scripts/fetch-ocr-resources.ps1");
            return;
        }

        let provider = TesseractProvider::new(binary, tessdata).expect("sidecar starts");
        let image = std::fs::read(&image_path).expect("fixture readable");
        let page = provider
            .recognise(&OcrInput {
                image: &image,
                relative_path: "inbox/2026-03-26_ordonnance-scan.png",
                page_number: 1,
                format: ImageFormat::Png,
                locale: "fr-FR",
            })
            .expect("the bundled sidecar recognises the fixture");

        assert_eq!(page.status, OcrStatus::Recognised);
        assert!(page.text.contains("Paracetamol"), "text was: {}", page.text);
    }

    #[test]
    fn a_page_with_no_recognised_words_has_no_confidence() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n";

        let (text, confidence) = parse_tsv(tsv);

        assert_eq!(text, "");
        assert_eq!(confidence, None);
    }
}
