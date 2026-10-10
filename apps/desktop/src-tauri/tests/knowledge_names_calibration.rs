//! Measures the phonetic keys of `knowledge::phonetic` on public lists of real names (KB lot 2 bis).
//!
//! Two variants of the same checks:
//!
//! - `committed_samples_hold` always runs, on the small samples under
//!   `tests/fixtures/knowledge/names-sample-*.tsv` (the most frequent names of each list, with
//!   counts, and for first names the number of men and women).
//! - `full_lists_calibration` runs on the complete lists when `scripts/fetch_name_lists.py` has
//!   downloaded them (`data/names/`, git-ignored, or the folder in `ACAI_NAMES_DIR`); otherwise it
//!   prints a notice and passes. It prints the evidence tables of the lot report. Run it with:
//!
//! ```text
//! cargo test --test knowledge_names_calibration -- --nocapture
//! ```
//!
//! `ACAI_NAMES_EVIDENCE=<file>` also writes the tables to a file. `ACAI_NAMES_WRITE_SAMPLES=1`
//! (with `--ignored`) rewrites the committed samples from the full lists.
//!
//! Sources and licences: `docs/DATA-SOURCES.md`. The lists are aggregates of surnames and first
//! names; names are printed here and in the review file only, never by the product.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use assistant_cabinet_ai_lib::knowledge::normalize::{fold, normalize_name, TitleSet};
use assistant_cabinet_ai_lib::knowledge::phonetic::{
    bounded_damerau_levenshtein, encoder_for_locale, PhoneticEncoder,
};
use serde_json::Value;

const SAMPLE_SIZE: usize = 300;
const TOP_NAMES: usize = 5_000;
const SELECTIONS: usize = 2_000;
const SELECTION_SIZE: usize = 30;
const MENTIONS_PER_SELECTION: usize = 20;
const LENGTHS: [usize; 4] = [3, 4, 5, 6];
/// A first name counts as gendered when at least this share of its bearers is of one sex.
const SEX_SHARE: f64 = 0.9;

// ----------------------------------------------------------------------------- data

#[derive(Clone, Debug)]
struct Raw {
    text: String,
    count: u64,
    male: u64,
    female: u64,
}

#[derive(Clone, Debug)]
struct Name {
    text: String,
    folded: String,
    count: u64,
    male: u64,
    female: u64,
    key: String,
}

impl Name {
    fn letters(&self) -> usize {
        self.folded.chars().filter(|c| c.is_alphanumeric()).count()
    }

    /// `Some('M')` or `Some('F')` when at least `SEX_SHARE` of the bearers are of one sex.
    fn sex(&self, minimum_total: u64) -> Option<char> {
        let total = self.male + self.female;
        if total < minimum_total {
            return None;
        }
        let share = |n: u64| n as f64 / total as f64;
        if share(self.male) >= SEX_SHARE {
            Some('M')
        } else if share(self.female) >= SEX_SHARE {
            Some('F')
        } else {
            None
        }
    }
}

struct Language {
    encoder: Box<dyn PhoneticEncoder>,
    set: TitleSet,
}

fn words(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` is missing"))
        .iter()
        .map(|word| word.as_str().expect("a string").to_string())
        .collect()
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("knowledge")
}

fn french() -> Language {
    let text = std::fs::read_to_string(fixtures_dir().join("name-words-fr.json")).unwrap();
    let file: Value = serde_json::from_str(&text).unwrap();
    Language {
        encoder: encoder_for_locale("fr-FR"),
        set: TitleSet::new()
            .with_titles(words(&file, "titles"))
            .with_weak_titles(words(&file, "weak_titles"))
            .with_particles(words(&file, "particles"))
            .with_stop_words(words(&file, "stop_words")),
    }
}

fn english() -> Language {
    Language {
        encoder: encoder_for_locale("en-US"),
        set: TitleSet::new(),
    }
}

fn names_dir() -> PathBuf {
    match std::env::var_os("ACAI_NAMES_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("data")
            .join("names"),
    }
}

fn build(raw: Vec<Raw>, language: &Language) -> Vec<Name> {
    let mut merged: HashMap<String, Name> = HashMap::new();
    for entry in raw {
        let folded = fold(&entry.text);
        if folded.chars().all(|c| !c.is_alphanumeric()) {
            continue;
        }
        merged
            .entry(folded.clone())
            .and_modify(|name| {
                name.count += entry.count;
                name.male += entry.male;
                name.female += entry.female;
            })
            .or_insert(Name {
                text: entry.text,
                folded,
                count: entry.count,
                male: entry.male,
                female: entry.female,
                key: String::new(),
            });
    }
    let mut names: Vec<Name> = merged.into_values().collect();
    names.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.folded.cmp(&b.folded)));
    for name in &mut names {
        name.key = key_of(language, &name.text);
    }
    names
}

fn key_of(language: &Language, text: &str) -> String {
    language
        .encoder
        .encode_name(&normalize_name(text, &language.set))
}

fn title_case(upper: &str) -> String {
    let mut out = String::new();
    let mut start = true;
    for c in upper.chars() {
        if start {
            out.extend(c.to_uppercase());
        } else {
            out.extend(c.to_lowercase());
        }
        start = !c.is_alphanumeric();
    }
    out
}

/// Insee "Fichier des noms": tab separated, one count per decade of birth.
fn load_insee_surnames(dir: &Path) -> Option<Vec<Raw>> {
    let path = dir
        .join("insee-noms")
        .join("extracted")
        .join("noms2008nat_txt.txt");
    let text = std::fs::read_to_string(path).ok()?;
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        let mut cells = line.split('\t');
        let name = cells.next()?.trim().to_string();
        // The file ends with a row that gathers every rarer name.
        if name.is_empty() || name == "AUTRES NOMS" {
            continue;
        }
        let count: u64 = cells
            .filter_map(|cell| cell.trim().parse::<u64>().ok())
            .sum();
        rows.push(Raw {
            text: name,
            count,
            male: 0,
            female: 0,
        });
    }
    Some(rows)
}

/// Insee "Fichier des prénoms": `sexe;prenom;periode;valeur`, UTF-8, upper case with accents.
/// Sex 1 is male, 2 female.
fn load_insee_first_names(dir: &Path) -> Option<Vec<Raw>> {
    let path = dir
        .join("insee-prenoms")
        .join("extracted")
        .join("prenoms-2024-nat.csv");
    let text = String::from_utf8(std::fs::read(path).ok()?).ok()?;
    let mut by_name: HashMap<String, (u64, u64)> = HashMap::new();
    for line in text.lines().skip(1) {
        let cells: Vec<&str> = line.split(';').collect();
        if cells.len() < 4 || cells[1].starts_with('_') {
            continue;
        }
        let value: u64 = cells[3].trim().parse().unwrap_or(0);
        let entry = by_name.entry(cells[1].to_string()).or_default();
        match cells[0] {
            "1" => entry.0 += value,
            "2" => entry.1 += value,
            _ => {}
        }
    }
    Some(
        by_name
            .into_iter()
            .map(|(text, (male, female))| Raw {
                text,
                count: male + female,
                male,
                female,
            })
            .collect(),
    )
}

/// US Census 2010 surnames: `name,rank,count,...`.
fn load_census_surnames(dir: &Path) -> Option<Vec<Raw>> {
    let path = dir
        .join("census-2010")
        .join("extracted")
        .join("Names_2010Census.csv");
    let text = std::fs::read_to_string(path).ok()?;
    let mut rows = Vec::new();
    for line in text.lines().skip(1) {
        let cells: Vec<&str> = line.split(',').collect();
        if cells.len() < 3 || cells[0] == "ALL OTHER NAMES" {
            continue;
        }
        let Ok(count) = cells[2].trim().parse::<u64>() else {
            continue;
        };
        rows.push(Raw {
            text: cells[0].to_string(),
            count,
            male: 0,
            female: 0,
        });
    }
    Some(rows)
}

/// SSA national baby names: one file per year, `name,sex,count`; summed over every year.
fn load_ssa_first_names(dir: &Path) -> Option<Vec<Raw>> {
    let folder = dir.join("ssa-babynames").join("extracted");
    let mut by_name: HashMap<String, (u64, u64)> = HashMap::new();
    let mut files = 0;
    for entry in std::fs::read_dir(folder).ok()? {
        let path = entry.ok()?.path();
        let file_name = path.file_name()?.to_string_lossy().to_string();
        if !(file_name.starts_with("yob") && file_name.ends_with(".txt")) {
            continue;
        }
        files += 1;
        for line in std::fs::read_to_string(&path).ok()?.lines() {
            let cells: Vec<&str> = line.split(',').collect();
            if cells.len() < 3 {
                continue;
            }
            let value: u64 = cells[2].trim().parse().unwrap_or(0);
            let entry = by_name.entry(cells[0].to_string()).or_default();
            match cells[1] {
                "M" => entry.0 += value,
                "F" => entry.1 += value,
                _ => {}
            }
        }
    }
    if files == 0 {
        return None;
    }
    Some(
        by_name
            .into_iter()
            .map(|(text, (male, female))| Raw {
                text,
                count: male + female,
                male,
                female,
            })
            .collect(),
    )
}

struct Lists {
    fr_surnames: Vec<Name>,
    fr_first: Vec<Name>,
    en_surnames: Vec<Name>,
    en_first: Vec<Name>,
}

fn load_full(fr: &Language, en: &Language) -> Option<Lists> {
    let dir = names_dir();
    Some(Lists {
        fr_surnames: build(load_insee_surnames(&dir)?, fr),
        fr_first: build(load_insee_first_names(&dir)?, fr),
        en_surnames: build(load_census_surnames(&dir)?, en),
        en_first: build(load_ssa_first_names(&dir)?, en),
    })
}

// ----------------------------------------------------------------------------- samples

const SAMPLE_FILES: [(&str, &str); 4] = [
    ("fr_surnames", "names-sample-fr-surnames.tsv"),
    ("fr_first", "names-sample-fr-first-names.tsv"),
    ("en_surnames", "names-sample-en-surnames.tsv"),
    ("en_first", "names-sample-en-first-names.tsv"),
];

fn read_sample(file: &str, language: &Language) -> Vec<Name> {
    let path = fixtures_dir().join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let mut raw = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let cells: Vec<&str> = line.split('\t').collect();
        raw.push(Raw {
            text: cells[0].to_string(),
            count: cells[1].parse().unwrap(),
            male: cells[2].parse().unwrap(),
            female: cells[3].parse().unwrap(),
        });
    }
    build(raw, language)
}

fn load_samples(fr: &Language, en: &Language) -> Lists {
    Lists {
        fr_surnames: read_sample(SAMPLE_FILES[0].1, fr),
        fr_first: read_sample(SAMPLE_FILES[1].1, fr),
        en_surnames: read_sample(SAMPLE_FILES[2].1, en),
        en_first: read_sample(SAMPLE_FILES[3].1, en),
    }
}

fn sample_text(names: &[Name], source: &str) -> String {
    let mut out = format!(
        "# {source}\n# name<TAB>count<TAB>men<TAB>women; the {SAMPLE_SIZE} most frequent names, derived by tests/knowledge_names_calibration.rs\n"
    );
    for name in names.iter().take(SAMPLE_SIZE) {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}",
            name.text, name.count, name.male, name.female
        );
    }
    out
}

// ----------------------------------------------------------------------------- measures

/// Every pair of names (indexes, smaller first) at Damerau-Levenshtein distance 1, found without
/// comparing every pair: deletions and swaps are looked up, substitutions share a deletion variant.
fn near_pairs(names: &[Name]) -> Vec<(usize, usize)> {
    let index: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.folded.as_str(), i))
        .collect();
    let mut found: HashSet<(usize, usize)> = HashSet::new();
    let mut variants: HashMap<String, Vec<usize>> = HashMap::new();
    let ordered = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };

    for (i, name) in names.iter().enumerate() {
        let chars: Vec<char> = name.folded.chars().collect();
        for skip in 0..chars.len() {
            let variant: String = chars
                .iter()
                .enumerate()
                .filter(|(at, _)| *at != skip)
                .map(|(_, c)| *c)
                .collect();
            if let Some(&j) = index.get(variant.as_str()) {
                found.insert(ordered(i, j));
            }
            variants.entry(variant).or_default().push(i);
        }
        for at in 0..chars.len().saturating_sub(1) {
            if chars[at] == chars[at + 1] {
                continue;
            }
            let mut swapped = chars.clone();
            swapped.swap(at, at + 1);
            let swapped: String = swapped.into_iter().collect();
            if let Some(&j) = index.get(swapped.as_str()) {
                found.insert(ordered(i, j));
            }
        }
    }
    for group in variants.values().filter(|group| group.len() > 1) {
        for (position, &a) in group.iter().enumerate() {
            for &b in &group[position + 1..] {
                if bounded_damerau_levenshtein(&names[a].folded, &names[b].folded, 1) == Some(1) {
                    found.insert(ordered(a, b));
                }
            }
        }
    }
    let mut pairs: Vec<(usize, usize)> = found.into_iter().collect();
    pairs.sort_unstable();
    pairs
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

struct Report(String);

impl Report {
    fn line(&mut self, text: impl AsRef<str>) {
        self.0.push_str(text.as_ref());
        self.0.push('\n');
    }

    fn blank(&mut self) {
        self.0.push('\n');
    }
}

fn health(report: &mut Report, label: &str, names: &[Name], language: &Language) {
    let empty = names.iter().filter(|n| n.key.is_empty()).count();
    let non_ascii = names.iter().filter(|n| !n.key.is_ascii()).count();
    let unstable = names
        .iter()
        .filter(|n| key_of(language, &n.text) != n.key)
        .count();

    let mut groups: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, name) in names.iter().enumerate() {
        groups.entry(name.key.as_str()).or_default().push(i);
    }
    let mut sizes: Vec<usize> = groups.values().map(Vec::len).collect();
    sizes.sort_unstable();
    let bands: [(&str, usize, usize); 7] = [
        ("1", 1, 1),
        ("2", 2, 2),
        ("3-5", 3, 5),
        ("6-10", 6, 10),
        ("11-50", 11, 50),
        ("51-200", 51, 200),
        (">200", 201, usize::MAX),
    ];
    let mut cells = Vec::new();
    for (name, low, high) in bands {
        let groups_in_band = sizes.iter().filter(|s| **s >= low && **s <= high).count();
        let names_in_band: usize = sizes.iter().filter(|s| **s >= low && **s <= high).sum();
        cells.push(format!(
            "{name}: {groups_in_band} keys / {names_in_band} names"
        ));
    }
    report.line(format!("#### {label}"));
    report.blank();
    report.line(format!(
        "{} names, {} keys (mean {:.2} names per key); empty keys {empty}; non-ASCII keys {non_ascii}; keys that changed on a second run {unstable}; largest group {}.",
        names.len(),
        groups.len(),
        names.len() as f64 / groups.len().max(1) as f64,
        sizes.last().copied().unwrap_or(0)
    ));
    report.blank();
    report.line(format!("Group sizes: {}.", cells.join("; ")));
    report.blank();
    if empty > 0 {
        let shown: Vec<String> = names
            .iter()
            .filter(|n| n.key.is_empty())
            .take(12)
            .map(|n| title_case(&n.text))
            .collect();
        report.line(format!("Names with an empty key: {}.", shown.join(", ")));
        report.blank();
    }
    report.line("| key | names | most frequent members |");
    report.line("| --- | --- | --- |");
    let mut biggest: Vec<(&&str, &Vec<usize>)> = groups.iter().collect();
    biggest.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
    for (key, members) in biggest.into_iter().take(20) {
        let shown: Vec<String> = members
            .iter()
            .take(6)
            .map(|i| title_case(&names[*i].text))
            .collect();
        report.line(format!(
            "| `{key}` | {} | {} |",
            members.len(),
            shown.join(", ")
        ));
    }
    report.blank();
}

/// Pairs at distance 1 that pass the gate "same key, both words of at least `length` letters".
fn gate_passes(names: &[Name], pairs: &[(usize, usize)], length: usize) -> Vec<(usize, usize)> {
    pairs
        .iter()
        .copied()
        .filter(|&(a, b)| {
            names[a].key == names[b].key
                && !names[a].key.is_empty()
                && names[a].letters().min(names[b].letters()) >= length
        })
        .collect()
}

fn gate_table(report: &mut Report, label: &str, names: &[Name]) {
    let pairs = near_pairs(names);
    let top = TOP_NAMES.min(names.len());
    let top_pairs: Vec<(usize, usize)> = pairs.iter().copied().filter(|&(_, b)| b < top).collect();

    report.line(format!("#### {label}"));
    report.blank();
    report.line(format!(
        "{} pairs of names at edit distance 1 in the whole list, {} among the {top} most frequent.",
        pairs.len(),
        top_pairs.len()
    ));
    report.blank();
    report.line("| min length | pairs passing, whole list | pairs passing, top names | pairs at distance 1 with another key, top names |");
    report.line("| --- | --- | --- | --- |");
    for length in LENGTHS {
        let whole = gate_passes(names, &pairs, length).len();
        let passing_top = gate_passes(names, &top_pairs, length);
        let apart = top_pairs
            .iter()
            .filter(|&&(a, b)| {
                names[a].key != names[b].key && names[a].letters().min(names[b].letters()) >= length
            })
            .count();
        report.line(format!(
            "| {length} | {whole} | {} | {apart} |",
            passing_top.len()
        ));
    }
    report.blank();

    // Usage: a selection of 30 names drawn by frequency from the top names.
    let weights: Vec<u64> = names.iter().take(top).map(|n| n.count.max(1)).collect();
    let cumulative: Vec<u64> = weights
        .iter()
        .scan(0u64, |sum, w| {
            *sum += w;
            Some(*sum)
        })
        .collect();
    let total = *cumulative.last().unwrap_or(&1);
    let draw = |rng: &mut Lcg| -> usize {
        let target = (rng.next() as u128 * total as u128 / (1u128 << 31)) as u64;
        cumulative.partition_point(|&c| c <= target).min(top - 1)
    };

    let mut examples = gate_passes(names, &top_pairs, 4);
    examples.sort_by_key(|&(a, b)| std::cmp::Reverse(names[a].count + names[b].count));
    let shown: Vec<String> = examples
        .iter()
        .take(14)
        .map(|&(a, b)| {
            format!(
                "{} / {}",
                title_case(&names[a].text),
                title_case(&names[b].text)
            )
        })
        .collect();
    report.line(format!(
        "Most frequent pairs that pass the gate at 4 letters: {}.",
        shown.join("; ")
    ));
    report.blank();

    report.line(format!(
        "Usage, {SELECTIONS} random selections of {SELECTION_SIZE} names drawn by frequency from the {top} most frequent. *Inside*: pairs of the selection that the gate would call possible matches of each other. *Outside*: {MENTIONS_PER_SELECTION} mentions of other frequent names per selection, each offered the selected names that pass the gate (a mention of a real name that is not selected and still gets a suggestion). The rate is per 100 such mentions."
    ));
    report.blank();
    report.line("| min length | inside, pairs per selection | outside, suggestions per 100 mentions | selections with at least one outside suggestion |");
    report.line("| --- | --- | --- | --- |");
    for length in LENGTHS {
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); top];
        for &(a, b) in &gate_passes(names, &top_pairs, length) {
            adjacency[a].push(b);
            adjacency[b].push(a);
        }
        // The same selections at every length, so the columns compare.
        let mut rng = Lcg(2026);
        let (mut inside, mut outside, mut selections_hit) = (0usize, 0usize, 0usize);
        for _ in 0..SELECTIONS {
            let mut selected: HashSet<usize> = HashSet::new();
            while selected.len() < SELECTION_SIZE.min(top) {
                selected.insert(draw(&mut rng));
            }
            for &a in &selected {
                inside += adjacency[a]
                    .iter()
                    .filter(|b| selected.contains(b) && **b > a)
                    .count();
            }
            let mut hit = false;
            let mut mentions = 0;
            while mentions < MENTIONS_PER_SELECTION {
                let mention = draw(&mut rng);
                if selected.contains(&mention) {
                    continue;
                }
                mentions += 1;
                for &neighbour in &adjacency[mention] {
                    if selected.contains(&neighbour) {
                        outside += 1;
                        hit = true;
                    }
                }
            }
            selections_hit += usize::from(hit);
        }
        report.line(format!(
            "| {length} | {:.3} | {:.2} | {:.1}% |",
            inside as f64 / SELECTIONS as f64,
            100.0 * outside as f64 / (SELECTIONS * MENTIONS_PER_SELECTION) as f64,
            100.0 * selections_hit as f64 / SELECTIONS as f64,
        ));
    }
    report.blank();
}

/// Pairs of opposite-sex first names that are the same sound in the language, so no rule can
/// separate them ("Billy" and "Billie"): `known_shared` of the contract file, folded and ordered.
fn known_shared(file: &str) -> HashSet<(String, String)> {
    let text = std::fs::read_to_string(fixtures_dir().join(file)).unwrap();
    let contract: Value = serde_json::from_str(&text).unwrap();
    contract["known_shared"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    let a = fold(row[0].as_str().unwrap());
                    let b = fold(row[1].as_str().unwrap());
                    (a.clone().min(b.clone()), a.max(b))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Rule: two clearly gendered first names of opposite sex never share a key. Returns the violating
/// pairs (indexes) and prints the counts.
fn sex_rule(
    report: &mut Report,
    label: &str,
    names: &[Name],
    minimum_total: u64,
    known: &HashSet<(String, String)>,
) -> Vec<(usize, usize)> {
    let mut men: Vec<usize> = Vec::new();
    let mut women: Vec<usize> = Vec::new();
    for (i, name) in names.iter().enumerate() {
        match name.sex(minimum_total) {
            Some('M') => men.push(i),
            Some('F') => women.push(i),
            _ => {}
        }
    }
    let mut women_by_key: HashMap<&str, Vec<usize>> = HashMap::new();
    for &w in &women {
        women_by_key
            .entry(names[w].key.as_str())
            .or_default()
            .push(w);
    }
    let mut violations = Vec::new();
    for &m in &men {
        for &w in women_by_key
            .get(names[m].key.as_str())
            .into_iter()
            .flatten()
        {
            violations.push((m, w));
        }
    }
    violations.sort_by_key(|&(m, w)| std::cmp::Reverse(names[m].count + names[w].count));

    let is_known = |m: usize, w: usize| {
        let (a, b) = (names[m].folded.clone(), names[w].folded.clone());
        known.contains(&(a.clone().min(b.clone()), a.max(b)))
    };
    let accepted = violations.iter().filter(|&&(m, w)| is_known(m, w)).count();
    report.line(format!(
        "{label}: names with at least {minimum_total} bearers and {:.0}% of one sex: {} masculine, {} feminine; the rule covers {} opposite-sex pairs; pairs sharing a key: {}, of which {accepted} are listed as known homophones of the contract file and {} are not.",
        SEX_SHARE * 100.0,
        men.len(),
        women.len(),
        men.len() * women.len(),
        violations.len(),
        violations.len() - accepted
    ));
    report.blank();
    if !violations.is_empty() {
        report.line("| masculine | feminine | key | known homophones |");
        report.line("| --- | --- | --- | --- |");
        for &(m, w) in violations.iter().take(100) {
            report.line(format!(
                "| {} | {} | `{}` | {} |",
                title_case(&names[m].text),
                title_case(&names[w].text),
                names[m].key,
                if is_known(m, w) { "yes" } else { "NO" }
            ));
        }
        report.blank();
    }
    violations
        .into_iter()
        .filter(|&(m, w)| !is_known(m, w))
        .collect()
}

/// Whether two names at edit distance 1 differ by an edit that spelling variants usually make: a
/// doubled letter, an added or dropped `h`, `y`/`i`, `c`/`k`, `k`/`q`, `s`/`z`, `c`/`s`, `g`/`j`,
/// `d`/`t`. Used only to pick which candidate misses are worth the owner's time: most pairs at
/// distance 1 are simply different names (Martin and Marin).
fn is_spelling_variant(a: &str, b: &str) -> bool {
    fn collapse(text: &str) -> String {
        let mut out = String::new();
        for c in text.chars() {
            if !out.ends_with(c) {
                out.push(c);
            }
        }
        out
    }
    if collapse(a) == collapse(b) {
        return true;
    }
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (left, right) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    match (left, right) {
        ([c], []) | ([], [c]) => *c == 'h',
        ([x], [y]) => {
            let pair = |p: char, q: char| (*x == p && *y == q) || (*x == q && *y == p);
            pair('i', 'y')
                || pair('c', 'k')
                || pair('k', 'q')
                || pair('s', 'z')
                || pair('c', 's')
                || pair('g', 'j')
                || pair('d', 't')
        }
        _ => false,
    }
}

/// Distance-1 pairs among the top names, split by whether they share a key; plus pairs that share a
/// key at a larger distance.
fn misses_and_hits(report: &mut Report, label: &str, names: &[Name]) {
    let top = TOP_NAMES.min(names.len());
    let head = &names[..top];
    let pairs = near_pairs(head);
    let combined = |a: usize, b: usize| head[a].count + head[b].count;

    let different_key: Vec<(usize, usize)> = pairs
        .iter()
        .copied()
        .filter(|&(a, b)| head[a].key != head[b].key)
        .collect();
    let mut misses: Vec<(usize, usize)> = different_key
        .iter()
        .copied()
        .filter(|&(a, b)| is_spelling_variant(&head[a].folded, &head[b].folded))
        .collect();
    misses.sort_by_key(|&(a, b)| std::cmp::Reverse(combined(a, b)));
    let near: HashSet<(usize, usize)> = pairs.iter().copied().collect();

    let mut groups: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, name) in head.iter().enumerate() {
        groups.entry(name.key.as_str()).or_default().push(i);
    }
    let mut hits: Vec<(usize, usize)> = Vec::new();
    for members in groups.values().filter(|m| m.len() > 1) {
        for (p, &a) in members.iter().enumerate() {
            for &b in &members[p + 1..] {
                if !near.contains(&(a.min(b), a.max(b))) {
                    hits.push((a.min(b), a.max(b)));
                }
            }
        }
    }
    hits.sort_by_key(|&(a, b)| std::cmp::Reverse(combined(a, b)));

    report.line(format!(
        "#### {label}: {} pairs at distance 1 with different keys, {} of them differ by a usual spelling variation (candidate misses); {} candidate hits (same key, distance above 1); among the {top} most frequent.",
        different_key.len(),
        misses.len(),
        hits.len()
    ));
    report.blank();
    for (title, list) in [
        ("Candidate misses (spelling variations kept apart)", &misses),
        ("Candidate hits", &hits),
    ] {
        report.line(format!("{title}, by combined frequency:"));
        report.blank();
        report.line("| pair | counts | keys |");
        report.line("| --- | --- | --- |");
        for &(a, b) in list.iter().take(25) {
            report.line(format!(
                "| {} / {} | {} / {} | `{}` / `{}` |",
                title_case(&head[a].text),
                title_case(&head[b].text),
                head[a].count,
                head[b].count,
                head[a].key,
                head[b].key
            ));
        }
        report.blank();
    }
}

/// Thirty frequent surnames and first names that come from outside French spelling, chosen by hand.
const NAMES_FROM_ELSEWHERE: [&str; 30] = [
    "BENALI",
    "HADDAD",
    "NGUYEN",
    "TRAN",
    "MARTINS",
    "FERREIRA",
    "PEREIRA",
    "GONCALVES",
    "RODRIGUES",
    "KOWALSKI",
    "NOWAK",
    "YILMAZ",
    "DEMIR",
    "KAYA",
    "OZTURK",
    "BENSAID",
    "MEBARKI",
    "KHELIFI",
    "SAIDI",
    "DIALLO",
    "TRAORE",
    "CAMARA",
    "TOURE",
    "SYLLA",
    "BOUKHARI",
    "KONE",
    "LEFEBVRE",
    "GARCIA",
    "FERNANDEZ",
    "ROSSI",
];

fn elsewhere(report: &mut Report, names: &[Name]) {
    report.line("| name | rank | key |");
    report.line("| --- | --- | --- |");
    let by_folded: HashMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.folded.as_str(), i))
        .collect();
    for wanted in NAMES_FROM_ELSEWHERE {
        match by_folded.get(fold(wanted).as_str()) {
            Some(&i) => report.line(format!(
                "| {} | {} | `{}` |",
                title_case(wanted),
                i + 1,
                names[i].key
            )),
            None => report.line(format!(
                "| {} | absent from the list | |",
                title_case(wanted)
            )),
        }
    }
    report.blank();
}

// ----------------------------------------------------------------------------- tests

fn write_evidence(report: &Report) {
    println!("{}", report.0);
    if let Some(path) = std::env::var_os("ACAI_NAMES_EVIDENCE") {
        std::fs::write(&path, &report.0).expect("cannot write the evidence file");
    }
}

#[test]
fn full_lists_calibration() {
    let (fr, en) = (french(), english());
    let Some(lists) = load_full(&fr, &en) else {
        println!(
            "NOTICE: the full name lists are not in {} - run `python scripts/fetch_name_lists.py` to measure on them. Skipped.",
            names_dir().display()
        );
        return;
    };
    let mut report = Report(String::new());
    report.line(format!(
        "Encoders: French `{}`, English `{}`.",
        fr.encoder.signature(),
        en.encoder.signature()
    ));
    report.blank();

    report.line("### 1. Health");
    report.blank();
    health(
        &mut report,
        "French surnames (Insee)",
        &lists.fr_surnames,
        &fr,
    );
    health(
        &mut report,
        "French first names (Insee)",
        &lists.fr_first,
        &fr,
    );
    health(
        &mut report,
        "English surnames (US Census 2010)",
        &lists.en_surnames,
        &en,
    );
    health(
        &mut report,
        "English first names (US SSA)",
        &lists.en_first,
        &en,
    );

    report.line("### 2. Gate calibration");
    report.blank();
    gate_table(&mut report, "French surnames", &lists.fr_surnames);
    gate_table(&mut report, "French first names", &lists.fr_first);
    gate_table(&mut report, "English surnames", &lists.en_surnames);
    gate_table(&mut report, "English first names", &lists.en_first);

    report.line("### 3. Sex rule");
    report.blank();
    let unexpected_fr = sex_rule(
        &mut report,
        "French first names",
        &lists.fr_first,
        2_000,
        &known_shared("phonetic-fr.json"),
    );
    let unexpected_en = sex_rule(
        &mut report,
        "English first names",
        &lists.en_first,
        20_000,
        &known_shared("phonetic-en.json"),
    );

    report.line("### 4. Misses and hits");
    report.blank();
    misses_and_hits(&mut report, "French surnames", &lists.fr_surnames);
    misses_and_hits(&mut report, "French first names", &lists.fr_first);
    misses_and_hits(&mut report, "English surnames", &lists.en_surnames);
    misses_and_hits(&mut report, "English first names", &lists.en_first);

    report.line("### 5. Names from elsewhere (French encoder, Insee list)");
    report.blank();
    elsewhere(&mut report, &lists.fr_surnames);

    write_evidence(&report);
    assert!(
        unexpected_fr.is_empty() && unexpected_en.is_empty(),
        "opposite-sex first names share a key and are not in `known_shared`: see the table above"
    );
}

#[test]
fn committed_samples_hold() {
    let (fr, en) = (french(), english());
    let lists = load_samples(&fr, &en);
    for (label, names, language) in [
        ("fr_surnames", &lists.fr_surnames, &fr),
        ("fr_first", &lists.fr_first, &fr),
        ("en_surnames", &lists.en_surnames, &en),
        ("en_first", &lists.en_first, &en),
    ] {
        assert!(names.len() >= SAMPLE_SIZE - 20, "{label}: sample too small");
        for name in names {
            assert!(!name.key.is_empty(), "{label}: empty key for {}", name.text);
            assert!(
                name.key.is_ascii(),
                "{label}: non-ASCII key for {}",
                name.text
            );
            assert_eq!(
                name.key,
                key_of(language, &name.text),
                "{label}: unstable key for {}",
                name.text
            );
        }
    }

    // The sex rule on the committed first names: a masculine and a feminine name never share a key.
    for (label, names, file) in [
        ("fr", &lists.fr_first, "phonetic-fr.json"),
        ("en", &lists.en_first, "phonetic-en.json"),
    ] {
        let mut report = Report(String::new());
        let violations = sex_rule(&mut report, label, names, 1, &known_shared(file));
        assert!(
            violations.is_empty(),
            "opposite-sex first names share a key:\n{}",
            report.0
        );
    }
}

/// Rewrites the committed samples from the full lists. Not part of a normal run.
#[test]
#[ignore]
fn write_samples_from_full_lists() {
    if std::env::var_os("ACAI_NAMES_WRITE_SAMPLES").is_none() {
        println!("Set ACAI_NAMES_WRITE_SAMPLES=1 to rewrite the committed samples.");
        return;
    }
    let (fr, en) = (french(), english());
    let lists = load_full(&fr, &en).expect("download the lists first");
    let sources = [
        "Insee, Fichier des noms (Licence Ouverte 2.0)",
        "Insee, Fichier des prenoms (Licence Ouverte 2.0)",
        "US Census Bureau, Frequently Occurring Surnames from the 2010 Census",
        "US Social Security Administration, Baby names (CC0)",
    ];
    for (((_, file), names), source) in SAMPLE_FILES
        .iter()
        .zip([
            &lists.fr_surnames,
            &lists.fr_first,
            &lists.en_surnames,
            &lists.en_first,
        ])
        .zip(sources)
    {
        std::fs::write(fixtures_dir().join(file), sample_text(names, source)).unwrap();
    }
}

/// The letters that differ between two names at edit distance 1, for example `h/` or `c/k`.
fn edit_kind(a: &str, b: &str) -> String {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let side = |word: &[char]| -> String { word[prefix..word.len() - suffix].iter().collect() };
    let (left, right) = (side(&a), side(&b));
    if left <= right {
        format!("{left}/{right}")
    } else {
        format!("{right}/{left}")
    }
}

struct ReviewPair {
    a: usize,
    b: usize,
}

fn one_word(name: &Name) -> bool {
    name.folded.chars().all(|c| c.is_alphanumeric())
}

/// Takes pairs in order, never reusing a name, until `count` are chosen.
fn pick_distinct(
    names: &[Name],
    pairs: impl Iterator<Item = (usize, usize)>,
    count: usize,
    used: &mut HashSet<usize>,
) -> Vec<ReviewPair> {
    let mut chosen = Vec::new();
    for (a, b) in pairs {
        if chosen.len() == count {
            break;
        }
        if used.contains(&a) || used.contains(&b) || !one_word(&names[a]) || !one_word(&names[b]) {
            continue;
        }
        used.insert(a);
        used.insert(b);
        chosen.push(ReviewPair { a, b });
    }
    chosen
}

fn review_row(id: &str, names: &[Name], pair: &ReviewPair, note: &str) -> String {
    let (a, b) = (&names[pair.a], &names[pair.b]);
    let keys = if a.key == b.key {
        format!("`{}`", a.key)
    } else {
        format!("`{}` / `{}`", a.key, b.key)
    };
    format!(
        "| {id} | {} / {} | {} / {} | {keys} | {note} | |",
        title_case(&a.text),
        title_case(&b.text),
        a.count,
        b.count
    )
}

/// The candidate pairs of the owner's review, with the real number of candidates in each class.
/// Not a test of anything:
/// `cargo test --test knowledge_names_calibration print_review_candidates -- --ignored --nocapture`.
#[test]
#[ignore]
fn print_review_candidates() {
    let (fr, en) = (french(), english());
    let Some(lists) = load_full(&fr, &en) else {
        println!("NOTICE: no full lists.");
        return;
    };
    let mut report = Report(String::new());
    let sources = [
        ("FS", "French surnames", &lists.fr_surnames),
        ("FF", "French first names", &lists.fr_first),
        ("ES", "English surnames", &lists.en_surnames),
        ("EF", "English first names", &lists.en_first),
    ];
    let mut hit_rows = Vec::new();
    let mut miss_rows = Vec::new();
    let mut hit_counter = 0;
    let mut miss_counter = 0;

    for (prefix, label, names) in sources {
        let head = &names[..TOP_NAMES.min(names.len())];
        let pairs = near_pairs(head);
        let combined = |&(a, b): &(usize, usize)| std::cmp::Reverse(head[a].count + head[b].count);
        let min_letters = |&(a, b): &(usize, usize)| head[a].letters().min(head[b].letters());

        // Gate: same key, distance 1, three strata of length.
        let mut same: Vec<(usize, usize)> = pairs
            .iter()
            .copied()
            .filter(|&(a, b)| head[a].key == head[b].key && min_letters(&(a, b)) >= 4)
            .collect();
        same.sort_by_key(combined);
        report.line(format!(
            "{label}: {} pairs share a key at distance 1 and have 4 letters or more ({} with 4, {} with 5, {} with 6 or more); {} are made of single words.",
            same.len(),
            same.iter().filter(|p| min_letters(p) == 4).count(),
            same.iter().filter(|p| min_letters(p) == 5).count(),
            same.iter().filter(|p| min_letters(p) >= 6).count(),
            same.iter().filter(|&&(a, b)| one_word(&head[a]) && one_word(&head[b])).count(),
        ));
        let mut used = HashSet::new();
        let mut rows = Vec::new();
        for (stratum, test) in [
            ("4 letters", (|n: usize| n == 4) as fn(usize) -> bool),
            ("5 letters", |n| n == 5),
            ("6 or more", |n| n >= 6),
        ] {
            let chosen = pick_distinct(
                head,
                same.iter().copied().filter(|p| test(min_letters(p))),
                4,
                &mut used,
            );
            for pair in chosen {
                rows.push((stratum, pair));
            }
        }
        report.blank();
        report.line(format!("#### {label}"));
        report.blank();
        report.line("| id | pair | frequencies | key | length | agree? |");
        report.line("| --- | --- | --- | --- | --- | --- |");
        for (number, (stratum, pair)) in rows.iter().enumerate() {
            report.line(review_row(
                &format!("{prefix}{}", number + 1),
                head,
                pair,
                stratum,
            ));
        }
        report.blank();

        // Hits: same key, distance above 1.
        let near: HashSet<(usize, usize)> = pairs.iter().copied().collect();
        let mut groups: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, name) in head.iter().enumerate() {
            groups.entry(name.key.as_str()).or_default().push(i);
        }
        let mut hits: Vec<(usize, usize)> = Vec::new();
        for members in groups.values().filter(|m| m.len() > 1) {
            for (p, &a) in members.iter().enumerate() {
                for &b in &members[p + 1..] {
                    let pair = (a.min(b), a.max(b));
                    if !near.contains(&pair) {
                        hits.push(pair);
                    }
                }
            }
        }
        hits.sort_by_key(combined);
        let single_word_hits = hits
            .iter()
            .filter(|&&(a, b)| one_word(&head[a]) && one_word(&head[b]))
            .count();
        report.line(format!(
            "{label}: {} pairs share a key at a distance above 1 ({single_word_hits} made of single words).",
            hits.len()
        ));
        let mut used = HashSet::new();
        for pair in pick_distinct(head, hits.iter().copied(), 5, &mut used) {
            hit_counter += 1;
            hit_rows.push(review_row(&format!("H{hit_counter}"), head, &pair, label));
        }

        // Misses: distance 1, other key, an edit that spelling variants usually make; one pair per
        // kind of edit, most frequent kinds first.
        let mut variants: Vec<(usize, usize)> = pairs
            .iter()
            .copied()
            .filter(|&(a, b)| {
                head[a].key != head[b].key && is_spelling_variant(&head[a].folded, &head[b].folded)
            })
            .collect();
        variants.sort_by_key(combined);
        report.line(format!(
            "{label}: {} pairs at distance 1 have other keys and differ by a usual spelling variation.",
            variants.len()
        ));
        let mut seen_kinds = HashSet::new();
        let one_per_kind: Vec<(usize, usize)> = variants
            .iter()
            .copied()
            .filter(|&(a, b)| seen_kinds.insert(edit_kind(&head[a].folded, &head[b].folded)))
            .collect();
        let mut used = HashSet::new();
        for pair in pick_distinct(head, one_per_kind.iter().copied(), 5, &mut used) {
            miss_counter += 1;
            miss_rows.push(review_row(
                &format!("M{miss_counter}"),
                head,
                &pair,
                &format!(
                    "{label}, edit `{}`",
                    edit_kind(&head[pair.a].folded, &head[pair.b].folded)
                ),
            ));
        }
        report.blank();
    }
    report.line("#### Hits (same key, spelling further apart)");
    report.blank();
    report.line("| id | pair | frequencies | key | list | agree? |");
    report.line("| --- | --- | --- | --- | --- | --- |");
    for row in hit_rows {
        report.line(row);
    }
    report.blank();
    report.line("#### Misses (different keys, a usual spelling variation)");
    report.blank();
    report.line("| id | pair | frequencies | keys | list | agree? |");
    report.line("| --- | --- | --- | --- | --- | --- |");
    for row in miss_rows {
        report.line(row);
    }
    write_evidence(&report);
}

/// Prints rows for the `differ` list of the contract files: opposite-sex first names that are at
/// most two edits apart and do not share a key, most frequent first. Not part of a normal run:
/// `cargo test --test knowledge_names_calibration print_sex_pair_rows -- --ignored --nocapture`.
#[test]
#[ignore]
fn print_sex_pair_rows() {
    let (fr, en) = (french(), english());
    let Some(lists) = load_full(&fr, &en) else {
        println!("NOTICE: no full lists.");
        return;
    };
    for (label, names, minimum_total) in [
        ("French", &lists.fr_first, 2_000),
        ("English", &lists.en_first, 20_000),
    ] {
        let mut men = Vec::new();
        let mut women = Vec::new();
        for (i, name) in names.iter().enumerate() {
            match name.sex(minimum_total) {
                Some('M') => men.push(i),
                Some('F') => women.push(i),
                _ => {}
            }
        }
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for &m in &men {
            for &w in &women {
                if names[m].key != names[w].key
                    && bounded_damerau_levenshtein(&names[m].folded, &names[w].folded, 2).is_some()
                {
                    pairs.push((m, w));
                }
            }
        }
        pairs.sort_by_key(|&(m, w)| std::cmp::Reverse(names[m].count + names[w].count));
        println!("{label}: {} pairs", pairs.len());
        let mut used = HashSet::new();
        for (m, w) in pairs {
            if used.len() >= 80 {
                break;
            }
            if used.insert(names[m].folded.clone()) | used.insert(names[w].folded.clone()) {
                println!(
                    "    [\"{}\", \"{}\"],",
                    title_case(&names[m].text),
                    title_case(&names[w].text)
                );
            }
        }
    }
}
