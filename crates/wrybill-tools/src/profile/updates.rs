//! Whether this OS still gets security updates (spec 10.3).
//!
//! The answer comes from the table in this file, which is built into each
//! release. It covers macOS and Windows, and anything it doesn't list is
//! "unknown". Nothing is looked up on the internet.
//!
//! How the table is filled in:
//!
//! - **macOS.** Apple publishes no end dates. A version counts as supported
//!   while Apple still ships security updates for it. Once newer versions get
//!   updates without it, it counts as out of support from the day of its own
//!   last security update. Updates with no published security fixes don't
//!   count.
//! - **Windows.** Microsoft publishes the last day of updates for each
//!   version, and those dates are used as they are. They differ between the
//!   Home and Pro editions and the Enterprise and Education ones. Server,
//!   LTSC and IoT editions aren't in the table. A build number that isn't a
//!   released version, such as a preview build, isn't in it either.
//!
//!   Microsoft's lifecycle pages give each end as a moment in UTC, early on
//!   the day after. The table holds the day itself, which is always a
//!   Tuesday.
//!
//! The pages it was checked against:
//!
//! - <https://support.apple.com/en-us/100100> (Apple security releases) and
//!   <https://support.apple.com/en-us/121012> (2022 to 2023)
//! - <https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information>
//! - <https://learn.microsoft.com/en-us/lifecycle/products/windows-11-home-and-pro>
//! - <https://learn.microsoft.com/en-us/lifecycle/products/windows-11-enterprise-and-education>
//! - <https://learn.microsoft.com/en-us/lifecycle/products/windows-10-home-and-pro>
//! - <https://learn.microsoft.com/en-us/windows/release-health/release-information>
//!   (the build number of each Windows 10 version)
//! - <https://www.microsoft.com/en-us/windows/extended-security-updates>
//!
//! When you update the table, update [`TABLE_CHECKED`] too.

use super::date::Date;

/// The day the table was last checked against the vendors' pages.
pub const TABLE_CHECKED: Date = Date::new(2026, 10, 6);

/// After this many days the table is too old to say "supported".
const TABLE_GOOD_FOR_DAYS: u64 = 365;

/// Windows 10 only gets updates through this programme now.
const ESU_CONDITION: &str = "only with Extended Security Updates (ESU), which need enrolling in";

/// Windows 11 versions: the build number, the last day of updates for Home
/// and Pro, and the last day for Enterprise and Education.
const WINDOWS_11: [(u32, Date, Date); 7] = [
    // 21H2
    (22_000, Date::new(2023, 10, 10), Date::new(2024, 10, 8)),
    // 22H2
    (22_621, Date::new(2024, 10, 8), Date::new(2025, 10, 14)),
    // 23H2
    (22_631, Date::new(2025, 11, 11), Date::new(2026, 11, 10)),
    // 24H2
    (26_100, Date::new(2026, 10, 13), Date::new(2027, 10, 12)),
    // 25H2
    (26_200, Date::new(2027, 10, 12), Date::new(2028, 10, 10)),
    // 26H2
    (26_300, Date::new(2028, 10, 10), Date::new(2029, 10, 9)),
    // 26H1
    (28_000, Date::new(2028, 3, 14), Date::new(2029, 3, 13)),
];

/// The first build that is Windows 11. Lower builds are Windows 10 or older.
const FIRST_WINDOWS_11_BUILD: u32 = 22_000;

/// Windows 10, version 22H2: the last version of Windows 10.
const WINDOWS_10_22H2: u32 = 19_045;

/// The day the Extended Security Updates programme for Home and Pro ends.
const WINDOWS_10_ESU_ENDS: Date = Date::new(2027, 10, 12);

/// Every older Windows 10 version, Home and Pro: the build number and the
/// last day of updates.
const OLDER_WINDOWS_10: [(u32, Date); 13] = [
    // 21H2
    (19_044, Date::new(2023, 6, 13)),
    // 21H1
    (19_043, Date::new(2022, 12, 13)),
    // 20H2
    (19_042, Date::new(2022, 5, 10)),
    // 2004
    (19_041, Date::new(2021, 12, 14)),
    // 1909
    (18_363, Date::new(2021, 5, 11)),
    // 1903
    (18_362, Date::new(2020, 12, 8)),
    // 1809
    (17_763, Date::new(2020, 11, 10)),
    // 1803
    (17_134, Date::new(2019, 11, 12)),
    // 1709
    (16_299, Date::new(2019, 4, 9)),
    // 1703
    (15_063, Date::new(2018, 10, 9)),
    // 1607
    (14_393, Date::new(2018, 4, 10)),
    // 1511
    (10_586, Date::new(2017, 10, 10)),
    // 1507, the first Windows 10
    (10_240, Date::new(2017, 5, 9)),
];

/// Whether an OS still gets security updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityUpdates {
    /// It does.
    Supported {
        /// The last day of updates, when the vendor has published one.
        until: Option<Date>,
    },
    /// It does, but only when a condition is met.
    SupportedIf {
        /// The condition, worded to follow "gets security updates".
        condition: &'static str,
        /// The last day of updates, when the vendor has published one.
        until: Option<Date>,
    },
    /// It doesn't any more.
    OutOfSupport {
        /// The last day it got them.
        since: Date,
    },
    /// Wrybill can't tell.
    Unknown(WhyUnknown),
}

/// Why Wrybill can't tell whether an OS still gets security updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhyUnknown {
    /// The table only covers macOS and Windows.
    OsNotCovered,
    /// This version or edition isn't in the table.
    NotInTable,
    /// The table is more than a year old, so it can no longer say
    /// "supported".
    TableTooOld,
}

/// The OS to look up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsRelease<'a> {
    /// macOS, with its version as the OS gives it, such as `26.7.1`.
    MacOs {
        /// The version.
        version: &'a str,
    },
    /// Windows.
    Windows {
        /// The build number, such as 26100.
        build: u32,
        /// The product name, such as `Windows 11 Pro`.
        product: &'a str,
    },
    /// Anything else.
    Other,
}

/// One line of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    /// Still supported, with no end date published.
    Open,
    /// Supported until the end of this day.
    Until(Date),
    /// Supported until the end of this day, and only on a condition.
    UntilIf(Date, &'static str),
    /// Out of support. Its last security update was on this day.
    Ended(Date),
}

/// The two groups of Windows editions that the table has dates for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edition {
    HomeOrPro,
    EnterpriseOrEducation,
}

/// Looks an OS up in the table.
///
/// `today` is the number of days since 1 January 1970, in UTC.
pub fn security_updates(release: &OsRelease<'_>, today: u64) -> SecurityUpdates {
    let entry = match release {
        OsRelease::MacOs { version } => macos_entry(version),
        OsRelease::Windows { build, product } => windows_entry(*build, product),
        OsRelease::Other => return SecurityUpdates::Unknown(WhyUnknown::OsNotCovered),
    };
    match entry {
        Some(entry) => answer(entry, today),
        None => SecurityUpdates::Unknown(WhyUnknown::NotInTable),
    }
}

/// Turns a line of the table into today's answer.
fn answer(entry: Entry, today: u64) -> SecurityUpdates {
    let table_is_too_old = today > TABLE_CHECKED.days_since_1970() + TABLE_GOOD_FOR_DAYS;

    match entry {
        Entry::Ended(since) => SecurityUpdates::OutOfSupport { since },
        // A published end date holds however old the table is.
        Entry::Until(end) | Entry::UntilIf(end, _) if today > end.days_since_1970() => {
            SecurityUpdates::OutOfSupport { since: end }
        }
        _ if table_is_too_old => SecurityUpdates::Unknown(WhyUnknown::TableTooOld),
        Entry::Open => SecurityUpdates::Supported { until: None },
        Entry::Until(end) => SecurityUpdates::Supported { until: Some(end) },
        Entry::UntilIf(end, condition) => SecurityUpdates::SupportedIf {
            condition,
            until: Some(end),
        },
    }
}

fn macos_entry(version: &str) -> Option<Entry> {
    let mut parts = version.trim().split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = match parts.next() {
        Some(minor) => minor.parse().ok()?,
        None => 0,
    };

    Some(match (major, minor) {
        // Golden Gate, Tahoe and Sequoia.
        (27 | 26 | 15, _) => Entry::Open,
        // Sonoma.
        (14, _) => Entry::Ended(Date::new(2026, 8, 6)),
        // Ventura.
        (13, _) => Entry::Ended(Date::new(2025, 8, 20)),
        // Monterey.
        (12, _) => Entry::Ended(Date::new(2024, 7, 29)),
        // Big Sur, which calls itself 10.16 to some programs.
        (11, _) | (10, 16) => Entry::Ended(Date::new(2023, 9, 11)),
        // Catalina.
        (10, 15) => Entry::Ended(Date::new(2022, 7, 20)),
        _ => return None,
    })
}

fn windows_entry(build: u32, product: &str) -> Option<Entry> {
    let edition = edition(product)?;

    if build >= FIRST_WINDOWS_11_BUILD {
        let (_, home_or_pro, enterprise_or_education) = WINDOWS_11
            .iter()
            .find(|(known_build, _, _)| *known_build == build)?;
        return Some(Entry::Until(match edition {
            Edition::HomeOrPro => *home_or_pro,
            Edition::EnterpriseOrEducation => *enterprise_or_education,
        }));
    }

    // Windows 10. Its Enterprise and Education editions have their own
    // programme for extended updates, which the table doesn't cover.
    if edition != Edition::HomeOrPro {
        return None;
    }
    if build == WINDOWS_10_22H2 {
        return Some(Entry::UntilIf(WINDOWS_10_ESU_ENDS, ESU_CONDITION));
    }
    // Only the builds that were released versions. Anything between them,
    // such as a preview build, isn't in the table, so it's "unknown".
    let (_, ended) = OLDER_WINDOWS_10
        .iter()
        .find(|(known_build, _)| *known_build == build)?;
    Some(Entry::Until(*ended))
}

/// Works out which group of editions a Windows product name belongs to.
/// `None` for the ones the table has no dates for.
fn edition(product: &str) -> Option<Edition> {
    let product = product.to_ascii_lowercase();
    let has = |word: &str| product.contains(word);

    if has("server") || has("ltsc") || has("ltsb") || has("iot") {
        return None;
    }
    // "Pro Education" is one of the Pro editions.
    if has("enterprise") || (has("education") && !has("pro education")) {
        return Some(Edition::EnterpriseOrEducation);
    }
    if has("home") || has(" pro") {
        return Some(Edition::HomeOrPro);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        Date, ESU_CONDITION, OLDER_WINDOWS_10, OsRelease, SecurityUpdates, TABLE_CHECKED,
        WINDOWS_11, WhyUnknown, security_updates,
    };

    /// The day the table was checked.
    fn checked_day() -> u64 {
        TABLE_CHECKED.days_since_1970()
    }

    fn macos(version: &str, today: u64) -> SecurityUpdates {
        security_updates(&OsRelease::MacOs { version }, today)
    }

    fn windows(build: u32, product: &str, today: u64) -> SecurityUpdates {
        security_updates(&OsRelease::Windows { build, product }, today)
    }

    fn out_since(year: u16, month: u8, day: u8) -> SecurityUpdates {
        SecurityUpdates::OutOfSupport {
            since: Date::new(year, month, day),
        }
    }

    #[test]
    fn the_three_newest_macos_versions_are_supported() {
        for version in ["27.0.1", "26.7.1", "26", "15.8.1"] {
            assert_eq!(
                macos(version, checked_day()),
                SecurityUpdates::Supported { until: None },
                "{version}"
            );
        }
    }

    #[test]
    fn older_macos_versions_are_out_of_support_since_their_last_update() {
        let today = checked_day();

        assert_eq!(macos("14.8.9", today), out_since(2026, 8, 6));
        assert_eq!(macos("13.7.8", today), out_since(2025, 8, 20));
        // The newest macOS a 2015 MacBook can run (spec section 5).
        assert_eq!(macos("12.7.6", today), out_since(2024, 7, 29));
        assert_eq!(macos("11.7.11", today), out_since(2023, 9, 11));
        assert_eq!(macos("10.16", today), out_since(2023, 9, 11));
        assert_eq!(macos("10.15.7", today), out_since(2022, 7, 20));
    }

    #[test]
    fn a_macos_version_that_is_not_in_the_table_is_unknown() {
        for version in ["28.0", "10.14.6", "", "sixteen", "26.x"] {
            assert_eq!(
                macos(version, checked_day()),
                SecurityUpdates::Unknown(WhyUnknown::NotInTable),
                "{version:?}"
            );
        }
    }

    #[test]
    fn windows_11_is_supported_until_its_published_day_and_not_after() {
        let last_day = Date::new(2026, 10, 13);

        // Version 24H2, Home and Pro: updates end on 13 October 2026.
        assert_eq!(
            windows(26_100, "Windows 11 Pro", last_day.days_since_1970()),
            SecurityUpdates::Supported {
                until: Some(last_day)
            }
        );
        assert_eq!(
            windows(26_100, "Windows 11 Home", last_day.days_since_1970() + 1),
            out_since(2026, 10, 13)
        );
    }

    #[test]
    fn enterprise_and_education_editions_have_their_own_dates() {
        let today = Date::new(2026, 10, 20).days_since_1970();

        assert_eq!(
            windows(26_100, "Windows 11 Enterprise", today),
            SecurityUpdates::Supported {
                until: Some(Date::new(2027, 10, 12))
            }
        );
        assert_eq!(
            windows(26_100, "Windows 11 Education", today),
            SecurityUpdates::Supported {
                until: Some(Date::new(2027, 10, 12))
            }
        );
        // "Pro Education" follows the Pro dates.
        assert_eq!(
            windows(26_100, "Windows 11 Pro Education", today),
            out_since(2026, 10, 13)
        );
        assert_eq!(
            windows(22_631, "Windows 11 Enterprise", today),
            SecurityUpdates::Supported {
                until: Some(Date::new(2026, 11, 10))
            }
        );
    }

    #[test]
    fn windows_11_versions_whose_day_has_passed_are_out_of_support() {
        let today = checked_day();

        assert_eq!(
            windows(22_000, "Windows 11 Home", today),
            out_since(2023, 10, 10)
        );
        assert_eq!(
            windows(22_621, "Windows 11 Pro", today),
            out_since(2024, 10, 8)
        );
        assert_eq!(
            windows(22_631, "Windows 11 Pro", today),
            out_since(2025, 11, 11)
        );
        assert_eq!(
            windows(22_621, "Windows 11 Enterprise", today),
            out_since(2025, 10, 14)
        );
    }

    #[test]
    fn windows_10_is_supported_only_with_extended_security_updates() {
        let expected = SecurityUpdates::SupportedIf {
            condition: ESU_CONDITION,
            until: Some(Date::new(2027, 10, 12)),
        };

        assert_eq!(windows(19_045, "Windows 10 Home", checked_day()), expected);
        assert_eq!(windows(19_045, "Windows 10 Pro", checked_day()), expected);
    }

    #[test]
    fn older_windows_10_versions_are_out_of_support() {
        let today = checked_day();

        assert_eq!(
            windows(19_044, "Windows 10 Pro", today),
            out_since(2023, 6, 13)
        );
        assert_eq!(
            windows(19_041, "Windows 10 Home", today),
            out_since(2021, 12, 14)
        );
        // Version 1909, version 1809, and the first Windows 10 of all.
        assert_eq!(
            windows(18_363, "Windows 10 Home", today),
            out_since(2021, 5, 11)
        );
        assert_eq!(
            windows(17_763, "Windows 10 Pro", today),
            out_since(2020, 11, 10)
        );
        assert_eq!(
            windows(10_240, "Windows 10 Home", today),
            out_since(2017, 5, 9)
        );
    }

    #[test]
    fn every_released_windows_10_version_is_in_the_table() {
        // Each build number on Microsoft's Windows 10 release page.
        let released = [
            10_240, 10_586, 14_393, 15_063, 16_299, 17_134, 17_763, 18_362, 18_363, 19_041, 19_042,
            19_043, 19_044, 19_045,
        ];
        for build in released {
            assert_ne!(
                windows(build, "Windows 10 Home", checked_day()),
                SecurityUpdates::Unknown(WhyUnknown::NotInTable),
                "{build}"
            );
        }
    }

    #[test]
    fn a_windows_10_build_that_was_never_a_released_version_is_unknown() {
        // Preview builds between two versions, and numbers just outside the
        // table. Nothing is guessed for them.
        for build in [19_040, 18_900, 17_000, 12_000, 10_239, 19_046, 21_390] {
            for product in ["Windows 10 Home", "Windows 10 Pro"] {
                assert_eq!(
                    windows(build, product, checked_day()),
                    SecurityUpdates::Unknown(WhyUnknown::NotInTable),
                    "{build} {product}"
                );
            }
        }
    }

    #[test]
    fn windows_editions_and_builds_outside_the_table_are_unknown() {
        let cases = [
            (26_100, "Windows Server 2025 Datacenter"),
            (26_100, "Windows 11 Enterprise LTSC 2024"),
            (26_100, "Windows 11 IoT Enterprise"),
            (19_044, "Windows 10 Enterprise LTSC 2021"),
            (19_045, "Windows 10 Enterprise"),
            (19_045, "Windows 10 Education"),
            // A preview build.
            (27_900, "Windows 11 Pro"),
            // Windows 8.1.
            (9_600, "Windows 8.1 Pro"),
            (26_100, "Windows"),
            (26_100, ""),
        ];
        for (build, product) in cases {
            assert_eq!(
                windows(build, product, checked_day()),
                SecurityUpdates::Unknown(WhyUnknown::NotInTable),
                "{build} {product:?}"
            );
        }
    }

    #[test]
    fn any_other_os_is_not_covered() {
        assert_eq!(
            security_updates(&OsRelease::Other, checked_day()),
            SecurityUpdates::Unknown(WhyUnknown::OsNotCovered)
        );
    }

    #[test]
    fn once_the_table_is_over_a_year_old_supported_becomes_unknown() {
        let last_good_day = checked_day() + 365;
        let too_old = SecurityUpdates::Unknown(WhyUnknown::TableTooOld);

        assert_eq!(
            macos("26.7.1", last_good_day),
            SecurityUpdates::Supported { until: None }
        );
        assert_eq!(macos("26.7.1", last_good_day + 1), too_old);

        // With an end date that hasn't come yet, and with a condition.
        assert_eq!(
            windows(26_200, "Windows 11 Enterprise", last_good_day + 1),
            too_old
        );
        assert_eq!(
            windows(19_045, "Windows 10 Home", last_good_day + 1),
            too_old
        );
    }

    #[test]
    fn out_of_support_stays_out_of_support_however_old_the_table_is() {
        let years_later = checked_day() + 5 * 365;

        assert_eq!(macos("12.7.6", years_later), out_since(2024, 7, 29));
        // A published end date still counts once it has passed.
        assert_eq!(
            windows(26_200, "Windows 11 Home", years_later),
            out_since(2027, 10, 12)
        );
        assert_eq!(
            windows(19_045, "Windows 10 Home", years_later),
            out_since(2027, 10, 12)
        );
    }

    #[test]
    fn every_date_in_the_table_is_a_real_one_and_no_build_is_listed_twice() {
        let mut builds: Vec<u32> = WINDOWS_11.iter().map(|(build, _, _)| *build).collect();
        builds.extend(OLDER_WINDOWS_10.iter().map(|(build, _)| *build));
        let listed = builds.len();
        builds.sort_unstable();
        builds.dedup();
        assert_eq!(builds.len(), listed);

        for (build, home_or_pro, enterprise_or_education) in WINDOWS_11 {
            // Enterprise and Education always get updates for longer.
            assert!(
                enterprise_or_education.days_since_1970() > home_or_pro.days_since_1970(),
                "{build}"
            );
            // Updates end on a Tuesday. 1 January 1970 was a Thursday.
            for day in [home_or_pro, enterprise_or_education] {
                assert_eq!(day.days_since_1970() % 7, 5, "{build}: {day}");
            }
        }
        for (build, ended) in OLDER_WINDOWS_10 {
            assert_eq!(ended.days_since_1970() % 7, 5, "{build}: {ended}");
        }
    }
}
