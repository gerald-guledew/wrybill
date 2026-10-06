//! Calendar dates, as the security-updates table needs them.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// A calendar date.
///
/// It prints as, for example, `6 October 2026`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}

impl Date {
    /// A date. Months and days count from 1.
    pub const fn new(year: u16, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    /// How many days after 1 January 1970 this date is.
    ///
    /// This is Howard Hinnant's `days_from_civil`, for dates from 1970 on:
    /// <https://howardhinnant.github.io/date_algorithms.html#days_from_civil>
    pub const fn days_since_1970(self) -> u64 {
        // Count the year from March, so a leap day is the last day of its year.
        let year = self.year as u64 - (self.month <= 2) as u64;
        let era = year / 400;
        let year_of_era = year % 400;
        let month_from_march = (self.month as u64 + 9) % 12;
        let day_of_year = (153 * month_from_march + 2) / 5 + self.day as u64 - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let month = MONTHS
            .get(usize::from(self.month).wrapping_sub(1))
            .copied()
            .unwrap_or("month");
        write!(f, "{} {month} {}", self.day, self.year)
    }
}

/// How many days after 1 January 1970 a moment is, in UTC.
///
/// A clock set before 1970 counts as 1 January 1970.
pub(crate) fn days_since_1970(moment: SystemTime) -> u64 {
    moment
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() / 86_400)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::{Date, days_since_1970};

    #[test]
    fn known_dates_are_the_right_number_of_days_after_1970() {
        assert_eq!(Date::new(1970, 1, 1).days_since_1970(), 0);
        assert_eq!(Date::new(1970, 1, 2).days_since_1970(), 1);
        assert_eq!(Date::new(1999, 12, 31).days_since_1970(), 10_956);
        assert_eq!(Date::new(2000, 1, 1).days_since_1970(), 10_957);
        // Leap days: 2000 is a leap year, and so is 2024.
        assert_eq!(Date::new(2000, 2, 29).days_since_1970(), 11_016);
        assert_eq!(Date::new(2000, 3, 1).days_since_1970(), 11_017);
        assert_eq!(Date::new(2024, 2, 29).days_since_1970(), 19_782);
        assert_eq!(Date::new(2024, 3, 1).days_since_1970(), 19_783);
        assert_eq!(Date::new(2026, 10, 6).days_since_1970(), 20_732);
        // 2100 is not a leap year.
        assert_eq!(
            Date::new(2100, 3, 1).days_since_1970() - Date::new(2100, 2, 28).days_since_1970(),
            1
        );
    }

    #[test]
    fn a_day_later_is_one_more() {
        let days = [
            (Date::new(2026, 12, 31), Date::new(2027, 1, 1)),
            (Date::new(2027, 2, 28), Date::new(2027, 3, 1)),
            (Date::new(2028, 2, 28), Date::new(2028, 2, 29)),
            (Date::new(2026, 10, 13), Date::new(2026, 10, 14)),
        ];
        for (one, next) in days {
            assert_eq!(
                one.days_since_1970() + 1,
                next.days_since_1970(),
                "{one} then {next}"
            );
        }
    }

    #[test]
    fn a_date_prints_as_day_month_year() {
        assert_eq!(Date::new(2026, 10, 6).to_string(), "6 October 2026");
        assert_eq!(Date::new(2024, 7, 29).to_string(), "29 July 2024");
        assert_eq!(Date::new(2027, 1, 1).to_string(), "1 January 2027");
    }

    #[test]
    fn a_moment_counts_whole_utc_days() {
        let at = |seconds| days_since_1970(UNIX_EPOCH + Duration::from_secs(seconds));

        assert_eq!(at(0), 0);
        assert_eq!(at(86_399), 0);
        assert_eq!(at(86_400), 1);
        // Noon on 5 October 2026.
        assert_eq!(at(1_791_201_600), Date::new(2026, 10, 5).days_since_1970());
    }

    #[test]
    fn a_clock_set_before_1970_counts_as_the_first_day() {
        assert_eq!(days_since_1970(UNIX_EPOCH - Duration::from_secs(1)), 0);
    }
}
