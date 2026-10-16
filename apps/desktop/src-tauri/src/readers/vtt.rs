//! WebVTT transcripts (`.vtt`). The header, the `NOTE`, `STYLE` and `REGION` blocks, the cue identifiers and the
//! formatting tags are dropped; a voice tag (`<v Alice>`) becomes the speaker.

use super::cues;

/// The text of a WebVTT file, one cue per line.
pub fn text(raw: &str) -> String {
    let normalised = cues::normalise(raw);
    cues::render(&cues::parse(
        &normalised,
        &["WEBVTT", "NOTE", "STYLE", "REGION"],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transcript_becomes_one_line_per_cue_with_its_start_time() {
        let raw = "WEBVTT - the weekly call\n\nNOTE written by hand\n\n1\n00:00:01.000 --> 00:00:04.000\n<v Alice>Welcome to the call.\n\n2\n00:01:05.250 --> 00:01:08.000\nThe budget is <b>CHF 9&nbsp;800</b>.\nIt was approved.\n\n";
        assert_eq!(
            text(raw),
            "[00:00:01] Alice: Welcome to the call.\n[00:01:05] The budget is CHF 9 800. It was approved."
        );
    }

    #[test]
    fn windows_line_ends_and_short_clocks_are_read() {
        let raw = "WEBVTT\r\n\r\n01:02.500 --> 01:04.000\r\nHello there.\r\n";
        assert_eq!(text(raw), "[00:01:02] Hello there.");
    }

    #[test]
    fn style_and_region_blocks_and_cue_settings_are_ignored() {
        let raw = "WEBVTT\n\nSTYLE\n::cue { color: red }\n\nREGION\nid:fred\n\nintro\n00:00:02.000 --> 00:00:05.000 align:start\nOpening words.\n";
        assert_eq!(text(raw), "[00:00:02] Opening words.");
    }

    #[test]
    fn a_file_with_no_cue_is_empty() {
        assert_eq!(text("WEBVTT\n\nNOTE nothing was said\n"), "");
        assert_eq!(text(""), "");
    }
}
