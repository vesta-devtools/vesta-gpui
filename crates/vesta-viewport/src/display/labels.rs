//! Numeric annotations over Host-projected world positions, not author identities.
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NumericAtlas {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericLabel {
    pub position: [f32; 3],
    pub number: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumericLabelStyle {
    pub cell_size: [f32; 2],
    pub glyph_scale: f32,
    pub advance_fraction: f32,
    pub offset: [f32; 2],
    pub max_digits: u32,
    pub atlas_cells: u32,
    pub color: [f32; 4],
    pub scale_factor: f32,
    pub depth_tolerance: f32,
    pub occlusion_fraction: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NumericLabels {
    pub atlas: Option<Arc<NumericAtlas>>,
    pub points: Vec<NumericLabel>,
    pub primitives: Vec<NumericLabel>,
    pub style: NumericLabelStyle,
}
