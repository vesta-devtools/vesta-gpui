//! Versioned capture addressing and sensitivity admission; no GPU or image I/O.
use serde_json::Value;
use vesta_gpui::{ElementId, Rect, UiSnapshot, UiWindowId, Viewport};
#[derive(Clone, Debug)]
pub struct UiCaptureRequest {
    pub instance: u128,
    pub window: UiWindowId,
    pub revision: u64,
    pub element: Option<ElementId>,
    pub rect: Option<Rect>,
    pub viewport: Option<Viewport>,
}
#[derive(Clone, Copy, Debug)]
pub struct UiCaptureGeometry {
    pub logical: Rect,
    pub origin: [u32; 2],
    pub extent: [u32; 2],
}
impl UiCaptureRequest {
    pub fn parse(value: &Value) -> Result<Self, &'static str> {
        let keys = [
            "schema_version",
            "request_id",
            "op",
            "instance",
            "window",
            "revision",
            "element",
            "rect",
            "viewport",
        ];
        if value
            .as_object()
            .is_none_or(|v| v.keys().any(|key| !keys.contains(&key.as_str())))
        {
            return Err("invalid_request");
        }
        let number = |key: &str| {
            value[key]
                .as_str()
                .ok_or("invalid_request")?
                .parse::<u64>()
                .map_err(|_| "invalid_request")
        };
        let scalar = |object: &Value, key: &str| {
            object[key]
                .as_f64()
                .map(|v| v as f32)
                .filter(|v| v.is_finite())
                .ok_or("invalid_request")
        };
        let rect = if let Some(r) = value.get("rect") {
            if r.as_object().is_none_or(|r| {
                r.len() != 4
                    || r.keys()
                        .any(|key| !["x", "y", "width", "height"].contains(&key.as_str()))
            }) {
                return Err("invalid_request");
            }
            Some(Rect::new(
                scalar(r, "x")?,
                scalar(r, "y")?,
                scalar(r, "width")?,
                scalar(r, "height")?,
            ))
        } else {
            None
        };
        let viewport = if let Some(v) = value.get("viewport") {
            if v.as_object().is_none_or(|v| {
                v.len() != 3
                    || v.keys()
                        .any(|key| !["width", "height", "scale_factor"].contains(&key.as_str()))
            }) {
                return Err("invalid_request");
            }
            Some(Viewport::new(
                scalar(v, "width")?,
                scalar(v, "height")?,
                scalar(v, "scale_factor")?,
            ))
        } else {
            None
        };
        if rect.is_some() && value.get("element").is_some() {
            return Err("invalid_request");
        }
        Ok(Self {
            instance: value["instance"]
                .as_str()
                .ok_or("invalid_request")?
                .parse()
                .map_err(|_| "invalid_request")?,
            window: UiWindowId(number("window")?),
            revision: number("revision")?,
            element: value
                .get("element")
                .map(|_| number("element").map(ElementId::new))
                .transpose()?,
            rect,
            viewport,
        })
    }
    pub fn geometry(
        &self,
        snapshot: &UiSnapshot,
        viewport: Viewport,
    ) -> Result<UiCaptureGeometry, &'static str> {
        if snapshot.node_limit_reached || snapshot.depth_limit_reached {
            return Err("capture_incomplete_source");
        }
        let rect = if let Some(element) = self.element {
            snapshot
                .nodes()
                .iter()
                .find(|node| node.id == element)
                .ok_or("target_not_captured")?
                .visible_bounds
                .ok_or("offscreen_target")?
        } else {
            self.rect.unwrap_or(viewport.logical_rect())
        };
        let geometry = UiCaptureGeometry::new(rect, viewport)?;
        if snapshot.nodes().iter().any(|node| {
            (node.value.redacted || node.text.redacted || node.name.redacted)
                && node
                    .visible_bounds
                    .is_some_and(|r| r.intersection(geometry.logical).is_some())
        }) {
            return Err("sensitive_region");
        }
        Ok(geometry)
    }
}
impl UiCaptureGeometry {
    pub fn new(rect: Rect, viewport: Viewport) -> Result<Self, &'static str> {
        if !viewport.width.is_finite()
            || !viewport.height.is_finite()
            || !viewport.scale_factor.is_finite()
            || viewport.width <= 0.0
            || viewport.height <= 0.0
            || !(0.25..=4.0).contains(&viewport.scale_factor)
            || [rect.x, rect.y, rect.width, rect.height]
                .iter()
                .any(|v| !v.is_finite())
            || rect.width <= 0.0
            || rect.height <= 0.0
        {
            return Err("invalid_capture_region");
        }
        let logical = rect
            .intersection(viewport.logical_rect())
            .ok_or("offscreen_target")?;
        let origin = [
            (logical.x * viewport.scale_factor).floor().max(0.0) as u32,
            (logical.y * viewport.scale_factor).floor().max(0.0) as u32,
        ];
        let end = [
            ((logical.x + logical.width) * viewport.scale_factor).ceil() as u32,
            ((logical.y + logical.height) * viewport.scale_factor).ceil() as u32,
        ];
        let extent = [
            end[0].saturating_sub(origin[0]),
            end[1].saturating_sub(origin[1]),
        ];
        if extent.contains(&0)
            || extent.iter().any(|n| *n > 4096)
            || u64::from(extent[0]) * u64::from(extent[1]) > 4_194_304
        {
            return Err("invalid_capture_region");
        }
        Ok(Self {
            logical,
            origin,
            extent,
        })
    }
}
