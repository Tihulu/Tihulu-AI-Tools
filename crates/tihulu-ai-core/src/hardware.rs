//! Host identification is separate from actual accelerator/runtime verification.

/// User-selectable hardware profiles; these are recommendations, not proof of support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HardwarePreset {
    AutoDetect,
    RegularComputer,
    PerformanceExperimental,
    RaspberryPi5EightGb,
    RaspberryPi5AiHatPlus2,
    Custom,
}

impl HardwarePreset {
    pub const ALL: [Self; 6] = [
        Self::AutoDetect,
        Self::RegularComputer,
        Self::PerformanceExperimental,
        Self::RaspberryPi5EightGb,
        Self::RaspberryPi5AiHatPlus2,
        Self::Custom,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::AutoDetect => "Auto Detect",
            Self::RegularComputer => "Regular Computer",
            Self::PerformanceExperimental => "Performance / Experimental",
            Self::RaspberryPi5EightGb => "Raspberry Pi 5 — 8 GB",
            Self::RaspberryPi5AiHatPlus2 => "Raspberry Pi 5 + AI HAT+ 2",
            Self::Custom => "Custom",
        }
    }
}

/// Architecture and OS are detected by Rust; GPU/NPU/VRAM are not yet probed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedHost {
    pub os: &'static str,
    pub architecture: &'static str,
    pub accelerator_verified: bool,
}

impl DetectedHost {
    pub fn detect() -> Self {
        Self {
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            accelerator_verified: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_six_presets_have_unique_labels() {
        let labels: Vec<_> = HardwarePreset::ALL.iter().map(|x| x.label()).collect();
        for i in 0..labels.len() {
            assert!(!labels[i].is_empty());
            for j in i + 1..labels.len() {
                assert_ne!(labels[i], labels[j]);
            }
        }
    }

    #[test]
    fn os_detection_does_not_claim_accelerator_support() {
        let host = DetectedHost::detect();
        assert!(!host.os.is_empty());
        assert!(!host.architecture.is_empty());
        assert!(!host.accelerator_verified);
    }
}
