//! GPU capability *discovery interface*, not GPU discovery execution.
//! This crate never touches CUDA, ROCm, Metal, drivers, sysfs or GPU memory.
use crate::protocol::{AcceleratorId, HostId};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AcceleratorBackend {
    Cuda,
    Rocm,
    Metal,
    Vulkan,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuDeviceHint {
    pub device: AcceleratorId,
    pub backend: AcceleratorBackend,
    /// Fixture/driver claims, NOT measured allocatable capacity.
    pub claimed_vram_total_mib: Option<u64>,
    pub claimed_vram_available_mib: Option<u64>,
    pub supports_inference_claim: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CapabilityEvidence {
    /// Deterministic test fixture, including CI without a GPU.
    MockFixture,
    /// Untrusted/unsupported claim; it must never enable remote scheduling.
    UnverifiedReport,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuCapabilitySnapshot {
    pub host: HostId,
    pub at_ms: u64,
    pub evidence: CapabilityEvidence,
    pub devices: Vec<GpuDeviceHint>,
}

impl GpuCapabilitySnapshot {
    /// Real runtimes must separately verify device/runtime capability and approval.
    pub fn verified_runnable(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CapabilityError {
    UnknownHost,
    IdentityMismatch,
    DuplicateDevice,
    ImpossibleMemoryClaim,
    TooManyDevices,
}

/// A future host-local driver adapter implements this and validates its own
/// real probe evidence. The current crate only offers an in-memory mock.
pub trait GpuCapabilityProbe {
    fn probe(&self, expected: &HostId) -> Result<GpuCapabilitySnapshot, CapabilityError>;
}

#[derive(Default)]
pub struct MockGpuCapabilityProbe {
    snapshots: HashMap<HostId, GpuCapabilitySnapshot>,
}

impl MockGpuCapabilityProbe {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put(&mut self, snapshot: GpuCapabilitySnapshot) -> Result<(), CapabilityError> {
        if snapshot.devices.len() > 32 {
            return Err(CapabilityError::TooManyDevices);
        }
        let mut seen = std::collections::HashSet::new();
        for device in &snapshot.devices {
            if !seen.insert(device.device.clone()) {
                return Err(CapabilityError::DuplicateDevice);
            }
            if matches!(
                (device.claimed_vram_total_mib, device.claimed_vram_available_mib),
                (Some(total), Some(available)) if available > total
            ) {
                return Err(CapabilityError::ImpossibleMemoryClaim);
            }
        }
        self.snapshots.insert(snapshot.host.clone(), snapshot);
        Ok(())
    }
}

impl GpuCapabilityProbe for MockGpuCapabilityProbe {
    fn probe(&self, expected: &HostId) -> Result<GpuCapabilitySnapshot, CapabilityError> {
        let snapshot = self
            .snapshots
            .get(expected)
            .ok_or(CapabilityError::UnknownHost)?;
        if &snapshot.host != expected {
            return Err(CapabilityError::IdentityMismatch);
        }
        Ok(snapshot.clone())
    }
}
