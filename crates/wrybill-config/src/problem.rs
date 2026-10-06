//! Problems found in a config file, worded for the person who has to fix them.

use std::fmt;

/// One thing that's wrong with the config file, or worth a warning.
///
/// The text never holds the value of an `api_key`, a URL or a path, so a
/// problem is always safe to print, to log and to paste into a public issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The line in the file, counted from 1, when the problem has one.
    pub line: Option<usize>,
    /// Where it is, in the file's own terms, such as
    /// `[limits] max_steps_per_task`. Empty when the whole file is the problem.
    pub place: String,
    /// What's wrong and, where it helps, what to do about it.
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(f, "line {line}: ")?;
        }
        if self.place.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.place, self.message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Problem;

    #[test]
    fn a_problem_reads_as_line_then_place_then_message() {
        let problem = Problem {
            line: Some(12),
            place: "[limits] max_steps_per_task".to_owned(),
            message: "must be 1 or more, but it's 0 here.".to_owned(),
        };

        assert_eq!(
            problem.to_string(),
            "line 12: [limits] max_steps_per_task: must be 1 or more, but it's 0 here."
        );
    }

    #[test]
    fn a_problem_with_no_line_or_place_is_just_its_message() {
        let problem = Problem {
            line: None,
            place: String::new(),
            message: "the file is empty.".to_owned(),
        };

        assert_eq!(problem.to_string(), "the file is empty.");
    }
}
