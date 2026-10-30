//! Backend-neutral retained state and immutable display snapshots for reusable 3D viewports.

#![forbid(unsafe_code)]

pub mod display;
mod lighting;
pub mod render;
pub use lighting::SurfaceLighting;

use std::f32::consts::FRAC_PI_2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[must_use]
    pub fn plus(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    #[must_use]
    pub fn minus(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    #[must_use]
    pub fn scaled(self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor, self.z * factor)
    }

    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    #[must_use]
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    #[must_use]
    pub fn normalized(self) -> Self {
        let length = self.dot(self).sqrt();
        if length <= f32::EPSILON {
            Self::default()
        } else {
            self.scaled(1.0 / length)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewportId(u64);

impl ViewportId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RenderableId(u64);

impl RenderableId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TransformMode {
    #[default]
    Select,
    Translate,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TransformSpace {
    Local,
    #[default]
    World,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GizmoPart {
    AxisX,
    AxisY,
    AxisZ,
    PlaneXy,
    PlaneYz,
    PlaneZx,
    Screen,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GizmoState {
    pub mode: TransformMode,
    pub space: TransformSpace,
    pub active: Option<GizmoPart>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HitProxyNamespace(u64);

impl HitProxyNamespace {
    pub const RENDERABLE: Self = Self(1);
    pub const GIZMO: Self = Self(2);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Stable, renderer-independent hit meaning. Hosts reserve their own namespace and interpret
/// `owner`/`subpart`; the viewport never needs to know a product's node or geometry types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HitProxyKey {
    pub namespace: HitProxyNamespace,
    pub owner: u64,
    pub subpart: u32,
}

impl HitProxyKey {
    #[must_use]
    pub const fn new(namespace: HitProxyNamespace, owner: u64, subpart: u32) -> Self {
        Self {
            namespace,
            owner,
            subpart,
        }
    }

    #[must_use]
    pub const fn renderable(id: RenderableId) -> Self {
        Self::new(HitProxyNamespace::RENDERABLE, id.get(), 0)
    }

    #[must_use]
    pub const fn gizmo(owner: RenderableId, part: GizmoPart) -> Self {
        Self::new(HitProxyNamespace::GIZMO, owner.get(), gizmo_part_code(part))
    }
}

const fn gizmo_part_code(part: GizmoPart) -> u32 {
    match part {
        GizmoPart::AxisX => 1,
        GizmoPart::AxisY => 2,
        GizmoPart::AxisZ => 3,
        GizmoPart::PlaneXy => 4,
        GizmoPart::PlaneYz => 5,
        GizmoPart::PlaneZx => 6,
        GizmoPart::Screen => 7,
    }
}

/// Compact ID written by the GPU. Zero is reserved for "no hit".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HitProxyId(u32);

impl HitProxyId {
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Per-frame indirection from compact GPU IDs to open-ended host semantic keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HitProxyTable {
    frame_revision: u64,
    entries: Vec<HitProxyKey>,
}

impl HitProxyTable {
    #[must_use]
    pub const fn frame_revision(&self) -> u64 {
        self.frame_revision
    }

    pub fn begin_frame(&mut self, frame_revision: u64) {
        self.frame_revision = frame_revision;
        self.entries.clear();
    }

    pub fn register(&mut self, key: HitProxyKey) -> Result<HitProxyId, HitProxyTableFull> {
        let raw = u32::try_from(self.entries.len() + 1).map_err(|_| HitProxyTableFull)?;
        self.entries.push(key);
        Ok(HitProxyId(raw))
    }

    #[must_use]
    pub fn resolve(&self, frame_revision: u64, id: HitProxyId) -> Option<HitProxyKey> {
        if frame_revision != self.frame_revision || id.0 == 0 {
            return None;
        }
        self.entries.get(id.0 as usize - 1).copied()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitProxyTableFull;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PickRequestId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickRequest {
    pub id: PickRequestId,
    pub viewport: ViewportId,
    pub physical_x: u32,
    pub physical_y: u32,
    pub frame_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickResult {
    pub request: PickRequestId,
    pub frame_revision: u64,
    pub target: Option<HitProxyKey>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransformAction {
    Begin {
        target: RenderableId,
        part: GizmoPart,
        space: TransformSpace,
    },
    Update {
        delta: Vec3,
    },
    Commit,
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Clockwise screen rotation in radians; not an author-data parameter.
    pub roll: f32,
    pub distance: f32,
    pub vertical_fov_radians: f32,
    pub near: f32,
    pub far: f32,
    pub projection: Projection,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Projection {
    #[default]
    Perspective,
    Orthographic {
        vertical_size: f32,
    },
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target: Vec3::new(0.0, 0.4, 0.0),
            yaw: -0.7,
            pitch: -0.45,
            roll: 0.0,
            distance: 8.75,
            vertical_fov_radians: 50.0_f32.to_radians(),
            near: 0.05,
            far: 500.0,
            projection: Projection::Perspective,
        }
    }
}

impl Camera {
    pub fn orbit(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw -= delta_x * 0.008;
        self.pitch = (self.pitch - delta_y * 0.008).clamp(-FRAC_PI_2 + 0.02, FRAC_PI_2 - 0.02);
    }

    pub fn dolly(&mut self, delta: f32) {
        let factor = (-delta * 0.12).exp();
        self.distance = (self.distance * factor).clamp(0.25, 500.0);
        if let Projection::Orthographic { vertical_size } = &mut self.projection {
            *vertical_size = (*vertical_size * factor).clamp(0.1, 1000.0);
        }
    }

    pub fn pan(&mut self, delta_x: f32, delta_y: f32) {
        let (right, up) = self.screen_axes();
        let scale = self.distance * 0.0015;
        self.target = self
            .target
            .plus(right.scaled(-delta_x * scale))
            .plus(up.scaled(delta_y * scale));
    }

    #[must_use]
    pub fn eye(self) -> Vec3 {
        self.target.plus(self.forward().scaled(-self.distance))
    }

    #[must_use]
    pub fn forward(self) -> Vec3 {
        let (yaw_sin, yaw_cos) = self.yaw.sin_cos();
        let (pitch_sin, pitch_cos) = self.pitch.sin_cos();
        Vec3::new(yaw_sin * pitch_cos, pitch_sin, yaw_cos * pitch_cos).normalized()
    }
    #[must_use]
    pub fn screen_axes(self) -> (Vec3, Vec3) {
        let forward = self.forward();
        let horizontal = forward.cross(Vec3::new(0.0, 1.0, 0.0));
        let right = if horizontal.dot(horizontal) > 1.0e-12 {
            horizontal.normalized()
        } else {
            let (s, c) = self.yaw.sin_cos();
            Vec3::new(-c, 0.0, s)
        };
        let up = right.cross(forward).normalized();
        let (s, c) = self.roll.sin_cos();
        (
            right.scaled(c).plus(up.scaled(s)),
            up.scaled(c).minus(right.scaled(s)),
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Renderable {
    pub id: RenderableId,
    pub mesh: Mesh,
    pub translation: Vec3,
    pub color: [f32; 4],
    pub selected: bool,
}

/// An invisible pickable mesh. Hosts can assign arbitrary semantic keys to tool handles,
/// geometry components, or other custom hit regions without adding product types here.
#[derive(Clone, Debug, PartialEq)]
pub struct HitProxyShape {
    pub key: HitProxyKey,
    pub mesh: Mesh,
    pub translation: Vec3,
    pub depth: HitProxyDepth,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HitProxyDepth {
    #[default]
    Scene,
    /// Tool overlays can receive input ahead of scene geometry at the same screen position.
    Overlay,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewportScene {
    pub display: display::DisplayOptions,
    /// Optional world-space line pairs drawn with the Grid toggle. None retains
    /// the generic grid; Some(empty) explicitly supplies no grid geometry.
    pub grid_lines: Option<Vec<render::LineVertex>>,
    pub sources: Option<display::DisplaySources>,
    pub renderables: Vec<Renderable>,
    pub hit_proxies: Vec<HitProxyShape>,
    pub gizmo: GizmoState,
    /// Optional per-object display material. Absent entries retain solid shading.
    pub materials: std::collections::BTreeMap<RenderableId, SurfaceMaterial>,
    /// Optional RGB multipliers in each renderable's Mesh vertex order.
    /// Missing entries preserve the object's color; alpha stays object-owned.
    pub vertex_colors: std::collections::BTreeMap<RenderableId, Vec<[f32; 3]>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurfaceMaterial {
    Lit {
        lighting: SurfaceLighting,
        normals: LightingNormals,
    },
    Terrain {
        height_range: [f32; 2],
        lighting: SurfaceLighting,
        normals: LightingNormals,
    },
}

/// Lighting orientation is independent of raster culling and diagnostic normals.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightingNormals {
    Source,
    FaceViewer,
}

impl ViewportScene {
    pub fn select(&mut self, selected: Option<RenderableId>) {
        for renderable in &mut self.renderables {
            renderable.selected = Some(renderable.id) == selected;
        }
    }
}

/// Picks the nearest renderable using normalized device coordinates in `[-1, 1]`.
/// The reference contract uses each renderable's unit-cube bounds; backends may add an
/// accelerated implementation as long as it produces the same stable identity result.
#[must_use]
pub fn pick(
    scene: &ViewportScene,
    camera: Camera,
    aspect: f32,
    ndc_x: f32,
    ndc_y: f32,
) -> Option<RenderableId> {
    let (origin, direction) = pick_ray(camera, aspect, ndc_x, ndc_y);
    scene
        .renderables
        .iter()
        .filter_map(|renderable| {
            ray_mesh(origin, direction, &renderable.mesh, renderable.translation)
                .map(|distance| (distance, renderable.id))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map(|(_, id)| id)
}

/// Returns the nearest triangle hit, with overlay hit regions taking precedence over scene
/// surfaces. Every returned key is supplied by the host or derived from a renderable ID.
#[must_use]
pub fn pick_target(
    scene: &ViewportScene,
    camera: Camera,
    aspect: f32,
    ndc_x: f32,
    ndc_y: f32,
) -> Option<HitProxyKey> {
    let (origin, direction) = pick_ray(camera, aspect, ndc_x, ndc_y);
    let scene_hits = scene.renderables.iter().map(|renderable| {
        (
            HitProxyKey::renderable(renderable.id),
            &renderable.mesh,
            renderable.translation,
        )
    });
    let custom_scene_hits = scene
        .hit_proxies
        .iter()
        .filter(|shape| shape.depth == HitProxyDepth::Scene)
        .map(|shape| (shape.key, &shape.mesh, shape.translation));
    let overlay_hits = scene
        .hit_proxies
        .iter()
        .filter(|shape| shape.depth == HitProxyDepth::Overlay)
        .filter_map(|shape| {
            ray_mesh(origin, direction, &shape.mesh, shape.translation)
                .map(|distance| (distance, shape.key))
        })
        .min_by(|left, right| left.0.total_cmp(&right.0));
    overlay_hits
        .or_else(|| {
            scene_hits
                .chain(custom_scene_hits)
                .filter_map(|(key, mesh, translation)| {
                    ray_mesh(origin, direction, mesh, translation).map(|distance| (distance, key))
                })
                .min_by(|left, right| left.0.total_cmp(&right.0))
        })
        .map(|(_, key)| key)
}

/// A backend-neutral, one-axis translation gesture. The host owns the
/// authoritative object transform and applies each preview/commit action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TranslationDrag {
    target: RenderableId,
    part: GizmoPart,
    space: TransformSpace,
    original: Vec3,
    axis: Vec3,
    start_cursor: [f32; 2],
    screen_axis: [f32; 2],
}

impl TranslationDrag {
    /// Hit-tests the visible translation axis lines in logical window pixels.
    #[must_use]
    pub fn begin(
        scene: &ViewportScene,
        camera: Camera,
        width: f32,
        height: f32,
        cursor_x: f32,
        cursor_y: f32,
    ) -> Option<Self> {
        if scene.gizmo.mode != TransformMode::Translate || width <= 0.0 || height <= 0.0 {
            return None;
        }
        const AXIS_LENGTH: f32 = 1.3;
        const HIT_RADIUS_SQUARED: f32 = 10.0 * 10.0;
        let cursor = [cursor_x, cursor_y];
        scene
            .renderables
            .iter()
            .filter(|renderable| renderable.selected)
            .flat_map(|renderable| {
                [
                    (GizmoPart::AxisX, Vec3::new(1.0, 0.0, 0.0)),
                    (GizmoPart::AxisY, Vec3::new(0.0, 1.0, 0.0)),
                    (GizmoPart::AxisZ, Vec3::new(0.0, 0.0, 1.0)),
                ]
                .into_iter()
                .filter_map(move |(part, axis)| {
                    let start = project_to_pixel(camera, width, height, renderable.translation)?;
                    let end = project_to_pixel(
                        camera,
                        width,
                        height,
                        renderable.translation.plus(axis.scaled(AXIS_LENGTH)),
                    )?;
                    let screen_axis = [end[0] - start[0], end[1] - start[1]];
                    let squared = screen_axis[0] * screen_axis[0] + screen_axis[1] * screen_axis[1];
                    if squared < 16.0 {
                        return None;
                    }
                    let distance = distance_squared_to_segment(cursor, start, end);
                    (distance <= HIT_RADIUS_SQUARED).then_some((
                        distance,
                        Self {
                            target: renderable.id,
                            part,
                            space: scene.gizmo.space,
                            original: renderable.translation,
                            axis,
                            start_cursor: cursor,
                            screen_axis,
                        },
                    ))
                })
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .map(|(_, drag)| drag)
    }

    #[must_use]
    pub const fn target(self) -> RenderableId {
        self.target
    }

    #[must_use]
    pub const fn part(self) -> GizmoPart {
        self.part
    }

    #[must_use]
    pub const fn original(self) -> Vec3 {
        self.original
    }

    #[must_use]
    pub const fn begin_action(self) -> TransformAction {
        TransformAction::Begin {
            target: self.target,
            part: self.part,
            space: self.space,
        }
    }

    #[must_use]
    pub fn update(self, cursor_x: f32, cursor_y: f32) -> TransformAction {
        let motion = [
            cursor_x - self.start_cursor[0],
            cursor_y - self.start_cursor[1],
        ];
        let denominator =
            self.screen_axis[0] * self.screen_axis[0] + self.screen_axis[1] * self.screen_axis[1];
        let units =
            (motion[0] * self.screen_axis[0] + motion[1] * self.screen_axis[1]) / denominator * 1.3;
        TransformAction::Update {
            delta: self.axis.scaled(units),
        }
    }
}

/// Projects a world-space point to logical top-left-origin viewport pixels.
#[must_use]
pub fn project_to_pixel(camera: Camera, width: f32, height: f32, point: Vec3) -> Option<[f32; 2]> {
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let forward = camera.forward();
    let (right, up) = camera.screen_axes();
    let relative = point.minus(camera.eye());
    let depth = relative.dot(forward);
    if depth <= camera.near || !depth.is_finite() {
        return None;
    }
    let half_height = match camera.projection {
        Projection::Perspective => depth * (camera.vertical_fov_radians * 0.5).tan(),
        Projection::Orthographic { vertical_size } => vertical_size.max(0.01) * 0.5,
    };
    let half_width = half_height * (width / height);
    let ndc_x = relative.dot(right) / half_width;
    let ndc_y = -relative.dot(up) / half_height;
    (ndc_x.is_finite() && ndc_y.is_finite())
        .then_some([(ndc_x + 1.0) * 0.5 * width, (ndc_y + 1.0) * 0.5 * height])
}

fn distance_squared_to_segment(point: [f32; 2], start: [f32; 2], end: [f32; 2]) -> f32 {
    let axis = [end[0] - start[0], end[1] - start[1]];
    let offset = [point[0] - start[0], point[1] - start[1]];
    let squared = axis[0] * axis[0] + axis[1] * axis[1];
    let t = ((offset[0] * axis[0] + offset[1] * axis[1]) / squared).clamp(0.0, 1.0);
    let dx = offset[0] - t * axis[0];
    let dy = offset[1] - t * axis[1];
    dx * dx + dy * dy
}

/// Ray using the renderer's projection and rolled screen axes; NDC y grows down.
pub fn pick_ray(camera: Camera, aspect: f32, ndc_x: f32, ndc_y: f32) -> (Vec3, Vec3) {
    let forward = camera.forward();
    let (right, up) = camera.screen_axes();
    match camera.projection {
        Projection::Perspective => {
            let vertical = (camera.vertical_fov_radians * 0.5).tan();
            let direction = forward
                .plus(right.scaled(ndc_x * vertical * aspect.max(0.001)))
                .plus(up.scaled(-ndc_y * vertical))
                .normalized();
            (camera.eye(), direction)
        }
        Projection::Orthographic { vertical_size } => {
            let half_height = vertical_size.max(0.01) * 0.5;
            let origin = camera
                .eye()
                .plus(right.scaled(ndc_x * half_height * aspect.max(0.001)))
                .plus(up.scaled(-ndc_y * half_height));
            (origin, forward)
        }
    }
}

fn ray_mesh(origin: Vec3, direction: Vec3, mesh: &Mesh, translation: Vec3) -> Option<f32> {
    mesh.indices
        .chunks_exact(3)
        .filter_map(|triangle| {
            let a = mesh.vertices.get(triangle[0] as usize)?;
            let b = mesh.vertices.get(triangle[1] as usize)?;
            let c = mesh.vertices.get(triangle[2] as usize)?;
            ray_triangle(
                origin,
                direction,
                Vec3::new(a.position[0], a.position[1], a.position[2]).plus(translation),
                Vec3::new(b.position[0], b.position[1], b.position[2]).plus(translation),
                Vec3::new(c.position[0], c.position[1], c.position[2]).plus(translation),
            )
        })
        .min_by(f32::total_cmp)
}

fn ray_triangle(origin: Vec3, direction: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let edge_ab = b.minus(a);
    let edge_ac = c.minus(a);
    let p = direction.cross(edge_ac);
    let determinant = edge_ab.dot(p);
    if determinant.abs() <= 1e-7 {
        return None;
    }
    let inverse = 1.0 / determinant;
    let offset = origin.minus(a);
    let u = offset.dot(p) * inverse;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = offset.cross(edge_ab);
    let v = direction.dot(q) * inverse;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let distance = edge_ac.dot(q) * inverse;
    (distance.is_finite() && distance > 0.0).then_some(distance)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewportAction {
    Orbit { delta_x: f32, delta_y: f32 },
    Pan { delta_x: f32, delta_y: f32 },
    Dolly { delta: f32 },
    FrameAll,
    ToggleProjection,
}

pub fn apply_action(camera: &mut Camera, action: ViewportAction) {
    match action {
        ViewportAction::Orbit { delta_x, delta_y } => camera.orbit(delta_x, delta_y),
        ViewportAction::Pan { delta_x, delta_y } => camera.pan(delta_x, delta_y),
        ViewportAction::Dolly { delta } => camera.dolly(delta),
        ViewportAction::FrameAll => *camera = Camera::default(),
        ViewportAction::ToggleProjection => {
            camera.projection = match camera.projection {
                Projection::Perspective => Projection::Orthographic {
                    vertical_size: camera.distance.max(1.0),
                },
                Projection::Orthographic { .. } => Projection::Perspective,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl ViewportScene {
        #[must_use]
        fn acceptance_scene() -> Self {
            Self {
                display: Default::default(),
                grid_lines: None,
                sources: None,
                renderables: vec![
                    Renderable {
                        id: RenderableId::new(1),
                        mesh: cube_mesh(),
                        translation: Vec3::new(-0.75, 0.7, 0.0),
                        color: [0.18, 0.50, 0.95, 1.0],
                        selected: true,
                    },
                    Renderable {
                        id: RenderableId::new(2),
                        mesh: cube_mesh(),
                        translation: Vec3::new(0.85, 0.45, 0.45),
                        color: [0.93, 0.42, 0.20, 1.0],
                        selected: false,
                    },
                ],
                hit_proxies: Vec::new(),
                materials: Default::default(),
                vertex_colors: Default::default(),
                gizmo: GizmoState {
                    mode: TransformMode::Translate,
                    space: TransformSpace::World,
                    active: None,
                },
            }
        }
    }

    fn cube_mesh() -> Mesh {
        let positions = [
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [0.5, -0.5, -0.5],
            [-0.5, -0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [0.5, 0.5, -0.5],
        ];
        let vertices = positions
            .into_iter()
            .map(|position| Vertex {
                position,
                normal: Vec3::new(position[0], position[1], position[2])
                    .normalized()
                    .into_array(),
            })
            .collect();
        Mesh {
            vertices,
            indices: vec![
                0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7, 5, 0, 3, 5, 3, 6, 1, 4, 7, 1, 7, 2, 3, 2, 7, 3,
                7, 6, 5, 4, 1, 5, 1, 0,
            ],
        }
    }

    impl Vec3 {
        const fn into_array(self) -> [f32; 3] {
            [self.x, self.y, self.z]
        }
    }

    #[test]
    fn rolled_camera_projection_pick_ray_and_pan_share_screen_axes() {
        let (width, height) = (800.0, 600.0);
        for projection in [
            Projection::Perspective,
            Projection::Orthographic { vertical_size: 6.0 },
        ] {
            for quarter in 0..4 {
                let camera = Camera {
                    roll: quarter as f32 * FRAC_PI_2,
                    projection,
                    ..Camera::default()
                };
                let (right, up) = camera.screen_axes();
                assert!(right.dot(up).abs() < 0.00001);
                let point = camera.target.plus(right.scaled(0.2)).plus(up.scaled(0.1));
                let pixel = project_to_pixel(camera, width, height, point).expect("visible point");
                let ndc = [pixel[0] / width * 2.0 - 1.0, pixel[1] / height * 2.0 - 1.0];
                let (origin, direction) = pick_ray(camera, width / height, ndc[0], ndc[1]);
                assert!(
                    point
                        .minus(origin)
                        .cross(direction)
                        .dot(point.minus(origin).cross(direction))
                        < 0.00001
                );
                let matrix = render::view_projection(camera, width / height);
                let uniform = render::camera_uniform(camera, width / height);
                assert_eq!(&uniform[..4], &matrix);
                let to_eye = if uniform[4][3] > 0.5 {
                    Vec3::new(uniform[4][0], uniform[4][1], uniform[4][2]).minus(point)
                } else {
                    Vec3::new(-uniform[5][0], -uniform[5][1], -uniform[5][2])
                };
                assert!(
                    to_eye.normalized().dot(direction) < -0.99999,
                    "GPU two-sided sight lines must match perspective/orthographic picking, including roll"
                );
                let world = [point.x, point.y, point.z, 1.0];
                let clip: [f32; 4] = std::array::from_fn(|row| {
                    (0..4)
                        .map(|column| matrix[column][row] * world[column])
                        .sum()
                });
                assert!((clip[0] / clip[3] - ndc[0]).abs() < 0.00001);
                assert!((clip[1] / clip[3] + ndc[1]).abs() < 0.00001);
                let mut panned = camera;
                panned.pan(10.0, 0.0);
                let delta = panned.target.minus(camera.target);
                assert!(delta.dot(up).abs() < 0.00001);
                assert!((delta.dot(right) + camera.distance * 0.015).abs() < 0.00001);
            }
        }
    }

    #[test]
    fn picking_returns_a_stable_scene_identity() {
        let scene = ViewportScene::acceptance_scene();
        let picked = pick(&scene, Camera::default(), 16.0 / 9.0, 0.0, 0.0);
        assert!(picked.is_some());
    }

    #[test]
    fn custom_hit_proxy_keys_round_trip_and_reject_stale_frames() {
        let custom = HitProxyKey::new(HitProxyNamespace::new(0x0056_4553_5441), 42, 7);
        let mut table = HitProxyTable::default();
        table.begin_frame(10);
        let id = table.register(custom).expect("small table has capacity");
        assert_eq!(table.resolve(10, id), Some(custom));
        assert_eq!(table.resolve(9, id), None);
        table.begin_frame(11);
        assert_eq!(table.resolve(11, id), None);
    }

    #[test]
    fn custom_overlay_mesh_can_win_a_hit_without_becoming_a_renderable() {
        let id = RenderableId::new(7);
        let custom = HitProxyKey::new(HitProxyNamespace::new(42), 90, 3);
        let scene = ViewportScene {
            display: Default::default(),
            grid_lines: None,
            sources: None,
            renderables: vec![Renderable {
                id,
                mesh: cube_mesh(),
                translation: Vec3::new(0.0, 0.0, 0.0),
                color: [1.0; 4],
                selected: false,
            }],
            hit_proxies: vec![HitProxyShape {
                key: custom,
                mesh: cube_mesh(),
                translation: Vec3::new(0.0, 0.0, 0.0),
                depth: HitProxyDepth::Overlay,
            }],
            gizmo: GizmoState::default(),
            materials: Default::default(),
            vertex_colors: Default::default(),
        };
        let camera = Camera::default();
        assert_eq!(pick(&scene, camera, 1.0, 0.0, 0.0), Some(id));
        assert_eq!(pick_target(&scene, camera, 1.0, 0.0, 0.0), Some(custom));
    }

    #[test]
    fn translation_drag_hits_a_visible_axis_and_returns_stable_world_delta() {
        let scene = ViewportScene::acceptance_scene();
        let camera = Camera::default();
        let selected = &scene.renderables[0];
        let start = project_to_pixel(camera, 1280.0, 800.0, selected.translation)
            .expect("selected center projects");
        let end = project_to_pixel(
            camera,
            1280.0,
            800.0,
            selected.translation.plus(Vec3::new(1.3, 0.0, 0.0)),
        )
        .expect("x axis projects");
        let cursor = [start[0] * 0.4 + end[0] * 0.6, start[1] * 0.4 + end[1] * 0.6];
        let drag = TranslationDrag::begin(&scene, camera, 1280.0, 800.0, cursor[0], cursor[1])
            .expect("x axis hit");
        assert_eq!(drag.target(), selected.id);
        assert_eq!(drag.part(), GizmoPart::AxisX);
        assert_eq!(
            drag.begin_action(),
            TransformAction::Begin {
                target: selected.id,
                part: GizmoPart::AxisX,
                space: TransformSpace::World,
            }
        );
        let advanced = [
            cursor[0] + (end[0] - start[0]) * 0.25,
            cursor[1] + (end[1] - start[1]) * 0.25,
        ];
        let TransformAction::Update { delta } = drag.update(advanced[0], advanced[1]) else {
            panic!("drag must update");
        };
        assert!((delta.x - 0.325).abs() < 1e-4);
        assert_eq!(delta.y, 0.0);
        assert_eq!(delta.z, 0.0);
    }
}
