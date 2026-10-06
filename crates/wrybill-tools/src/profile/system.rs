//! What the OS reports about itself, the chip, the memory and the disk.
//!
//! It all comes through the `sysinfo` crate. Only the parts a profile needs
//! are asked for: no list of processes, users or network interfaces.

use std::path::Path;

use sysinfo::{CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, System};

use super::updates::{OsRelease, SecurityUpdates, security_updates};
use super::{Chip, Disk, DiskKind, Memory, Os, OsFamily, features, plain, plain_if_any};

/// The OS, and whether it still gets security updates.
///
/// `today` is the number of days since 1 January 1970, in UTC.
pub(super) fn os(today: u64) -> (Os, SecurityUpdates) {
    if cfg!(target_os = "macos") {
        let version = System::os_version().and_then(|version| plain_if_any(&version));
        let updates = match &version {
            Some(version) => security_updates(&OsRelease::MacOs { version }, today),
            None => security_updates(&OsRelease::MacOs { version: "" }, today),
        };
        let os = Os {
            family: OsFamily::MacOs,
            name: "macOS".to_owned(),
            version,
            kernel: None,
        };
        (os, updates)
    } else if cfg!(windows) {
        // On Windows the "kernel version" is the build number.
        let build = System::kernel_version().and_then(|build| plain_if_any(&build));
        let product = System::long_os_version()
            .and_then(|product| plain_if_any(&product))
            .unwrap_or_else(|| "Windows".to_owned());
        let updates = match build.as_deref().and_then(|build| build.parse().ok()) {
            Some(build) => security_updates(
                &OsRelease::Windows {
                    build,
                    product: &product,
                },
                today,
            ),
            // With no build number there's nothing to look up.
            None => security_updates(
                &OsRelease::Windows {
                    build: 0,
                    product: "",
                },
                today,
            ),
        };
        let os = Os {
            family: OsFamily::Windows,
            name: product,
            version: build.map(|build| format!("build {build}")),
            kernel: None,
        };
        (os, updates)
    } else if cfg!(target_os = "linux") {
        let os = Os {
            family: OsFamily::Linux,
            name: System::name()
                .and_then(|name| plain_if_any(&name))
                .unwrap_or_else(|| "Linux".to_owned()),
            version: System::os_version().and_then(|version| plain_if_any(&version)),
            kernel: System::kernel_version().and_then(|kernel| plain_if_any(&kernel)),
        };
        (os, security_updates(&OsRelease::Other, today))
    } else {
        let os = Os {
            family: OsFamily::Other,
            name: System::name()
                .and_then(|name| plain_if_any(&name))
                .unwrap_or_else(|| std::env::consts::OS.to_owned()),
            version: System::os_version().and_then(|version| plain_if_any(&version)),
            kernel: None,
        };
        (os, security_updates(&OsRelease::Other, today))
    }
}

/// The chip and the memory.
pub(super) fn chip_and_memory() -> (Chip, Memory) {
    let mut system = System::new();
    system.refresh_cpu_list(CpuRefreshKind::nothing());
    system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());

    let cpus = system.cpus();
    let chip = Chip {
        name: cpus.first().and_then(|cpu| plain_if_any(cpu.brand())),
        architecture: plain(&System::cpu_arch()),
        physical_cores: System::physical_core_count(),
        // The standard library is the fallback if the OS lists no cores.
        logical_cores: match cpus.len() {
            0 => std::thread::available_parallelism().map_or(1, usize::from),
            listed => listed,
        },
        features: features::detect(),
    };

    let total_bytes = system.total_memory();
    let memory = Memory {
        total_bytes,
        free_bytes: usable_memory(system.available_memory(), system.free_memory(), total_bytes),
    };
    (chip, memory)
}

/// How much memory a program could use right now.
///
/// "Available" is the better answer, because it counts what the OS would
/// hand back on request. Where the OS doesn't report it, "free" has to do.
/// The answer is never more than the computer has.
fn usable_memory(available: u64, free: u64, total: u64) -> u64 {
    let usable = if available > 0 { available } else { free };
    usable.min(total)
}

/// The disk that holds `folder`, when it can be found.
pub(super) fn disk_holding(folder: Option<&Path>) -> Option<Disk> {
    let disks = Disks::new_with_refreshed_list_specifics(
        DiskRefreshKind::nothing().with_kind().with_storage(),
    );
    let mounted: Vec<Mounted<'_>> = disks
        .list()
        .iter()
        .map(|disk| Mounted {
            at: disk.mount_point(),
            disk: Disk {
                total_bytes: disk.total_space(),
                // A disk can't have more free than it holds, whatever a
                // strange file system reports.
                free_bytes: disk.available_space().min(disk.total_space()),
                kind: match disk.kind() {
                    sysinfo::DiskKind::SSD => DiskKind::Ssd,
                    sysinfo::DiskKind::HDD => DiskKind::Hdd,
                    sysinfo::DiskKind::Unknown(_) => DiskKind::Unknown,
                },
            },
        })
        .collect();
    holding(&mounted, folder?)
}

/// A disk and where it's mounted.
#[derive(Debug, Clone, Copy)]
struct Mounted<'a> {
    at: &'a Path,
    disk: Disk,
}

/// Picks the disk that holds `folder`: the one mounted closest above it.
/// Disks that report no size, such as virtual file systems, are passed over.
fn holding(mounted: &[Mounted<'_>], folder: &Path) -> Option<Disk> {
    mounted
        .iter()
        .filter(|candidate| candidate.disk.total_bytes > 0 && is_inside(folder, candidate.at))
        .max_by_key(|candidate| candidate.at.components().count())
        .map(|candidate| candidate.disk)
}

/// Whether `folder` is `mount_point` or somewhere below it.
fn is_inside(folder: &Path, mount_point: &Path) -> bool {
    if folder.starts_with(mount_point) {
        return true;
    }
    // Windows doesn't care about the case of a drive letter or a folder name.
    cfg!(windows)
        && folder
            .to_string_lossy()
            .to_lowercase()
            .starts_with(&mount_point.to_string_lossy().to_lowercase())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Disk, DiskKind, Mounted, holding, usable_memory};

    fn disk(total_bytes: u64) -> Disk {
        Disk {
            total_bytes,
            free_bytes: total_bytes / 2,
            kind: DiskKind::Ssd,
        }
    }

    /// A path that reads the same on every OS the tests run on.
    fn path(text: &str) -> &Path {
        Path::new(text)
    }

    #[cfg(not(windows))]
    #[test]
    fn the_disk_mounted_closest_above_the_folder_holds_it() {
        let mounted = [
            Mounted {
                at: path("/"),
                disk: disk(100),
            },
            Mounted {
                at: path("/home"),
                disk: disk(200),
            },
            Mounted {
                at: path("/home/sam/big"),
                disk: disk(300),
            },
            Mounted {
                at: path("/var"),
                disk: disk(400),
            },
        ];

        assert_eq!(holding(&mounted, path("/home/sam")), Some(disk(200)));
        assert_eq!(
            holding(&mounted, path("/home/sam/big/data")),
            Some(disk(300))
        );
        assert_eq!(holding(&mounted, path("/srv/data")), Some(disk(100)));
        // `/homework` isn't inside `/home`.
        assert_eq!(holding(&mounted, path("/homework")), Some(disk(100)));
    }

    #[cfg(windows)]
    #[test]
    fn the_drive_the_folder_is_on_holds_it_whatever_the_case() {
        let mounted = [
            Mounted {
                at: path(r"C:\"),
                disk: disk(100),
            },
            Mounted {
                at: path(r"D:\"),
                disk: disk(200),
            },
        ];

        assert_eq!(holding(&mounted, path(r"C:\Users\sam")), Some(disk(100)));
        assert_eq!(holding(&mounted, path(r"d:\data")), Some(disk(200)));
        assert_eq!(holding(&mounted, path(r"E:\elsewhere")), None);
    }

    #[test]
    fn a_disk_that_reports_no_size_is_passed_over() {
        let root = if cfg!(windows) { r"C:\" } else { "/" };
        let mounted = [Mounted {
            at: path(root),
            disk: disk(0),
        }];

        assert_eq!(holding(&mounted, path(root)), None);
        assert_eq!(holding(&[], path(root)), None);
    }

    #[test]
    fn usable_memory_prefers_what_is_available_and_never_exceeds_the_total() {
        assert_eq!(usable_memory(6, 2, 8), 6);
        // Where the OS reports nothing as available, free memory has to do.
        assert_eq!(usable_memory(0, 2, 8), 2);
        assert_eq!(usable_memory(9, 2, 8), 8);
    }
}
