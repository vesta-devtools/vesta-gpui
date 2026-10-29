//! UI byte-color codecs; not a Core color-space transform.
use crate::ColorPickerMode;

pub fn format_rgba_hex(rgba: [u8; 4]) -> String {
    format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        rgba[0], rgba[1], rgba[2], rgba[3]
    )
}

pub fn parse_rgba_hex(value: &str) -> Option<[u8; 4]> {
    let digits = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |offset| u8::from_str_radix(&digits[offset..offset + 2], 16).ok();
    Some([
        channel(0)?,
        channel(2)?,
        channel(4)?,
        if digits.len() == 8 { channel(6)? } else { 255 },
    ])
}

pub fn rgba_to_hsva(rgba: [u8; 4]) -> [u16; 4] {
    let red = f32::from(rgba[0]) / 255.0;
    let green = f32::from(rgba[1]) / 255.0;
    let blue = f32::from(rgba[2]) / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    let hue = if delta <= f32::EPSILON {
        0.0
    } else if max == red {
        60.0 * ((green - blue) / delta).rem_euclid(6.0)
    } else if max == green {
        60.0 * ((blue - red) / delta + 2.0)
    } else {
        60.0 * ((red - green) / delta + 4.0)
    };
    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    [
        hue.round().rem_euclid(360.0) as u16,
        (saturation * 100.0).round().clamp(0.0, 100.0) as u16,
        (max * 100.0).round().clamp(0.0, 100.0) as u16,
        u16::from(rgba[3]),
    ]
}

pub fn hsva_to_rgba(hsva: [u16; 4]) -> [u8; 4] {
    let hue = f32::from(hsva[0] % 360);
    let saturation = f32::from(hsva[1].min(100)) / 100.0;
    let value = f32::from(hsva[2].min(100)) / 100.0;
    let chroma = value * saturation;
    let sector = hue / 60.0;
    let secondary = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (red, green, blue) = match sector.floor() as u8 {
        0 => (chroma, secondary, 0.0),
        1 => (secondary, chroma, 0.0),
        2 => (0.0, chroma, secondary),
        3 => (0.0, secondary, chroma),
        4 => (secondary, 0.0, chroma),
        _ => (chroma, 0.0, secondary),
    };
    let match_value = value - chroma;
    let byte = |component: f32| ((component + match_value) * 255.0).round() as u8;
    [byte(red), byte(green), byte(blue), hsva[3].min(255) as u8]
}

pub fn color_channel_step(
    mut rgba: [u8; 4],
    mode: ColorPickerMode,
    channel: u8,
    step: i8,
) -> Option<[u8; 4]> {
    let index = usize::from(channel);
    if index >= 4 {
        return None;
    }
    if mode == ColorPickerMode::Rgb || index == 3 {
        rgba[index] = if step >= 0 {
            rgba[index].saturating_add(step as u8)
        } else {
            rgba[index].saturating_sub(step.unsigned_abs())
        };
    } else {
        let mut hsva = rgba_to_hsva(rgba);
        let next = i32::from(hsva[index]) + i32::from(step);
        hsva[index] = if index == 0 {
            next.rem_euclid(360) as u16
        } else {
            next.clamp(0, 100) as u16
        };
        rgba = hsva_to_rgba(hsva);
    }
    Some(rgba)
}
