//! WebVTT transcripts (`.vtt`): the lines people said, each with the time it starts, as text.
//!
//! A cue is `[hh:mm:ss] speaker: words`. The header, the `NOTE`, `STYLE` and `REGION` blocks, the cue
//! identifiers and the formatting tags are dropped; a voice tag (`<v Alice>`) becomes the speaker. The time is
//! kept because a transcript is searched by what was said and read by when.

/// The text of a WebVTT file, one cue per line.
pub fn text(raw: &str) -> String {
    let normalised = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = Vec::new();
    for block in normalised.split("\n\n") {
        let block = block.trim_matches('\n');
        if block.is_empty() {
            continue;
        }
        let first = block.lines().next().unwrap_or("").trim_start();
        if first.starts_with("WEBVTT")
            || first.starts_with("NOTE")
            || first.starts_with("STYLE")
            || first.starts_with("REGION")
        {
            continue;
        }
        let mut cue = block.lines();
        let mut timing: Option<&str> = None;
        let mut spoken: Vec<String> = Vec::new();
        for line in cue.by_ref() {
            if timing.is_none() {
                if line.contains("-->") {
                    timing = Some(line);
                }
                // A line before the timing is the cue identifier.
                continue;
            }
            let cleaned = clean_cue_line(line);
            if !cleaned.is_empty() {
                spoken.push(cleaned);
            }
        }
        let (Some(timing), false) = (timing, spoken.is_empty()) else {
            continue;
        };
        let start = timing.split("-->").next().unwrap_or("").trim();
        lines.push(format!("[{}] {}", clock(start), spoken.join(" ")));
    }
    lines.join("\n")
}

/// `00:01:02.500` or `01:02.500` as `00:01:02` (the milliseconds say nothing a reader needs).
fn clock(start: &str) -> String {
    let main = start.split(['.', ',']).next().unwrap_or(start);
    let parts: Vec<&str> = main.split(':').collect();
    match parts.as_slice() {
        [minutes, seconds] => format!("00:{minutes}:{seconds}"),
        _ => main.to_string(),
    }
}

/// One cue line without its tags; a voice tag gives the speaker.
fn clean_cue_line(line: &str) -> String {
    let mut speaker: Option<String> = None;
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            out.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let tag = &rest[open + 1..open + close];
        if let Some(name) = tag.strip_prefix("v ").or_else(|| tag.strip_prefix("v.")) {
            if speaker.is_none() {
                let name = name.split_once(' ').map_or(name, |(_, tail)| tail);
                speaker = Some(name.trim().to_string());
            }
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    let body = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .trim()
        .to_string();
    match speaker {
        Some(name) if !name.is_empty() && !body.is_empty() => format!("{name}: {body}"),
        _ => body,
    }
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
    fn a_file_with_no_cue_is_empty() {
        assert_eq!(text("WEBVTT\n\nNOTE nothing was said\n"), "");
        assert_eq!(text(""), "");
    }
}
