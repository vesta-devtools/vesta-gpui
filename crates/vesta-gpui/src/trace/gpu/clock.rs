//! Checked calibration arithmetic. Anchor deviation is not a clock-drift bound.
use super::GpuClock;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuCalibrationSupport {
    #[default]
    NotRecorded,
    VulkanExtQpc,
    VulkanKhrQpc,
    UnsupportedPlatform,
    UnsupportedExtension,
    UnsupportedHostDomain,
    CapabilityQueryFailed,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clock() -> GpuClock {
        GpuClock {
            generation: 1,
            device_name: "fixture".into(),
            vendor_id: 1,
            device_id: 1,
            driver_version: 1,
            queue_family: 0,
            valid_bits: Some(64),
            period_ns: Some(1.0),
            unavailable: None,
            calibration_support: GpuCalibrationSupport::VulkanKhrQpc,
        }
    }
    fn sample() -> GpuCalibration {
        GpuCalibration {
            device_ticks: 9_007_199_254_741_999,
            host_ticks: 2_000_000_000_000_050,
            host_before_ticks: 2_000_000_000_000_000,
            host_after_ticks: 2_000_000_000_000_100,
            host_frequency_hz: 10_000_000,
            cpu_before_ns: 1_000_000,
            cpu_after_ns: 1_010_200,
            max_deviation_ns: 100,
        }
    }
    #[test]
    fn high_ticks_bridge_and_wrap_preserve_integer_evidence() {
        let c = sample();
        let e = map_execution(
            &clock(),
            [c.device_ticks - 5000, c.device_ticks - 4000],
            999000,
            1000,
            &c,
        )
        .expect("fresh calibration");
        assert_eq!(
            (e.start_ns, e.end_ns, e.anchor_cpu_ns),
            (1_000_100, 1_001_100, 1_005_100)
        );
        assert_eq!(e.anchor_uncertainty_ns, 301);
        assert_eq!(e.anchor_distance_ns, 5000);
        let mut k = clock();
        k.valid_bits = Some(8);
        let mut c = sample();
        c.device_ticks = 4;
        c.host_frequency_hz = 1_000_000_000;
        c.cpu_after_ns = 1_000_104;
        c.max_deviation_ns = 1;
        let e =
            map_execution(&k, [250, 255], 1_000_000, 5, &c).expect("unambiguous wrapped counter");
        assert_eq!(e.start_ns, 1_000_042);
        assert_eq!(e.end_ns - e.start_ns, 5);
        assert_eq!(
            map_execution(&k, [250, 255], 999000, 5, &c).expect_err("half wrap exceeded"),
            GpuCalibrationFailure::AmbiguousWrap
        );
    }
    #[test]
    fn invalid_bridge_clocks_and_impossible_execution_are_not_mapped() {
        let k = clock();
        let c = sample();
        let raw = [c.device_ticks - 5000, c.device_ticks - 4000];
        for field in 0..7 {
            let mut bad = c.clone();
            match field {
                0 => bad.host_frequency_hz = 0,
                1 => bad.cpu_after_ns = 1,
                2 => bad.host_after_ticks = 0,
                3 => bad.max_deviation_ns = 0,
                4 => bad.host_ticks = u64::MAX,
                5 => bad.host_ticks += 1_000_000,
                _ => bad.host_frequency_hz = 1_000_000,
            };
            assert!(
                map_execution(&k, raw, 999000, 1000, &bad).is_err(),
                "field {field}"
            );
        }
        assert_eq!(
            map_execution(
                &k,
                [c.device_ticks - 50000, c.device_ticks - 49000],
                999000,
                1000,
                &c
            )
            .expect_err("GPU cannot precede CPU recording"),
            GpuCalibrationFailure::ImplausibleMapping
        );
        assert!(
            map_execution(&k, raw, 999000, 900, &c).is_err(),
            "raw duration must agree"
        );
    }
}
impl GpuCalibrationSupport {
    pub fn available(self) -> bool {
        matches!(self, Self::VulkanExtQpc | Self::VulkanKhrQpc)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuCalibrationFailure {
    QueryFailed,
    HostClockFailed,
    InvalidMetadata,
    AmbiguousWrap,
    ImplausibleMapping,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuCalibration {
    pub device_ticks: u64,
    pub host_ticks: u64,
    pub host_before_ticks: u64,
    pub host_after_ticks: u64,
    pub host_frequency_hz: u64,
    pub cpu_before_ns: u64,
    pub cpu_after_ns: u64,
    pub max_deviation_ns: u64,
}
#[derive(Clone, Copy, Debug)]
pub struct GpuExecutionEstimate {
    pub start_ns: i64,
    pub end_ns: i64,
    pub anchor_cpu_ns: i64,
    pub anchor_uncertainty_ns: u64,
    pub anchor_distance_ns: u64,
}
pub fn map_execution(
    clock: &GpuClock,
    raw: [u64; 2],
    recorded_cpu_ns: u64,
    elapsed_ns: u64,
    c: &GpuCalibration,
) -> Result<GpuExecutionEstimate, GpuCalibrationFailure> {
    use GpuCalibrationFailure::*;
    let bits = clock
        .valid_bits
        .filter(|n| (1..=64).contains(n))
        .ok_or(InvalidMetadata)?;
    let period = clock
        .period_ns
        .filter(|p| p.is_finite() && *p > 0.0)
        .ok_or(InvalidMetadata)?;
    if !clock.calibration_support.available()
        || clock.unavailable.is_some()
        || c.host_frequency_hz == 0
        || c.host_frequency_hz > 1_000_000_000_000
        || c.cpu_before_ns < recorded_cpu_ns
        || c.cpu_after_ns < c.cpu_before_ns
        || c.cpu_after_ns > ((1u64 << 53) - 1)
        || c.host_after_ticks < c.host_before_ticks
        || c.host_after_ticks > i64::MAX as u64
        || c.host_ticks > i64::MAX as u64
        || c.max_deviation_ns == 0
        || c.max_deviation_ns > 1_000_000_000
    {
        return Err(InvalidMetadata);
    }
    let frequency = i128::from(c.host_frequency_hz);
    let floor_ns = |ticks: i128| (ticks * 1_000_000_000).div_euclid(frequency);
    let ceil_ns = |ticks: i128| -floor_ns(-ticks);
    let quantization = (1_000_000_000u64.div_ceil(c.host_frequency_hz))
        .max(1)
        .checked_add(period.ceil() as u64)
        .ok_or(InvalidMetadata)?;
    let padding = i128::from(c.max_deviation_ns) + i128::from(quantization);
    if ceil_ns(i128::from(c.host_ticks) - i128::from(c.host_before_ticks)) < -padding
        || floor_ns(i128::from(c.host_ticks) - i128::from(c.host_after_ticks)) > padding
    {
        return Err(InvalidMetadata);
    }
    let low = i128::from(c.cpu_before_ns)
        + floor_ns(i128::from(c.host_ticks) - i128::from(c.host_before_ticks))
        - padding;
    let high = i128::from(c.cpu_after_ns)
        + ceil_ns(i128::from(c.host_ticks) - i128::from(c.host_after_ticks))
        + padding;
    if low > high {
        return Err(InvalidMetadata);
    }
    let uncertainty = u64::try_from((high - low + 1) / 2).map_err(|_| InvalidMetadata)?;
    let anchor = (low + high).div_euclid(2);
    // A completed GPU mark lies between original CPU recording and this fresh
    // calibration. A half-wrap or larger observation interval is ambiguous.
    let half = 2f64.powi(bits as i32 - 1) * period;
    if (c.cpu_after_ns - recorded_cpu_ns) as f64 + uncertainty as f64 >= half {
        return Err(AmbiguousWrap);
    }
    let expected = super::timestamp_elapsed(
        raw,
        bits,
        period,
        std::time::Duration::from_nanos(c.cpu_after_ns - recorded_cpu_ns),
    )
    .map_err(|_| InvalidMetadata)?;
    if expected.as_nanos() != u128::from(elapsed_ns) {
        return Err(InvalidMetadata);
    }
    let modulus = 1u128 << bits;
    let mask = (modulus - 1) as u64;
    let delta = u128::from(raw[0].wrapping_sub(c.device_ticks) & mask);
    let signed = if delta >= modulus / 2 {
        delta as i128 - modulus as i128
    } else {
        delta as i128
    };
    let offset = signed as f64 * period;
    if !offset.is_finite() || offset.abs() > ((1u64 << 53) - 1) as f64 {
        return Err(InvalidMetadata);
    }
    let start = anchor + offset.round() as i128;
    let end = start + i128::from(elapsed_ns);
    if start + i128::from(uncertainty) < i128::from(recorded_cpu_ns)
        || end - i128::from(uncertainty) > i128::from(c.cpu_after_ns)
    {
        return Err(ImplausibleMapping);
    }
    Ok(GpuExecutionEstimate {
        start_ns: i64::try_from(start).map_err(|_| InvalidMetadata)?,
        end_ns: i64::try_from(end).map_err(|_| InvalidMetadata)?,
        anchor_cpu_ns: i64::try_from(anchor).map_err(|_| InvalidMetadata)?,
        anchor_uncertainty_ns: uncertainty,
        anchor_distance_ns: u64::try_from((anchor - start).abs().max((anchor - end).abs()))
            .map_err(|_| InvalidMetadata)?,
    })
}
