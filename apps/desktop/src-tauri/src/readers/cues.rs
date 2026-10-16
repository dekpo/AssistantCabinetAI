//! What WebVTT and SubRip share: a subtitle file is a list of cues, each a start time and the words said.
//! One cue is written as `[hh:mm:ss] words` (`speaker: words` when the format names the speaker), one per line.
//! The time is kept because a transcript is searched by what was said and read by when.

/// One cue: when it starts, as the file wrote it, and its text lines with their tags still in.
pub struct Cue<'a> {
    pub start: &'a str,
    pub lines: Vec<&'a str>,
}

/// The text with Unix line ends and no byte order mark, so a file saved on Windows is read like any other.
pub fn normalise(raw: &str) -> String {
    raw.trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

/// The cues of `normalised` text. A cue is a block of lines between blank lines with a timing line (`a --> b`);
/// the lines before it are its identifier or number and are dropped, the lines after it are its words. A block
/// whose first line starts with one of `skip` (a header, a note) is not a cue, and neither is a block with no
/// timing line.
pub fn parse<'a>(normalised: &'a str, skip: &[&str]) -> Vec<Cue<'a>> {
    let mut cues = Vec::new();
    for block in normalised.split("\n\n") {
        let first = block
            .trim_matches('\n')
            .lines()
            .next()
            .unwrap_or("")
            .trim_start();
        if first.is_empty() || skip.iter().any(|prefix| first.starts_with(prefix)) {
            continue;
        }
        let mut lines = block.lines().skip_while(|line| !line.contains("-->"));
        let Some(timing) = lines.next() else {
            continue;
        };
        let start = timing.split("-->").next().unwrap_or("").trim();
        cues.push(Cue {
            start,
            lines: lines.collect(),
        });
    }
    cues
}

/// The cues written as text, one cue per line. A cue with no words is left out.
pub fn render(cues: &[Cue]) -> String {
    let mut out: Vec<String> = Vec::new();
    for cue in cues {
        let spoken: Vec<String> = cue
            .lines
            .iter()
            .map(|line| clean_line(line))
            .filter(|line| !line.is_empty())
            .collect();
        if !spoken.is_empty() {
            out.push(format!("[{}] {}", clock(cue.start), spoken.join(" ")));
        }
    }
    out.join("\n")
}

/// The start time as `hh:mm:ss`: milliseconds dropped, a `mm:ss` start given its hours.
pub fn clock(start: &str) -> String {
    let main = start.split(['.', ',']).next().unwrap_or(start);
    let parts: Vec<&str> = main.split(':').collect();
    match parts.as_slice() {
        [minutes, seconds] => format!("00:{minutes}:{seconds}"),
        _ => main.to_string(),
    }
}

/// One cue line without its tags and positioning codes; a WebVTT voice tag gives the speaker.
pub fn clean_line(line: &str) -> String {
    let mut speaker: Option<String> = None;
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find(['<', '{']) {
        out.push_str(&rest[..open]);
        let tail = &rest[open..];
        let (closer, body_start) = if tail.starts_with('<') {
            ('>', 1)
        } else {
            ('}', 1)
        };
        let after = &tail[body_start..];
        let looks_like_markup = if closer == '>' {
            after.starts_with(|c: char| c.is_ascii_alphabetic() || c == '/')
        } else {
            after.starts_with('\\')
        };
        match (looks_like_markup, tail.find(closer)) {
            (true, Some(close)) => {
                if closer == '>' && speaker.is_none() {
                    speaker = voice_name(&tail[1..close]);
                }
                rest = &tail[close + 1..];
            }
            _ => {
                out.push_str(&tail[..1]);
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    let body = decode_entities(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    match speaker {
        Some(name) if !name.is_empty() && !body.is_empty() => format!("{name}: {body}"),
        _ => body,
    }
}

/// The speaker of a WebVTT voice tag (`v Alice`, `v.loud Bob Smith`), or `None` for any other tag.
fn voice_name(tag: &str) -> Option<String> {
    let rest = tag.strip_prefix('v')?;
    let name = if let Some(classes) = rest.strip_prefix('.') {
        classes.split_once(' ')?.1
    } else {
        rest.strip_prefix(' ')?
    };
    Some(name.trim().to_string())
}

fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let decoded = [
            ("&amp;", "&"),
            ("&lt;", "<"),
            ("&gt;", ">"),
            ("&nbsp;", " "),
            ("&quot;", "\""),
            ("&apos;", "'"),
        ]
        .iter()
        .find(|(entity, _)| tail.starts_with(entity));
        match decoded {
            Some((entity, plain)) => {
                out.push_str(plain);
                rest = &tail[entity.len()..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clock_loses_its_milliseconds_and_gains_its_hours() {
        assert_eq!(clock("00:00:01,000"), "00:00:01");
        assert_eq!(clock("00:01:02.500"), "00:01:02");
        assert_eq!(clock("01:02.500"), "00:01:02");
        assert_eq!(clock("12:34:56,789"), "12:34:56");
    }

    #[test]
    fn tags_and_positioning_codes_are_dropped_and_entities_read() {
        assert_eq!(clean_line("<i>Hello</i> {\\an8}there"), "Hello there");
        assert_eq!(
            clean_line("<font color=\"#ff0000\">CHF 9&nbsp;800</font> &amp; more"),
            "CHF 9 800 & more"
        );
        assert_eq!(clean_line("2 < 3 and 4 > 3"), "2 < 3 and 4 > 3");
    }

    #[test]
    fn a_voice_tag_gives_the_speaker() {
        assert_eq!(clean_line("<v Alice>Welcome."), "Alice: Welcome.");
        assert_eq!(clean_line("<v.loud Bob Smith>Hi</v>"), "Bob Smith: Hi");
    }

    #[test]
    fn a_cue_with_no_words_is_left_out() {
        let cues = [
            Cue {
                start: "00:00:01,000",
                lines: vec!["<i></i>"],
            },
            Cue {
                start: "00:00:03,000",
                lines: vec!["Yes.", "No."],
            },
        ];
        assert_eq!(render(&cues), "[00:00:03] Yes. No.");
    }
}
