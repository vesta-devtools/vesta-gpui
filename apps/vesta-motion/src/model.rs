use crate::study::Study;
use vesta_gpui::{Color, ElementId, ElementIdAllocator, ElementScope};
use vesta_gpui_widgets::{WidgetIdentity, WidgetTheme};

pub const DURATION: f32 = 12.0;
pub const SURFACE: u32 = 600;
pub const SEEK: u32 = 1000;
pub const PLAY: u32 = 70;
pub const MINT: Color = Color::rgb8(132, 218, 193);
pub const GOLD: Color = Color::rgb8(225, 180, 111);
pub const MUTED: Color = Color::rgb8(142, 150, 155);
pub const BORDER: Color = Color::rgb8(49, 54, 58);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Ribbon,
    Trails,
    Title,
    Background,
}
impl Layer {
    pub const ALL: [Self; 4] = [Self::Ribbon, Self::Trails, Self::Title, Self::Background];
    pub const fn slot(self) -> u32 {
        match self {
            Self::Ribbon => 110,
            Self::Trails => 130,
            Self::Title => 150,
            Self::Background => 170,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ribbon => "Ribbon Array",
            Self::Trails => "Orbit Trails",
            Self::Title => "Title",
            Self::Background => "Background",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Parameter {
    Count,
    Radius,
    Twist,
    Speed,
}
impl Parameter {
    #[cfg(test)]
    pub const ALL: [Self; 4] = [Self::Count, Self::Radius, Self::Twist, Self::Speed];
    pub const fn slot(self) -> u32 {
        match self {
            Self::Count => 720,
            Self::Radius => 740,
            Self::Twist => 760,
            Self::Speed => 780,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Count => "Count",
            Self::Radius => "Radius",
            Self::Twist => "Twist",
            Self::Speed => "Playback speed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Study(Study),
    Tour,
    Play,
    Start,
    End,
    Loop,
    Reset,
    Select(Layer),
    Visibility(Layer),
    Parameter(Parameter),
    Seek,
    Palette(u8),
    Guides,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    pub count: u32,
    pub radius: f32,
    pub twist: f32,
    pub speed: f32,
    pub palette: u8,
}
impl Default for Parameters {
    fn default() -> Self {
        Study::Orbit.defaults()
    }
}

pub struct Model {
    pub scope: ElementScope,
    pub theme: WidgetTheme,
    parameters: [Parameters; 6],
    pub study: Study,
    pub tour: bool,
    pub time: f32,
    pub playing: bool,
    pub looping: bool,
    pub selected: Layer,
    pub visible: [bool; 4],
    pub guides: bool,
    pub hovered: Option<ElementId>,
    pub status: String,
}
impl Model {
    pub fn new() -> Result<Self, String> {
        let mut theme = WidgetTheme::dark();
        theme.palette.canvas = Color::rgb8(11, 15, 17);
        theme.palette.panel = Color::rgb8(29, 33, 36);
        theme.palette.surface = Color::rgb8(35, 40, 43);
        theme.palette.surface_hovered = Color::rgb8(47, 55, 57);
        theme.palette.surface_pressed = Color::rgb8(21, 26, 28);
        theme.palette.field = Color::rgb8(23, 27, 30);
        theme.palette.border = BORDER;
        theme.palette.accent = MINT;
        theme.palette.accent_hovered = Color::rgb8(160, 236, 215);
        theme.palette.focus = MINT;
        theme.palette.selection = Color::rgb8(49, 77, 71);
        theme.palette.menu_selected = Color::rgb8(44, 68, 62);
        theme.palette.text = Color::rgb8(224, 229, 229);
        theme.palette.text_muted = MUTED;
        theme.metrics.control_height = 28.0;
        theme.metrics.radius = 3.0;
        theme.typography.font_size = 12.0;
        theme.typography.small_font_size = 11.0;
        Ok(Self {
            scope: ElementIdAllocator
                .allocate_scope()
                .map_err(|e| format!("motion.identity: {e:?}"))?,
            theme,
            parameters: Study::ALL.map(Study::defaults),
            study: Study::Orbit,
            tour: false,
            time: 0.0,
            playing: true,
            looping: true,
            selected: Layer::Ribbon,
            visible: [true; 4],
            guides: true,
            hovered: None,
            status: "Ready".into(),
        })
    }
    pub fn id(&self, slot: u32) -> ElementId {
        self.scope.element(slot)
    }
    pub fn identity(&self, slot: u32) -> WidgetIdentity {
        WidgetIdentity::new(self.id(slot), format!("motion.{slot}"))
    }
    pub fn params(&self) -> Parameters {
        self.parameters[self.study.index()]
    }
    pub fn restore_params(&mut self, params: Parameters) {
        self.parameters[self.study.index()] = params;
    }
    pub fn layer_name(&self, layer: Layer) -> &'static str {
        match (layer, self.study) {
            (Layer::Ribbon, study) => study.geometry(),
            (Layer::Trails, Study::Wave) => "Stage Grid",
            (Layer::Trails, Study::Helix) => "Cross Links",
            (Layer::Trails, Study::Spiral) => "Guide Spiral",
            _ => layer.name(),
        }
    }
    pub fn visible(&self, layer: Layer) -> bool {
        self.visible[match layer {
            Layer::Ribbon => 0,
            Layer::Trails => 1,
            Layer::Title => 2,
            Layer::Background => 3,
        }]
    }
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Study(study) => {
                self.study = study;
                self.tour = false;
                self.time = 0.0;
                self.selected = Layer::Ribbon;
                self.visible = [true; 4];
                self.status = study.category().into();
            }
            Action::Tour => {
                self.tour = !self.tour;
                if self.tour {
                    self.playing = true;
                    self.looping = true;
                    self.time = 0.0;
                }
            }
            Action::Play => self.playing = !self.playing,
            Action::Start => self.time = 0.0,
            Action::End => {
                self.time = DURATION;
                self.playing = false;
            }
            Action::Loop => {
                self.looping = !self.looping;
                if !self.looping {
                    self.tour = false;
                }
            }
            Action::Reset => {
                self.parameters[self.study.index()] = self.study.defaults();
                self.time = 0.0;
                self.visible = [true; 4];
            }
            Action::Select(layer) => self.selected = layer,
            Action::Visibility(layer) => {
                let index = match layer {
                    Layer::Ribbon => 0,
                    Layer::Trails => 1,
                    Layer::Title => 2,
                    Layer::Background => 3,
                };
                self.visible[index] = !self.visible[index];
            }
            Action::Palette(palette) => {
                self.parameters[self.study.index()].palette = palette.min(2)
            }
            Action::Guides => self.guides = !self.guides,
            Action::Parameter(_) | Action::Seek => (),
        }
    }
    pub fn advance(&mut self, elapsed: f32) {
        if !self.playing || !elapsed.is_finite() || elapsed < 0.0 {
            return;
        }
        let time = self.time + elapsed * self.params().speed;
        if !time.is_finite() {
            return;
        }
        if time >= DURATION {
            if self.tour {
                for _ in 0..(time / DURATION) as usize % Study::ALL.len() {
                    self.study = self.study.next();
                }
                self.selected = Layer::Ribbon;
                self.status = self.study.category().into();
            }
            self.time = if self.looping {
                time.rem_euclid(DURATION)
            } else {
                DURATION
            };
            if !self.looping {
                self.playing = false;
            }
        } else {
            self.time = time;
        }
    }
    pub fn seek(&mut self, normalized: f32) {
        if !normalized.is_finite() {
            return;
        }
        self.time = normalized.clamp(0.0, 1.0) * DURATION;
    }
    pub fn value(&self, param: Parameter) -> f32 {
        match param {
            Parameter::Count => (self.params().count as f32 - 24.0) / 72.0,
            Parameter::Radius => (self.params().radius - 1.3) / 1.4,
            Parameter::Twist => (self.params().twist - 0.2) / 1.6,
            Parameter::Speed => (self.params().speed - 0.25) / 1.75,
        }
    }
    pub fn set(&mut self, param: Parameter, normalized: f32) {
        if !normalized.is_finite() {
            return;
        }
        let value = normalized.clamp(0.0, 1.0);
        let params = &mut self.parameters[self.study.index()];
        match param {
            Parameter::Count => params.count = (24.0 + 72.0 * value).round() as u32,
            Parameter::Radius => params.radius = 1.3 + 1.4 * value,
            Parameter::Twist => params.twist = 0.2 + 1.6 * value,
            Parameter::Speed => params.speed = 0.25 + 1.75 * value,
        }
    }
    pub fn formatted(&self, param: Parameter) -> String {
        match param {
            Parameter::Count => self.params().count.to_string(),
            Parameter::Radius => match self.study {
                Study::Orbit => format!("{:.0}", self.params().radius * 120.0),
                Study::Wave => format!("{:.2}", self.params().radius * 0.29),
                Study::Helix => format!("{:.2}", self.params().radius * 0.45),
                Study::Bloom => format!("{:.2}", 0.3 + self.params().radius * 1.23),
                Study::Spiral => format!("{:.2}", self.params().radius * 0.9),
                Study::Sphere => format!("{:.2}", self.params().radius),
            },
            Parameter::Twist => match self.study {
                Study::Helix => format!("{:.1}", 1.0 + self.params().twist * 2.0),
                Study::Wave => format!("{:.2}", (self.params().twist * 0.55 + 0.35) * 1.65),
                Study::Sphere => format!("{:.1}°", (self.params().twist * 0.2).to_degrees()),
                Study::Bloom => format!("{:.2}", self.params().twist),
                Study::Spiral => format!("{:.2}", 1.0 + self.params().twist * 1.7),
                _ => format!("{:.1}°", self.params().twist * 145.0),
            },
            Parameter::Speed => format!("{:.2}×", self.params().speed),
        }
    }
    pub fn timecode(&self) -> String {
        let frame = (self.time * 30.0).floor() as u32;
        format!("00:00:{:02}:{:02}", frame / 30, frame % 30)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn playback_and_scrubbing_share_bounded_time_and_parameter_state() {
        let mut model = Model::new().expect("identity");
        model.seek(0.5);
        model.advance(1.0);
        assert_eq!(model.time, 7.0);
        model.apply(Action::Play);
        model.advance(2.0);
        assert_eq!(model.time, 7.0);
        model.apply(Action::Play);
        model.seek(0.99);
        model.advance(0.5);
        assert!((model.time - 0.38).abs() < 0.0001);
        model.looping = false;
        model.advance(30.0);
        assert_eq!(model.time, DURATION);
        assert!(!model.playing);
        for parameter in Parameter::ALL {
            model.set(parameter, -10.0);
            assert!((model.value(parameter) - 0.0).abs() < 0.0001);
            model.set(parameter, 10.0);
            assert!((model.value(parameter) - 1.0).abs() < 0.0001);
        }
        model.apply(Action::Reset);
        assert_eq!(model.params(), Parameters::default());
        model.set(Parameter::Count, 0.25);
        let orbit = model.params();
        model.apply(Action::Study(Study::Wave));
        model.set(Parameter::Count, 0.75);
        let wave = model.params();
        model.apply(Action::Study(Study::Orbit));
        assert_eq!(model.params(), orbit);
        model.apply(Action::Reset);
        model.apply(Action::Study(Study::Wave));
        assert_eq!(model.params(), wave, "reset may not erase another study");
        model.apply(Action::Tour);
        model.advance(DURATION + 0.5);
        assert_eq!(model.study, Study::Sphere);
        assert!((model.time - 0.5).abs() < 0.0001);
        model.apply(Action::Study(Study::Bloom));
        assert!(!model.tour, "manual selection takes over the demo reel");
    }
}
