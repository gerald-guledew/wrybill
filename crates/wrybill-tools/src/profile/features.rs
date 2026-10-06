//! What the chip can do, as far as it matters for running models locally
//! (spec 10.3).
//!
//! The chip is asked directly, through the standard library. Nothing is
//! assumed from its name, so a budget chip without AVX is reported as it is
//! (spec section 5).

/// The chip's features that matter to local model servers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuFeatures {
    /// Whether the chip has AVX. `None` when it isn't an x86-64 chip, where
    /// the question doesn't arise.
    pub avx: Option<bool>,
    /// Whether the chip has AVX2. `None` when it isn't an x86-64 chip.
    pub avx2: Option<bool>,
    /// The other features that were found, out of the ones Wrybill looks for.
    pub others: Vec<&'static str>,
}

/// Asks the chip what it can do.
#[cfg(target_arch = "x86_64")]
pub(super) fn detect() -> CpuFeatures {
    use std::arch::is_x86_feature_detected;

    let mut others = Vec::new();
    if is_x86_feature_detected!("sse4.2") {
        others.push("SSE4.2");
    }
    if is_x86_feature_detected!("fma") {
        others.push("FMA");
    }
    if is_x86_feature_detected!("f16c") {
        others.push("F16C");
    }
    if is_x86_feature_detected!("avx512f") {
        others.push("AVX-512");
    }

    CpuFeatures {
        avx: Some(is_x86_feature_detected!("avx")),
        avx2: Some(is_x86_feature_detected!("avx2")),
        others,
    }
}

/// Asks the chip what it can do.
#[cfg(target_arch = "aarch64")]
pub(super) fn detect() -> CpuFeatures {
    use std::arch::is_aarch64_feature_detected;

    let mut others = Vec::new();
    if is_aarch64_feature_detected!("neon") {
        others.push("NEON");
    }
    if is_aarch64_feature_detected!("dotprod") {
        others.push("DotProd");
    }
    if is_aarch64_feature_detected!("i8mm") {
        others.push("I8MM");
    }
    if is_aarch64_feature_detected!("sve") {
        others.push("SVE");
    }
    if is_aarch64_feature_detected!("sve2") {
        others.push("SVE2");
    }

    CpuFeatures {
        avx: None,
        avx2: None,
        others,
    }
}

/// On any other kind of chip, Wrybill has nothing to look for.
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub(super) fn detect() -> CpuFeatures {
    CpuFeatures {
        avx: None,
        avx2: None,
        others: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::detect;

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn an_x86_chip_always_gets_an_answer_about_avx() {
        let features = detect();

        assert!(features.avx.is_some());
        assert!(features.avx2.is_some());
        // No chip has AVX2 without AVX.
        if features.avx2 == Some(true) {
            assert_eq!(features.avx, Some(true));
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn an_arm_chip_has_neon_and_no_answer_about_avx() {
        let features = detect();

        assert_eq!(features.avx, None);
        assert_eq!(features.avx2, None);
        // Every 64-bit ARM chip has NEON.
        assert!(features.others.contains(&"NEON"), "{features:?}");
    }

    #[test]
    fn asking_twice_gives_the_same_answer() {
        assert_eq!(detect(), detect());
    }
}
