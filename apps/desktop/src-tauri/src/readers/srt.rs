//! SubRip subtitles (`.srt`). The cue number, the `<i>`, `<b>`, `<u>` and `<font>` tags and the `{\an8}`
//! positioning codes are dropped. The cue code is shared with WebVTT (`readers::cues`).

use super::cues;

/// The text of a SubRip file, one cue per line.
pub fn text(raw: &str) -> String {
    let normalised = cues::normalise(raw);
    cues::render(&cues::parse(&normalised, &[]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subtitle_file_becomes_one_line_per_cue_with_its_start_time() {
        let raw = "1\n00:00:01,000 --> 00:00:04,000\nWelcome to the call.\n\n2\n00:01:05,250 --> 00:01:08,000\n<i>The budget is</i> <b>CHF 9&nbsp;800</b>.\n{\\an8}It was approved.\n\n";
        assert_eq!(
            text(raw),
            "[00:00:01] Welcome to the call.\n[00:01:05] The budget is CHF 9 800. It was approved."
        );
    }

    #[test]
    fn windows_line_ends_and_a_byte_order_mark_are_read() {
        let raw = "\u{feff}1\r\n00:00:01,000 --> 00:00:02,000\r\nHello.\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\nGoodbye.\r\n";
        assert_eq!(text(raw), "[00:00:01] Hello.\n[00:00:03] Goodbye.");
    }

    #[test]
    fn a_cue_without_a_number_and_a_number_inside_a_cue_are_read() {
        let raw = "00:00:10,000 --> 00:00:12,000\n42\nThe invoice number is 2026.\n";
        assert_eq!(text(raw), "[00:00:10] 42 The invoice number is 2026.");
    }

    #[test]
    fn extra_blank_lines_between_cues_are_harmless() {
        let raw = "1\n00:00:01,000 --> 00:00:02,000\nOne.\n\n\n\n2\n00:00:03,000 --> 00:00:04,000\nTwo.\n";
        assert_eq!(text(raw), "[00:00:01] One.\n[00:00:03] Two.");
    }

    #[test]
    fn a_file_with_no_cue_is_empty() {
        assert_eq!(text("1\nnot a timing line\nwords\n"), "");
        assert_eq!(text(""), "");
    }
}
