//! One owner for renderer projection, CPU rays, point projection and camera state.
use crate::Vec3;
use std::f32::consts::FRAC_PI_2;

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
pub enum ViewportAction {
    Orbit { delta_x: f32, delta_y: f32 },
    Pan { delta_x: f32, delta_y: f32 },
    Dolly { delta: f32 },
    ResetCamera,
    ToggleProjection,
}

pub fn apply_action(camera: &mut Camera, action: ViewportAction) {
    match action {
        ViewportAction::Orbit { delta_x, delta_y } => camera.orbit(delta_x, delta_y),
        ViewportAction::Pan { delta_x, delta_y } => camera.pan(delta_x, delta_y),
        ViewportAction::Dolly { delta } => camera.dolly(delta),
        ViewportAction::ResetCamera => *camera = Camera::default(),
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

/// Projects a world point to top-left-origin pixels using the renderer's exact
/// projection. XY may lie outside the viewport; clipped depth or non-finite
/// output returns None.
#[must_use]
pub fn project_to_pixel(camera: Camera, width: f32, height: f32, point: Vec3) -> Option<[f32; 2]> {
    let aspect = width / height;
    if ![width, height, aspect]
        .into_iter()
        .all(|v| v.is_finite() && v > 0.0)
    {
        return None;
    }
    let clip = clip_point(&view_projection(camera, aspect), point)?;
    let pixel = [
        (clip[0] / clip[3] + 1.0) * 0.5 * width,
        (1.0 - clip[1] / clip[3]) * 0.5 * height,
    ];
    pixel.into_iter().all(f32::is_finite).then_some(pixel)
}

/// Ray using the renderer's exact projection and rolled axes; NDC y grows down.
/// This raw helper requires valid finite camera/aspect/NDC inputs. Semantic
/// pick/pick_target additionally validate query inputs and camera clip depth.
pub fn pick_ray(camera: Camera, aspect: f32, ndc_x: f32, ndc_y: f32) -> (Vec3, Vec3) {
    let forward = camera.forward();
    let (right, up) = camera.screen_axes();
    let matrix = projection(camera, aspect);
    let horizontal = ndc_x / matrix[0][0];
    let vertical = -ndc_y / matrix[1][1];
    match camera.projection {
        Projection::Perspective => (
            camera.eye(),
            forward
                .plus(right.scaled(horizontal))
                .plus(up.scaled(vertical))
                .normalized(),
        ),
        Projection::Orthographic { .. } => (
            camera
                .eye()
                .plus(right.scaled(horizontal))
                .plus(up.scaled(vertical)),
            forward,
        ),
    }
}

pub(crate) fn clip_point(matrix: &[[f32; 4]; 4], point: Vec3) -> Option<[f32; 4]> {
    let world = [point.x, point.y, point.z, 1.0];
    let clip: [f32; 4] = std::array::from_fn(|row| {
        (0..4)
            .map(|column| matrix[column][row] * world[column])
            .sum()
    });
    (clip.into_iter().all(f32::is_finite) && clip[3] > 0.0 && clip[2] >= 0.0 && clip[2] <= clip[3])
        .then_some(clip)
}

/// Shader camera block: view/projection columns, eye + perspective flag, forward.
/// Orthographic lighting uses parallel sight lines rather than a finite eye.
#[must_use]
pub fn camera_uniform(camera: Camera, aspect: f32) -> [[f32; 4]; 6] {
    let matrix = view_projection(camera, aspect);
    let eye = camera.eye();
    let forward = camera.forward();
    [
        matrix[0],
        matrix[1],
        matrix[2],
        matrix[3],
        [
            eye.x,
            eye.y,
            eye.z,
            if matches!(camera.projection, Projection::Perspective) {
                1.0
            } else {
                0.0
            },
        ],
        [forward.x, forward.y, forward.z, 0.0],
    ]
}

pub fn view_projection(camera: Camera, aspect: f32) -> [[f32; 4]; 4] {
    multiply(projection(camera, aspect), look_at(camera))
}

fn projection(camera: Camera, aspect: f32) -> [[f32; 4]; 4] {
    match camera.projection {
        Projection::Perspective => {
            perspective(camera.vertical_fov_radians, aspect, camera.near, camera.far)
        }
        Projection::Orthographic { vertical_size } => {
            orthographic(vertical_size, aspect, camera.near, camera.far)
        }
    }
}

fn look_at(camera: Camera) -> [[f32; 4]; 4] {
    let eye = camera.eye();
    let forward = camera.forward();
    let (right, up) = camera.screen_axes();
    [
        [right.x, up.x, -forward.x, 0.0],
        [right.y, up.y, -forward.y, 0.0],
        [right.z, up.z, -forward.z, 0.0],
        [-right.dot(eye), -up.dot(eye), forward.dot(eye), 1.0],
    ]
}

fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let factor = 1.0 / (fov * 0.5).tan();
    [
        [factor / aspect.max(0.001), 0.0, 0.0, 0.0],
        [0.0, factor, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), -1.0],
        [0.0, 0.0, near * far / (near - far), 0.0],
    ]
}

fn orthographic(vertical_size: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    let height = vertical_size.max(0.01);
    let width = (height * aspect.max(0.001)).max(0.001);
    let depth = (far - near).max(0.001);
    [
        [2.0 / width, 0.0, 0.0, 0.0],
        [0.0, 2.0 / height, 0.0, 0.0],
        [0.0, 0.0, -1.0 / depth, 0.0],
        [0.0, 0.0, -near / depth, 1.0],
    ]
}

fn multiply(left: [[f32; 4]; 4], right: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut output = [[0.0; 4]; 4];
    for column in 0..4 {
        for row in 0..4 {
            output[column][row] = (0..4)
                .map(|index| left[index][row] * right[column][index])
                .sum();
        }
    }
    output
}
