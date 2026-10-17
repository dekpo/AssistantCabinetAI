//! Reading one file of a folder so the viewer can show it.
//!
//! Read-only by construction: this module opens a file and returns bytes. It never writes,
//! renames, moves, deletes or hashes anything, and it knows nothing of the index or of the
//! inventory caches, so opening a file for display cannot change what was analysed.
//!
//! The webview names the file, so the path is untrusted. `reveal::resolve_plain_file` rebuilds it
//! from plain names and refuses a last element that is a link or a folder; this module then
//! canonicalises both the folder and the target and requires the target to stay under the folder,
//! which catches a link or junction in the middle of the path.
//!
//! Text-like files are handed over as UTF-8, converted in memory, so accents are right whatever
//! the file's own encoding. The file on disk is never rewritten: a professional document's content
//! is not ours to change (`AGENTS.md`). Every other file is returned byte for byte.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::readers::{self, decode_text};
use crate::reveal::{resolve_plain_file, PathRefusal};

/// The largest file the viewer accepts: 128 MiB (owner decision, 17 October 2026). The bytes cross
/// the IPC, sit in the webview as an `ArrayBuffer`, then are copied into the WebAssembly engine, so
/// the peak memory is several times the file on a 2019 workstation (`docs/HARDWARE.md`).
pub const MAX_VIEWER_BYTES: u64 = 128 * 1024 * 1024;

/// Extensions whose bytes are text: converted to UTF-8 before they leave. EML stays raw (headers,
/// MIME structure and encoded bodies as stored); only its character encoding is made UTF-8.
const TEXT_LIKE_EXTENSIONS: [&str; 6] = ["txt", "md", "srt", "vtt", "eml", "csv"];

/// The bytes of `relative` under `root`, ready for the viewer, within `MAX_VIEWER_BYTES`.
pub fn read(root: &Path, relative: &str) -> Result<Vec<u8>, AppError> {
    read_with_cap(root, relative, MAX_VIEWER_BYTES)
}

/// `read` with the size cap given, so a test can use a small one.
pub(crate) fn read_with_cap(
    root: &Path,
    relative: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, AppError> {
    let (target, bytes) = read_stored(root, relative, max_bytes)?;
    if is_text_like(&target) {
        Ok(to_utf8(&bytes))
    } else {
        Ok(bytes)
    }
}

/// The file's bytes exactly as stored, with the path that was read: every check of `read`, none of
/// its conversion.
fn read_stored(
    root: &Path,
    relative: &str,
    max_bytes: u64,
) -> Result<(PathBuf, Vec<u8>), AppError> {
    let target = resolve_plain_file(root, relative).map_err(|refusal| match refusal {
        PathRefusal::Missing => AppError::ViewerFileMissing,
        PathRefusal::UnsafePath | PathRefusal::NotAPlainFile => AppError::ViewerFileUnreadable,
    })?;
    let target = canonical_inside(root, &target)?;

    let mut file = std::fs::File::open(&target).map_err(|error| map_io(&error))?;
    let size = file.metadata().map_err(|error| map_io(&error))?.len();
    if size > max_bytes {
        return Err(AppError::ViewerFileTooLarge);
    }
    // Read one byte past the cap: a file that grew since `metadata` is still refused.
    let mut bytes = Vec::with_capacity(size as usize);
    (&mut file)
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| map_io(&error))?;
    if bytes.len() as u64 > max_bytes {
        return Err(AppError::ViewerFileTooLarge);
    }

    Ok((target, bytes))
}

/// The extensions whose text the readers can give back, as the assistant reads it.
const READABLE_EXTENSIONS: [&str; 3] = ["eml", "srt", "vtt"];

/// The text of `relative` under `root` as the assistant reads it (`readers::eml`, `srt`, `vtt`),
/// as UTF-8 bytes. The path checks, the size cap and the codes are the raw read's own. A mail is
/// handed to its reader as stored, exactly as the indexing does, because the reader decodes the
/// character set the message declares and bytes already turned into UTF-8 would be decoded twice;
/// subtitles go through the same UTF-8 conversion as the raw view (UTF-16 included). The readers
/// are called, never changed, and nothing is written or indexed. A file the reader finds empty
/// gives no bytes. Any other extension is refused as unreadable.
pub fn read_readable_text(root: &Path, relative: &str) -> Result<Vec<u8>, AppError> {
    read_readable_text_with_cap(root, relative, MAX_VIEWER_BYTES)
}

/// `read_readable_text` with the size cap given, so a test can use a small one.
pub(crate) fn read_readable_text_with_cap(
    root: &Path,
    relative: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, AppError> {
    let extension = Path::new(relative)
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .unwrap_or_default();
    if !READABLE_EXTENSIONS.contains(&extension.as_str()) {
        return Err(AppError::ViewerFileUnreadable);
    }
    let (_, stored) = read_stored(root, relative, max_bytes)?;
    let text = match extension.as_str() {
        "eml" => readers::eml::text(&stored),
        "srt" => readers::srt::text(&decoded(&stored)),
        _ => readers::vtt::text(&decoded(&stored)),
    };
    Ok(text.into_bytes())
}

/// `target` canonicalised, if it still lies under `root` once every link on the way is resolved.
/// Both sides are canonicalised, never one of them: on Windows both come back in the same verbatim
/// `\\?\` form, and comparing a canonical path with a plain one would refuse every file.
fn canonical_inside(root: &Path, target: &Path) -> Result<PathBuf, AppError> {
    let root = root.canonicalize().map_err(|error| map_io(&error))?;
    let target = target.canonicalize().map_err(|error| map_io(&error))?;
    if target.starts_with(&root) {
        Ok(target)
    } else {
        Err(AppError::ViewerFileUnreadable)
    }
}

/// A file that vanished between the check and the read is missing; anything else is unreadable.
fn map_io(error: &std::io::Error) -> AppError {
    if error.kind() == std::io::ErrorKind::NotFound {
        AppError::ViewerFileMissing
    } else {
        AppError::ViewerFileUnreadable
    }
}

fn is_text_like(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .is_some_and(|extension| TEXT_LIKE_EXTENSIONS.contains(&extension.as_str()))
}

/// The text of stored bytes, through the conversion the raw view uses.
fn decoded(stored: &[u8]) -> String {
    String::from_utf8_lossy(&to_utf8(stored)).into_owned()
}

/// UTF-8 bytes for a text file. A UTF-16 byte-order mark comes first: `decode_text` alone would
/// read UTF-16 as Windows-1252 and corrupt it. Otherwise `decode_text` decides (UTF-8 if valid,
/// else Windows-1252, a UTF-8 mark dropped). The mark itself is not kept in the output.
fn to_utf8(raw: &[u8]) -> Vec<u8> {
    if let Some(rest) = raw.strip_prefix(&[0xFF, 0xFE]) {
        return encoding_rs::UTF_16LE
            .decode_without_bom_handling(rest)
            .0
            .into_owned()
            .into_bytes();
    }
    if let Some(rest) = raw.strip_prefix(&[0xFE, 0xFF]) {
        return encoding_rs::UTF_16BE
            .decode_without_bom_handling(rest)
            .0
            .into_owned()
            .into_bytes();
    }
    decode_text(raw).into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder_with(name: &str, bytes: &[u8]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::write(dir.path().join(name), bytes).expect("writes");
        dir
    }

    fn utf16(text: &str, little_endian: bool) -> Vec<u8> {
        let mut bytes = if little_endian {
            vec![0xFF, 0xFE]
        } else {
            vec![0xFE, 0xFF]
        };
        for unit in text.encode_utf16() {
            let pair = if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            };
            bytes.extend_from_slice(&pair);
        }
        bytes
    }

    /// A link at `link` pointing to `to`, or `false` where the platform or the account does not
    /// allow creating one. A test that gets `false` says so and passes: it proves nothing then.
    #[cfg(unix)]
    fn make_link(link: &Path, to: &Path, _is_dir: bool) -> bool {
        std::os::unix::fs::symlink(to, link).is_ok()
    }

    /// On Windows a symbolic link needs a privilege most accounts lack, but a junction does not,
    /// and a junction is what leads out of a folder without one. Both are tried.
    #[cfg(windows)]
    fn make_link(link: &Path, to: &Path, is_dir: bool) -> bool {
        if is_dir {
            if std::os::windows::fs::symlink_dir(to, link).is_ok() {
                return true;
            }
            return std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(to)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false);
        }
        std::os::windows::fs::symlink_file(to, link).is_ok()
    }

    #[cfg(not(any(unix, windows)))]
    fn make_link(_link: &Path, _to: &Path, _is_dir: bool) -> bool {
        false
    }

    #[test]
    fn a_plain_file_comes_back_byte_for_byte() {
        let bytes: Vec<u8> = (0..=255u8).collect();
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("inbox")).expect("creates");
        std::fs::write(dir.path().join("inbox").join("scan.pdf"), &bytes).expect("writes");
        std::fs::write(dir.path().join("inbox").join("memo.rtf"), &bytes).expect("writes");

        // A relative path with a subfolder, as the inventory lists it.
        assert_eq!(read(dir.path(), "inbox/scan.pdf").expect("reads"), bytes);
        assert_eq!(read(dir.path(), "inbox/memo.rtf").expect("reads"), bytes);
    }

    #[test]
    fn a_path_that_could_leave_the_folder_is_refused_as_unreadable() {
        let dir = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("temp dir");
        std::fs::write(outside.path().join("secret.txt"), b"x").expect("writes");
        let absolute = outside.path().join("secret.txt").display().to_string();

        for relative in [
            "../secret.txt",
            "inbox/../../secret.txt",
            absolute.as_str(),
            "",
            ".",
        ] {
            assert!(
                matches!(
                    read(dir.path(), relative),
                    Err(AppError::ViewerFileUnreadable)
                ),
                "{relative}"
            );
        }
    }

    #[test]
    fn an_absent_file_is_missing_not_unreadable() {
        let dir = tempfile::tempdir().expect("temp dir");

        assert!(matches!(
            read(dir.path(), "absent.pdf"),
            Err(AppError::ViewerFileMissing)
        ));
        assert!(matches!(
            read(dir.path(), "nowhere/absent.pdf"),
            Err(AppError::ViewerFileMissing)
        ));
    }

    #[test]
    fn a_folder_is_refused_as_unreadable() {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("inbox")).expect("creates");

        assert!(matches!(
            read(dir.path(), "inbox"),
            Err(AppError::ViewerFileUnreadable)
        ));
    }

    #[test]
    fn a_link_to_a_file_outside_the_folder_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("temp dir");
        let secret = outside.path().join("secret.txt");
        std::fs::write(&secret, b"x").expect("writes");
        if !make_link(&dir.path().join("shortcut.txt"), &secret, false) {
            eprintln!("skipped: this account cannot create a file link here");
            return;
        }

        assert!(matches!(
            read(dir.path(), "shortcut.txt"),
            Err(AppError::ViewerFileUnreadable)
        ));
    }

    #[test]
    fn a_link_in_the_middle_of_the_path_cannot_lead_out_of_the_folder() {
        let dir = tempfile::tempdir().expect("temp dir");
        let outside = tempfile::tempdir().expect("temp dir");
        std::fs::write(outside.path().join("secret.txt"), b"x").expect("writes");
        if !make_link(&dir.path().join("inbox"), outside.path(), true) {
            eprintln!("skipped: this account cannot create a folder link here");
            return;
        }

        assert!(matches!(
            read(dir.path(), "inbox/secret.txt"),
            Err(AppError::ViewerFileUnreadable)
        ));
    }

    #[test]
    fn a_link_that_stays_inside_the_folder_is_still_followed_in_the_middle() {
        // The containment check refuses a way out, not a folder that is itself reached by a link.
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("real")).expect("creates");
        std::fs::write(dir.path().join("real").join("note.pdf"), b"pdf").expect("writes");
        if !make_link(&dir.path().join("alias"), &dir.path().join("real"), true) {
            eprintln!("skipped: this account cannot create a folder link here");
            return;
        }

        assert_eq!(read(dir.path(), "alias/note.pdf").expect("reads"), b"pdf");
    }

    #[test]
    fn the_size_cap_is_enforced_and_a_file_at_the_cap_passes() {
        let dir = folder_with("big.pdf", &[7u8; 10]);

        assert!(matches!(
            read_with_cap(dir.path(), "big.pdf", 9),
            Err(AppError::ViewerFileTooLarge)
        ));
        assert_eq!(
            read_with_cap(dir.path(), "big.pdf", 10).expect("reads"),
            vec![7u8; 10]
        );
        assert_eq!(MAX_VIEWER_BYTES, 128 * 1024 * 1024);
    }

    #[test]
    fn windows_1252_text_comes_back_as_utf8() {
        let raw = b"1\n00:00:01,000 --> 00:00:02,000\nCaf\xE9 \xE0 bient\xF4t\n";
        let dir = folder_with("clip.srt", raw);

        let text = String::from_utf8(read(dir.path(), "clip.srt").expect("reads")).expect("utf8");

        assert_eq!(
            text,
            "1\n00:00:01,000 --> 00:00:02,000\nCaf\u{e9} \u{e0} bient\u{f4}t\n"
        );
        // The file on disk is exactly as it was.
        assert_eq!(
            std::fs::read(dir.path().join("clip.srt")).expect("reads"),
            raw
        );
    }

    #[test]
    fn utf16_with_a_byte_order_mark_is_decoded_in_both_orders() {
        let text = "Caf\u{e9}, \u{e0} bient\u{f4}t\r\n";
        for (name, little_endian) in [("little.txt", true), ("big.txt", false)] {
            let dir = folder_with(name, &utf16(text, little_endian));

            let read_back = read(dir.path(), name).expect("reads");

            assert_eq!(String::from_utf8(read_back).expect("utf8"), text, "{name}");
        }
    }

    #[test]
    fn a_utf8_byte_order_mark_is_dropped_and_plain_utf8_is_unchanged() {
        let with_mark = [&[0xEF, 0xBB, 0xBF][..], "Caf\u{e9}".as_bytes()].concat();
        let dir = folder_with("memo.md", &with_mark);
        assert_eq!(
            read(dir.path(), "memo.md").expect("reads"),
            "Caf\u{e9}".as_bytes()
        );

        let plain = folder_with("memo.md", "# Titre \u{e9}".as_bytes());
        assert_eq!(
            read(plain.path(), "memo.md").expect("reads"),
            "# Titre \u{e9}".as_bytes()
        );
    }

    #[test]
    fn every_text_like_extension_is_converted_whatever_its_case() {
        for name in ["a.txt", "a.md", "a.srt", "a.vtt", "a.eml", "a.csv", "A.TXT"] {
            let dir = folder_with(name, b"Caf\xE9");

            let text = read(dir.path(), name).expect("reads");

            assert_eq!(text, "Caf\u{e9}".as_bytes(), "{name}");
        }
    }

    #[test]
    fn an_eml_stays_raw_apart_from_its_encoding() {
        let raw =
            b"From: a@example.org\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nCaf=E9\r\n";
        let dir = folder_with("mail.eml", raw);

        assert_eq!(read(dir.path(), "mail.eml").expect("reads"), raw);
    }

    fn readable(name: &str, raw: &[u8]) -> String {
        let dir = folder_with(name, raw);
        String::from_utf8(read_readable_text(dir.path(), name).expect("reads")).expect("utf8")
    }

    #[test]
    fn a_mail_comes_back_decoded_with_its_header_fields_first() {
        let raw = b"From: a@example.org\r\nSubject: =?UTF-8?Q?Caf=C3=A9?=\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nBient=C3=B4t, \xC3\xA0 demain.\r\n";

        assert_eq!(
            readable("mail.eml", raw),
            "From: a@example.org\nSubject: Caf\u{e9}\n\nBient\u{f4}t, \u{e0} demain."
        );
    }

    #[test]
    fn a_mail_in_a_declared_8_bit_character_set_is_decoded_once() {
        let raw = b"From: a@example.org\r\nContent-Type: text/plain; charset=iso-8859-1\r\nContent-Transfer-Encoding: 8bit\r\n\r\nCaf\xE9\r\n";

        assert_eq!(
            readable("mail.eml", raw),
            "From: a@example.org\n\nCaf\u{e9}"
        );
    }

    #[test]
    fn subtitles_come_back_one_line_per_cue_in_both_formats() {
        let srt = b"1\n00:00:01,000 --> 00:00:04,000\nCaf\xE9 <i>noir</i>\n\n2\n00:01:05,250 --> 00:01:08,000\nBient\xF4t.\n";
        let vtt = b"WEBVTT\n\n00:00:01.000 --> 00:00:04.000\n<v Alice>Caf\xC3\xA9</v>\n";

        assert_eq!(
            readable("clip.srt", srt),
            "[00:00:01] Caf\u{e9} noir\n[00:01:05] Bient\u{f4}t."
        );
        assert_eq!(readable("clip.vtt", vtt), "[00:00:01] Alice: Caf\u{e9}");
        assert_eq!(readable("CLIP.SRT", srt), readable("clip.srt", srt));
    }

    #[test]
    fn utf16_subtitles_are_read_with_their_accents() {
        let dir = folder_with(
            "clip.srt",
            &utf16("1\r\n00:00:01,000 --> 00:00:02,000\r\nCaf\u{e9}\r\n", true),
        );

        let text = read_readable_text(dir.path(), "clip.srt").expect("reads");

        assert_eq!(
            String::from_utf8(text).expect("utf8"),
            "[00:00:01] Caf\u{e9}"
        );
    }

    #[test]
    fn a_file_the_reader_finds_empty_gives_no_bytes() {
        assert_eq!(readable("none.srt", b"1\nnot a timing line\n"), "");
        assert_eq!(readable("none.eml", b"no header here"), "");
    }

    #[test]
    fn the_readable_text_keeps_the_raw_read_s_guards() {
        let dir = folder_with("clip.srt", b"1\n00:00:01,000 --> 00:00:02,000\nHi\n");

        assert!(matches!(
            read_readable_text(dir.path(), "../clip.srt"),
            Err(AppError::ViewerFileUnreadable)
        ));
        assert!(matches!(
            read_readable_text(dir.path(), "absent.srt"),
            Err(AppError::ViewerFileMissing)
        ));
        assert!(matches!(
            read_readable_text_with_cap(dir.path(), "clip.srt", 5),
            Err(AppError::ViewerFileTooLarge)
        ));
    }

    #[test]
    fn a_format_without_a_readable_text_is_refused() {
        let dir = folder_with("memo.txt", b"hello");
        std::fs::write(dir.path().join("memo.pdf"), b"hello").expect("writes");

        for name in ["memo.txt", "memo.pdf"] {
            assert!(
                matches!(
                    read_readable_text(dir.path(), name),
                    Err(AppError::ViewerFileUnreadable)
                ),
                "{name}"
            );
        }
    }

    /// Compares the readable text of every `.eml`, `.srt` and `.vtt` of the owner's test folders
    /// with what the indexing path (`readers::read`) produces. Ignored by default, same two
    /// variables as the test below; run with `--ignored --nocapture` to see the figures.
    #[test]
    #[ignore = "reads the owner's test folders, named by two environment variables"]
    fn the_readable_text_equals_what_the_indexing_reads() {
        fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("lists").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files_under(&path, found);
                } else {
                    found.push(path);
                }
            }
        }

        let (mut compared, mut different, mut largest) = (0, 0, 0);
        for variable in ["VIEWER_TEST_DOCS_FOLDER", "VIEWER_TEST_DATA_FOLDER"] {
            let root = PathBuf::from(std::env::var(variable).expect(variable));
            let mut found = Vec::new();
            files_under(&root, &mut found);
            for path in found {
                let extension = path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(|extension| extension.to_ascii_lowercase())
                    .unwrap_or_default();
                if !READABLE_EXTENSIONS.contains(&extension.as_str()) {
                    continue;
                }
                let relative = path
                    .strip_prefix(&root)
                    .expect("inside")
                    .to_string_lossy()
                    .replace('\\', "/");
                let served = read_readable_text(&root, &relative).expect(&relative);
                let indexed = readers::read(&extension, &path)
                    .expect("a reader")
                    .expect("reads")
                    .join("\n");
                compared += 1;
                largest = largest.max(served.len());
                if served != indexed.as_bytes() {
                    different += 1;
                    println!("DIFFERENT: {relative}");
                }
            }
        }
        println!("compared: {compared}, different: {different}, largest: {largest} bytes");
        assert_eq!(different, 0);
    }

    /// Reads every file of the owner's two test folders through the real code path, read-only.
    /// Ignored by default: it needs the folders, given by two variables so that no path is written
    /// in the source. Run with `--ignored --nocapture` to see the figures.
    #[test]
    #[ignore = "reads the owner's test folders, named by two environment variables"]
    fn every_file_of_the_test_folders_reads_back() {
        fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).expect("lists").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files_under(&path, found);
                } else {
                    found.push(path);
                }
            }
        }

        let mut over_the_cap = 0;
        for variable in ["VIEWER_TEST_DOCS_FOLDER", "VIEWER_TEST_DATA_FOLDER"] {
            let root = PathBuf::from(std::env::var(variable).expect(variable));
            let mut found = Vec::new();
            files_under(&root, &mut found);
            for path in found {
                let relative = path
                    .strip_prefix(&root)
                    .expect("inside")
                    .to_string_lossy()
                    .replace('\\', "/");
                let on_disk = std::fs::read(&path).expect("reads");
                if on_disk.len() as u64 > MAX_VIEWER_BYTES {
                    over_the_cap += 1;
                    assert!(matches!(
                        read(&root, &relative),
                        Err(AppError::ViewerFileTooLarge)
                    ));
                    continue;
                }
                let served = read(&root, &relative).expect(&relative);
                if is_text_like(&path) {
                    assert_eq!(served, to_utf8(&on_disk), "{relative}");
                } else {
                    assert_eq!(served, on_disk, "{relative}");
                }
                println!(
                    "{variable}: {relative}: {} -> {} bytes",
                    on_disk.len(),
                    served.len()
                );
            }
        }
        println!("files over the cap: {over_the_cap}");
    }
}
