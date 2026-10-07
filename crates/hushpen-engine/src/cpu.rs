//! Checks that the CPU can run the ggml code the fork builds.
//!
//! The Linux x64 build targets x86-64-v3 (see `vendor/whisper-rs-sys/build.rs`). Without
//! this check an older CPU dies with SIGILL at the first matrix multiply.

/// The name of a missing instruction set, or `None` when this CPU can run the engine.
#[cfg(all(target_arch = "x86_64", target_os = "linux"))]
pub fn missing_cpu_feature() -> Option<&'static str> {
    [
        ("AVX2", is_x86_feature_detected!("avx2")),
        ("FMA", is_x86_feature_detected!("fma")),
        ("F16C", is_x86_feature_detected!("f16c")),
        ("BMI2", is_x86_feature_detected!("bmi2")),
    ]
    .into_iter()
    .find_map(|(name, present)| (!present).then_some(name))
}

/// The name of a missing instruction set, or `None` when this CPU can run the engine.
#[cfg(not(all(target_arch = "x86_64", target_os = "linux")))]
pub fn missing_cpu_feature() -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_machine_can_run_the_engine() {
        assert_eq!(missing_cpu_feature(), None);
    }
}
