//! Reads a parsed TOML table one key at a time.
//!
//! Every problem is collected, with its line, instead of stopping at the
//! first one. Wording lives here too, so the same mistake reads the same way
//! wherever it's made.

use toml::Spanned;
use toml::de::{DeTable, DeValue};

use crate::problem::Problem;

/// Turns positions in the file's text into line numbers.
struct Lines {
    /// Where each line starts, as a byte offset.
    starts: Vec<usize>,
}

impl Lines {
    fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            text.bytes()
                .enumerate()
                .filter(|(_, byte)| *byte == b'\n')
                .map(|(offset, _)| offset + 1),
        );
        Self { starts }
    }

    /// The line, counted from 1, that holds this byte offset.
    fn line_at(&self, offset: usize) -> usize {
        self.starts.partition_point(|start| *start <= offset)
    }
}

/// Everything found while checking one file.
pub(crate) struct Report {
    lines: Lines,
    errors: Vec<Problem>,
    warnings: Vec<Problem>,
}

impl Report {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            lines: Lines::new(text),
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub(crate) fn line_at(&self, offset: usize) -> usize {
        self.lines.line_at(offset)
    }

    pub(crate) fn error(
        &mut self,
        line: Option<usize>,
        place: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.errors.push(Problem {
            line,
            place: place.into(),
            message: message.into(),
        });
    }

    /// Records something that's allowed but worth a second look.
    pub(crate) fn warning(
        &mut self,
        line: Option<usize>,
        place: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.warnings.push(Problem {
            line,
            place: place.into(),
            message: message.into(),
        });
    }

    pub(crate) fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// The errors and the warnings, each in the order of the file.
    pub(crate) fn finish(mut self) -> (Vec<Problem>, Vec<Problem>) {
        // A stable sort, so problems on one line keep the order they were found in.
        self.errors.sort_by_key(|problem| problem.line);
        self.warnings.sort_by_key(|problem| problem.line);
        (self.errors, self.warnings)
    }
}

/// A value together with the line it's on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Found<T> {
    pub(crate) value: T,
    pub(crate) line: usize,
}

/// One item of a list in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Item<'a> {
    /// Its position in the list, counted from 1.
    pub(crate) number: usize,
    pub(crate) text: &'a str,
    pub(crate) line: usize,
}

/// The text items of a list in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct List<'a> {
    /// The items that are text. The others were reported and left out.
    pub(crate) items: Vec<Item<'a>>,
    /// How many items the file has, text or not.
    pub(crate) written: usize,
}

/// Where a table sits in the file, for wording problems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Place {
    /// The top of the file.
    Top,
    /// A section such as `[limits]`.
    Section(&'static str),
    /// One entry of a list such as `[[models]]`.
    Entry {
        list: &'static str,
        number: usize,
        label: Option<String>,
    },
}

impl Place {
    /// The place on its own: `[limits]`, or `[[models]] entry 2 ("claude-haiku")`.
    pub(crate) fn name(&self) -> String {
        match self {
            Self::Top => String::new(),
            Self::Section(name) => format!("[{name}]"),
            Self::Entry {
                list,
                number,
                label: Some(label),
            } => format!("[[{list}]] entry {number} (\"{label}\")"),
            Self::Entry {
                list,
                number,
                label: None,
            } => format!("[[{list}]] entry {number}"),
        }
    }

    /// A key in this place: `[limits] max_steps_per_task`.
    pub(crate) fn key(&self, key: &str) -> String {
        match self {
            Self::Top => key.to_owned(),
            Self::Section(_) => format!("{} {key}", self.name()),
            Self::Entry { .. } => format!("{}, {key}", self.name()),
        }
    }
}

/// A rule for a number that may have a decimal part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Amount {
    /// Anything above 0.
    AboveZero,
    /// 0 or anything above it.
    ZeroOrMore,
}

struct Entry<'a> {
    key: &'a str,
    /// Where the key is written, as a byte offset.
    offset: usize,
    value: &'a Spanned<DeValue<'a>>,
    taken: bool,
}

/// One table of the file, read key by key.
///
/// Every key that's asked for is remembered. [`Section::finish`] then reports
/// the keys nobody asked for, so a typo can't quietly switch something off.
pub(crate) struct Section<'a> {
    place: Place,
    entries: Vec<Entry<'a>>,
    asked: Vec<&'static str>,
}

impl<'a> Section<'a> {
    pub(crate) fn new(table: &'a DeTable<'a>, place: Place) -> Self {
        let entries = table
            .iter()
            .map(|(key, value)| Entry {
                key: key.get_ref().as_ref(),
                offset: key.span().start,
                value,
                taken: false,
            })
            .collect();
        Self {
            place,
            entries,
            asked: Vec::new(),
        }
    }

    /// Names an entry once its `id` is known, so later problems can say which
    /// entry they mean.
    pub(crate) fn set_label(&mut self, text: &str) {
        if let Place::Entry { label, .. } = &mut self.place {
            *label = shown_name(text).map(str::to_owned);
        }
    }

    /// Where this table sits in the file.
    pub(crate) fn place(&self) -> &Place {
        &self.place
    }

    /// Whether the file has this key here, whatever its value.
    pub(crate) fn has(&self, key: &str) -> bool {
        self.entries.iter().any(|entry| entry.key == key)
    }

    /// The line this key is on, if the file has it here, whatever its value.
    pub(crate) fn line_of(&self, key: &str, report: &Report) -> Option<usize> {
        let entry = self.entries.iter().find(|entry| entry.key == key)?;
        Some(report.line_at(entry.offset))
    }

    /// Whether the file has any key here apart from this one.
    pub(crate) fn has_keys_other_than(&self, key: &str) -> bool {
        self.entries.iter().any(|entry| entry.key != key)
    }

    /// Records an error about one of this table's keys.
    pub(crate) fn error(
        &self,
        line: usize,
        key: &str,
        message: impl Into<String>,
        report: &mut Report,
    ) {
        report.error(Some(line), self.place.key(key), message);
    }

    /// Records an error about this table as a whole.
    pub(crate) fn error_here(&self, line: usize, message: impl Into<String>, report: &mut Report) {
        report.error(Some(line), self.place.name(), message);
    }

    fn take(&mut self, key: &'static str, report: &Report) -> Option<Found<&'a DeValue<'a>>> {
        self.asked.push(key);
        let entry = self.entries.iter_mut().find(|entry| entry.key == key)?;
        entry.taken = true;
        Some(Found {
            value: entry.value.get_ref(),
            line: report.line_at(entry.offset),
        })
    }

    /// Text in quotes.
    pub(crate) fn text(
        &mut self,
        key: &'static str,
        report: &mut Report,
    ) -> Option<Found<&'a str>> {
        let found = self.take(key, report)?;
        match found.value {
            DeValue::String(text) => Some(Found {
                value: text.as_ref(),
                line: found.line,
            }),
            other => {
                let message = format!("must be text in quotes, but it's {} here.", kind(other));
                self.error(found.line, key, message, report);
                None
            }
        }
    }

    /// Text in quotes that isn't empty.
    pub(crate) fn filled_text(
        &mut self,
        key: &'static str,
        report: &mut Report,
    ) -> Option<Found<&'a str>> {
        let found = self.text(key, report)?;
        if found.value.trim().is_empty() {
            self.error(found.line, key, "can't be empty.", report);
            return None;
        }
        Some(found)
    }

    /// `true` or `false`.
    pub(crate) fn flag(&mut self, key: &'static str, report: &mut Report) -> Option<Found<bool>> {
        let found = self.take(key, report)?;
        match found.value {
            DeValue::Boolean(flag) => Some(Found {
                value: *flag,
                line: found.line,
            }),
            DeValue::String(text) if matches!(text.as_ref(), "true" | "false") => {
                self.error(
                    found.line,
                    key,
                    "must be true or false without quotes.",
                    report,
                );
                None
            }
            other => {
                let message = format!("must be true or false, but it's {} here.", kind(other));
                self.error(found.line, key, message, report);
                None
            }
        }
    }

    /// A key with only one allowed value, a whole number. `message` says so.
    pub(crate) fn exactly(
        &mut self,
        key: &'static str,
        wanted: i64,
        message: &str,
        report: &mut Report,
    ) {
        let Some(found) = self.take(key, report) else {
            return;
        };
        let matches = match found.value {
            DeValue::Integer(integer) => {
                i128::from_str_radix(integer.as_str(), integer.radix()) == Ok(i128::from(wanted))
            }
            _ => false,
        };
        if !matches {
            self.error(found.line, key, message, report);
        }
    }

    /// A whole number from `least` to `most`.
    pub(crate) fn whole(
        &mut self,
        key: &'static str,
        least: u64,
        most: u64,
        report: &mut Report,
    ) -> Option<Found<u64>> {
        let found = self.take(key, report)?;
        let DeValue::Integer(integer) = found.value else {
            let message = format!(
                "must be a whole number, {least} or more, but it's {} here.",
                kind(found.value)
            );
            self.error(found.line, key, message, report);
            return None;
        };

        // Wide enough for anything TOML can hold, negative numbers included.
        let Ok(number) = i128::from_str_radix(integer.as_str(), integer.radix()) else {
            self.error(found.line, key, "is too large a number.", report);
            return None;
        };
        if number < i128::from(least) {
            let message = format!("must be {least} or more, but it's {number} here.");
            self.error(found.line, key, message, report);
            return None;
        }
        if number > i128::from(most) {
            let message = format!("is too large. The most it can be is {most}.");
            self.error(found.line, key, message, report);
            return None;
        }
        // In range, so it fits.
        u64::try_from(number).ok().map(|value| Found {
            value,
            line: found.line,
        })
    }

    /// A number that may have a decimal part. A whole number is fine too.
    pub(crate) fn amount(
        &mut self,
        key: &'static str,
        rule: Amount,
        report: &mut Report,
    ) -> Option<Found<f64>> {
        let wanted = match rule {
            Amount::AboveZero => "a number above 0",
            Amount::ZeroOrMore => "a number, 0 or more",
        };
        let found = self.take(key, report)?;
        let number = match found.value {
            DeValue::Integer(integer) => i128::from_str_radix(integer.as_str(), integer.radix())
                .ok()
                .map(|n| n as f64),
            DeValue::Float(float) => float.as_str().parse::<f64>().ok(),
            other => {
                let message = format!("must be {wanted}, but it's {} here.", kind(other));
                self.error(found.line, key, message, report);
                return None;
            }
        };

        let allowed = |number: f64| match rule {
            Amount::AboveZero => number > 0.0,
            Amount::ZeroOrMore => number >= 0.0,
        };
        match number {
            Some(number) if number.is_finite() && allowed(number) => Some(Found {
                value: number,
                line: found.line,
            }),
            Some(number) if number.is_finite() => {
                let message = format!("must be {wanted}, but it's {number} here.");
                self.error(found.line, key, message, report);
                None
            }
            _ => {
                let message = format!("must be {wanted}.");
                self.error(found.line, key, message, report);
                None
            }
        }
    }

    /// One of a fixed set of words, such as `"plan"`, `"ask"` or `"auto"`.
    pub(crate) fn choice<T: Copy>(
        &mut self,
        key: &'static str,
        choices: &[(&'static str, T)],
        report: &mut Report,
    ) -> Option<Found<T>> {
        let found = self.take(key, report)?;
        let wanted = quoted_list(choices.iter().map(|(word, _)| *word));
        let DeValue::String(text) = found.value else {
            let message = format!("must be {wanted}, but it's {} here.", kind(found.value));
            self.error(found.line, key, message, report);
            return None;
        };

        match choices.iter().find(|(word, _)| *word == text.as_ref()) {
            Some((_, value)) => Some(Found {
                value: *value,
                line: found.line,
            }),
            None => {
                let message = format!("must be {wanted}, but it's {} here.", shown_word(text));
                self.error(found.line, key, message, report);
                None
            }
        }
    }

    /// A list of text values, with the line of its key.
    ///
    /// An item that isn't text is reported and left out, so the caller can
    /// still check the rest and every problem comes out in one go.
    pub(crate) fn texts(
        &mut self,
        key: &'static str,
        report: &mut Report,
    ) -> Option<Found<List<'a>>> {
        let found = self.take(key, report)?;
        let DeValue::Array(values) = found.value else {
            let message = format!(
                "must be a list in square brackets, such as [\"a\", \"b\"], but it's {} here.",
                kind(found.value)
            );
            self.error(found.line, key, message, report);
            return None;
        };

        let mut items = Vec::with_capacity(values.len());
        for (index, value) in values.iter().enumerate() {
            let number = index + 1;
            let line = report.line_at(value.span().start);
            match value.get_ref() {
                DeValue::String(text) => items.push(Item {
                    number,
                    text: text.as_ref(),
                    line,
                }),
                other => {
                    let message = format!(
                        "item {number} must be text in quotes, but it's {} here.",
                        kind(other)
                    );
                    self.error(line, key, message, report);
                }
            }
        }
        Some(Found {
            value: List {
                items,
                written: values.len(),
            },
            line: found.line,
        })
    }

    /// A section such as `[limits]`, ready to be read key by key.
    pub(crate) fn section(
        &mut self,
        key: &'static str,
        report: &mut Report,
    ) -> Option<Found<Section<'a>>> {
        let found = self.take(key, report)?;
        match found.value {
            DeValue::Table(table) => Some(Found {
                value: Section::new(table, Place::Section(key)),
                line: found.line,
            }),
            other => {
                let message = format!(
                    "must be a section, written as [{key}] on a line of its own, but it's {} here.",
                    kind(other)
                );
                self.error(found.line, key, message, report);
                None
            }
        }
    }

    /// A list of entries such as `[[models]]`, each ready to be read key by
    /// key, with the line the entry starts on.
    pub(crate) fn entries(
        &mut self,
        key: &'static str,
        report: &mut Report,
    ) -> Vec<Found<Section<'a>>> {
        let Some(found) = self.take(key, report) else {
            return Vec::new();
        };
        let DeValue::Array(items) = found.value else {
            let message = format!(
                "must be a list of entries. Start each one with [[{key}]], with two pairs of square brackets."
            );
            self.error(found.line, key, message, report);
            return Vec::new();
        };

        let mut entries = Vec::with_capacity(items.len());
        for (index, item) in items.iter().enumerate() {
            let line = report.line_at(item.span().start);
            match item.get_ref() {
                DeValue::Table(table) => entries.push(Found {
                    value: Section::new(
                        table,
                        Place::Entry {
                            list: key,
                            number: index + 1,
                            label: None,
                        },
                    ),
                    line,
                }),
                other => {
                    let message = format!(
                        "entry {} must start with [[{key}]] on a line of its own, but it's {} here.",
                        index + 1,
                        kind(other)
                    );
                    self.error(line, key, message, report);
                }
            }
        }
        entries
    }

    /// Reports every key that nobody asked for. Call it last.
    pub(crate) fn finish(self, report: &mut Report) {
        for entry in self.entries.iter().filter(|entry| !entry.taken) {
            let key = shown_name(entry.key).unwrap_or("(a name too long to show)");
            let suggestion = closest(entry.key, &self.asked);
            // At the top of the file, a table the file opens with `[name]` is a section.
            let is_section =
                self.place == Place::Top && matches!(entry.value.get_ref(), DeValue::Table(_));
            let (place, message) = if is_section {
                let message = match suggestion {
                    Some(known) => {
                        format!("isn't a section Wrybill knows. Did you mean [{known}]?")
                    }
                    None => "isn't a section Wrybill knows.".to_owned(),
                };
                (format!("[{key}]"), message)
            } else {
                let message = match suggestion {
                    Some(known) => format!("isn't a key Wrybill knows. Did you mean `{known}`?"),
                    None => "isn't a key Wrybill knows.".to_owned(),
                };
                (self.place.key(key), message)
            };
            report.error(Some(report.line_at(entry.offset)), place, message);
        }
    }
}

/// What kind of value this is, in everyday words.
fn kind(value: &DeValue<'_>) -> &'static str {
    match value {
        DeValue::String(_) => "text",
        DeValue::Integer(_) => "a whole number",
        DeValue::Float(_) => "a decimal number",
        DeValue::Boolean(_) => "true or false",
        DeValue::Datetime(_) => "a date or time",
        DeValue::Array(_) => "a list",
        DeValue::Table(_) => "a section",
    }
}

/// Words joined for a sentence: `"plan", "ask" or "auto"`.
pub(crate) fn quoted_list<'w>(words: impl Iterator<Item = &'w str>) -> String {
    let words: Vec<String> = words.map(|word| format!("\"{word}\"")).collect();
    match words.split_last() {
        None => String::new(),
        Some((only, [])) => only.clone(),
        Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
    }
}

/// A wrong value, quoted for a message when it's short and plain.
///
/// Anything longer or stranger than a word could be a key pasted into the
/// wrong place, so it's never shown.
pub(crate) fn shown_word(text: &str) -> String {
    let plain = text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'));
    if !text.is_empty() && text.len() <= 20 && plain {
        format!("\"{text}\"")
    } else {
        "something else".to_owned()
    }
}

/// A name from the file (a key, or an entry's `id`), if it's short and plain
/// enough to show. Real keys are far longer than this allows.
pub(crate) fn shown_name(text: &str) -> Option<&str> {
    let plain = text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
    (!text.is_empty() && text.len() <= 24 && plain).then_some(text)
}

/// The known key closest to a mistyped one, if one is close enough to suggest.
fn closest<'k>(typed: &str, known: &[&'k str]) -> Option<&'k str> {
    let limit = (typed.chars().count() / 3).clamp(1, 3);
    known
        .iter()
        .map(|known| (distance(typed, known), *known))
        .filter(|(distance, _)| *distance <= limit)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, known)| known)
}

/// How many single-character edits turn one word into the other.
fn distance(from: &str, to: &str) -> usize {
    let to: Vec<char> = to.chars().collect();
    // `row[j]` is the distance between the letters of `from` read so far and
    // the first `j` letters of `to`.
    let mut row: Vec<usize> = (0..=to.len()).collect();
    for (i, from_char) in from.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, to_char) in to.iter().enumerate() {
            let insert_or_delete = row[j].min(row[j + 1]) + 1;
            let replace = diagonal + usize::from(from_char != *to_char);
            diagonal = row[j + 1];
            row[j + 1] = insert_or_delete.min(replace);
        }
    }
    row[to.len()]
}

#[cfg(test)]
mod tests {
    use super::{Lines, closest, distance, quoted_list, shown_name, shown_word};

    #[test]
    fn lines_are_counted_from_one() {
        let lines = Lines::new("a = 1\nb = 2\n\nc = 3");

        assert_eq!(lines.line_at(0), 1);
        assert_eq!(lines.line_at(5), 1);
        assert_eq!(lines.line_at(6), 2);
        assert_eq!(lines.line_at(12), 3);
        assert_eq!(lines.line_at(13), 4);
    }

    #[test]
    fn edit_distance_counts_single_character_changes() {
        assert_eq!(distance("planner", "planner"), 0);
        assert_eq!(distance("plannner", "planner"), 1);
        assert_eq!(distance("api_kye", "api_key"), 2);
        assert_eq!(distance("", "abc"), 3);
        assert_eq!(distance("abc", ""), 3);
    }

    #[test]
    fn a_near_miss_gets_a_suggestion_and_a_far_one_does_not() {
        let known = ["planner", "worker", "summariser", "autonomy"];

        assert_eq!(closest("plannner", &known), Some("planner"));
        assert_eq!(closest("sumarizer", &known), Some("summariser"));
        assert_eq!(closest("colour", &known), None);
    }

    #[test]
    fn words_are_joined_for_a_sentence() {
        assert_eq!(quoted_list(["ask"].into_iter()), "\"ask\"");
        assert_eq!(
            quoted_list(["ask", "never"].into_iter()),
            "\"ask\" or \"never\""
        );
        assert_eq!(
            quoted_list(["plan", "ask", "auto"].into_iter()),
            "\"plan\", \"ask\" or \"auto\""
        );
    }

    #[test]
    fn only_short_plain_values_are_ever_shown() {
        assert_eq!(shown_word("atuo"), "\"atuo\"");
        assert_eq!(shown_word("two words"), "something else");
        assert_eq!(shown_word("abcdefghijklmnopqrstuvwxyz"), "something else");
        assert_eq!(shown_word(""), "something else");

        assert_eq!(shown_name("claude-sonnet"), Some("claude-sonnet"));
        assert_eq!(shown_name("qwen3:4b"), Some("qwen3:4b"));
        assert_eq!(shown_name("abcdefghijklmnopqrstuvwxyz"), None);
        assert_eq!(shown_name("has space"), None);
    }
}
