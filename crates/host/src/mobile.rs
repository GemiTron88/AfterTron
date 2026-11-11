//! Portable mobile hardware capability adapter for `AfterTron-Mobile` (Android Vulkan & iOS Metal).
//!
//! Negotiates GPU compositor backend (`Vulkan`, `Metal`, or deterministic `CpuCompositor` fallback),
//! RAM-preview frame cache budget, composition preview downsample factor, concurrent decode streams,
//! and thermal throttling without crashing on missing compute shaders.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MobileOsKind {
    Android,
    Ios,
    Ipados,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MobileMotionBackend {
    Vulkan,
    Metal,
    CpuCompositor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MobileThermalState {
    Nominal,
    Fair,
    Serious,
    Critical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MobileMemoryPressure {
    Normal,
    Moderate,
    Critical,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileMotionHardwareCapabilities {
    pub os: MobileOsKind,
    pub total_ram_mb: u32,
    pub max_texture_dimension_2d: u32,
    pub supports_vulkan_1_1: bool,
    pub supports_metal_family_apple5: bool,
    pub supports_16bpc_float_textures: bool,
    pub hardware_video_decoders: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileMotionRuntimeBudget {
    pub backend: MobileMotionBackend,
    pub ram_preview_cache_mb: u32,
    pub max_preview_long_edge: u32,
    pub preview_downsample_factor: u8,
    pub max_concurrent_decode_streams: u8,
    pub bit_depth_bpc: u8,
    pub enable_optical_glow_half_res: bool,
    pub cpu_fallback_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileMotionBudgetRequest {
    pub hardware: MobileMotionHardwareCapabilities,
    pub thermal: MobileThermalState,
    pub memory_pressure: MobileMemoryPressure,
}

pub fn negotiate_mobile_motion_budget(
    hw: &MobileMotionHardwareCapabilities,
    thermal: MobileThermalState,
    pressure: MobileMemoryPressure,
) -> MobileMotionRuntimeBudget {
    let (backend, fallback_reason) = match hw.os {
        MobileOsKind::Android => {
            if hw.supports_vulkan_1_1 && hw.max_texture_dimension_2d >= 4096 {
                (MobileMotionBackend::Vulkan, None)
            } else {
                (
                    MobileMotionBackend::CpuCompositor,
                    Some("Vulkan 1.1 or >=4096px 2D texture unavailable on Android device".into()),
                )
            }
        }
        MobileOsKind::Ios | MobileOsKind::Ipados => {
            if hw.supports_metal_family_apple5 && hw.max_texture_dimension_2d >= 4096 {
                (MobileMotionBackend::Metal, None)
            } else {
                (
                    MobileMotionBackend::CpuCompositor,
                    Some("Metal Apple5 GPU family unavailable on iOS/iPadOS device".into()),
                )
            }
        }
    };

    let base_cache = (hw.total_ram_mb / 6).clamp(96, 1536);
    let ram_preview_cache_mb = match pressure {
        MobileMemoryPressure::Normal => base_cache,
        MobileMemoryPressure::Moderate => (base_cache / 2).max(64),
        MobileMemoryPressure::Critical => 48,
    };

    let preview_downsample_factor = match (thermal, pressure) {
        (MobileThermalState::Critical, _) | (_, MobileMemoryPressure::Critical) => 4,
        (MobileThermalState::Serious, _) | (_, MobileMemoryPressure::Moderate) => 2,
        _ if backend == MobileMotionBackend::CpuCompositor => 2,
        _ => 1,
    };

    let max_preview_long_edge = match preview_downsample_factor {
        1 => hw.max_texture_dimension_2d.clamp(1920, 3840),
        2 => 1920,
        _ => 1280,
    };

    let max_concurrent_decode_streams = match (thermal, pressure) {
        (MobileThermalState::Critical, _) | (_, MobileMemoryPressure::Critical) => 1,
        (MobileThermalState::Serious, _) => hw.hardware_video_decoders.clamp(1, 2),
        _ => hw.hardware_video_decoders.clamp(1, 4),
    };

    let bit_depth_bpc = if backend != MobileMotionBackend::CpuCompositor
        && hw.supports_16bpc_float_textures
        && pressure == MobileMemoryPressure::Normal
    {
        16
    } else {
        8
    };

    MobileMotionRuntimeBudget {
        backend,
        ram_preview_cache_mb,
        max_preview_long_edge,
        preview_downsample_factor,
        max_concurrent_decode_streams,
        bit_depth_bpc,
        enable_optical_glow_half_res: preview_downsample_factor >= 2,
        cpu_fallback_reason: fallback_reason,
    }
}

/// JSON FFI entrypoint consumed by `AfterTron-Mobile` (`dart:ffi` / Flutter Rust Bridge).
pub fn negotiate_mobile_motion_budget_json(request_json: &str) -> Result<String, String> {
    let req: MobileMotionBudgetRequest = serde_json::from_str(request_json)
        .map_err(|e| format!("invalid AfterTron-Mobile budget request JSON: {e}"))?;
    let budget = negotiate_mobile_motion_budget(&req.hardware, req.thermal, req.memory_pressure);
    serde_json::to_string(&budget)
        .map_err(|e| format!("failed serializing AfterTron-Mobile budget JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_motion_adapter_negotiates_vulkan_metal_and_cpu_fallback() {
        let android_req = MobileMotionBudgetRequest {
            hardware: MobileMotionHardwareCapabilities {
                os: MobileOsKind::Android,
                total_ram_mb: 12_288,
                max_texture_dimension_2d: 8192,
                supports_vulkan_1_1: true,
                supports_metal_family_apple5: false,
                supports_16bpc_float_textures: true,
                hardware_video_decoders: 4,
            },
            thermal: MobileThermalState::Nominal,
            memory_pressure: MobileMemoryPressure::Normal,
        };
        let raw = serde_json::to_string(&android_req).unwrap_or_default();
        let out = negotiate_mobile_motion_budget_json(&raw);
        assert!(out.is_ok());
        let parsed: MobileMotionRuntimeBudget =
            serde_json::from_str(&out.unwrap_or_default()).expect("valid budget json");
        assert_eq!(parsed.backend, MobileMotionBackend::Vulkan);
        assert_eq!(parsed.bit_depth_bpc, 16);
        assert_eq!(parsed.preview_downsample_factor, 1);

        let fallback = negotiate_mobile_motion_budget(
            &MobileMotionHardwareCapabilities {
                os: MobileOsKind::Ios,
                total_ram_mb: 3072,
                max_texture_dimension_2d: 2048,
                supports_vulkan_1_1: false,
                supports_metal_family_apple5: false,
                supports_16bpc_float_textures: false,
                hardware_video_decoders: 2,
            },
            MobileThermalState::Critical,
            MobileMemoryPressure::Critical,
        );
        assert_eq!(fallback.backend, MobileMotionBackend::CpuCompositor);
        assert_eq!(fallback.preview_downsample_factor, 4);
        assert_eq!(fallback.max_concurrent_decode_streams, 1);
        assert!(fallback.cpu_fallback_reason.is_some());
    }
}
