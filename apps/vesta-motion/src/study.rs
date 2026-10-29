//! Display-study identities and presentation metadata; not an operation catalog.
use crate::model::{Parameter, Parameters};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Study {
    #[default]
    Orbit,
    Wave,
    Sphere,
    Helix,
    Bloom,
    Spiral,
}

impl Study {
    pub const ALL: [Self; 6] = [
        Self::Orbit,
        Self::Wave,
        Self::Sphere,
        Self::Helix,
        Self::Bloom,
        Self::Spiral,
    ];
    pub const fn index(self) -> usize {
        match self {
            Self::Orbit => 0,
            Self::Wave => 1,
            Self::Sphere => 2,
            Self::Helix => 3,
            Self::Bloom => 4,
            Self::Spiral => 5,
        }
    }
    pub const fn slot(self) -> u32 {
        match self {
            Self::Orbit => 20_000,
            Self::Wave => 20_100,
            Self::Sphere => 20_200,
            Self::Helix => 20_300,
            Self::Bloom => 20_400,
            Self::Spiral => 20_500,
        }
    }
    pub const fn key(self) -> &'static str {
        match self {
            Self::Orbit => "orbit",
            Self::Wave => "wave-field",
            Self::Sphere => "orbital-sphere",
            Self::Helix => "double-helix",
            Self::Bloom => "kinetic-bloom",
            Self::Spiral => "prism-spiral",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|study| study.key() == key)
    }
    pub const fn title(self) -> &'static str {
        match self {
            Self::Orbit => "Orbit Study",
            Self::Wave => "Wave Field",
            Self::Sphere => "Orbital Sphere",
            Self::Helix => "Double Helix",
            Self::Bloom => "Kinetic Bloom",
            Self::Spiral => "Prism Spiral",
        }
    }
    pub const fn heading(self) -> &'static str {
        match self {
            Self::Orbit => "O R B I T",
            Self::Wave => "W A V E",
            Self::Sphere => "S P H E R E",
            Self::Helix => "H E L I X",
            Self::Bloom => "B L O O M",
            Self::Spiral => "S P I R A L",
        }
    }
    pub const fn geometry(self) -> &'static str {
        match self {
            Self::Orbit => "Ribbon Array",
            Self::Wave => "Wave Columns",
            Self::Sphere => "Sphere Ribbons",
            Self::Helix => "Helix Structure",
            Self::Bloom => "Petal Array",
            Self::Spiral => "Prism Array",
        }
    }
    pub const fn category(self) -> &'static str {
        match self {
            Self::Orbit => "RADIAL RIBBONS",
            Self::Wave => "TRAVELING WAVE",
            Self::Sphere => "SPHERICAL LATTICE",
            Self::Helix => "TWIN STRANDS",
            Self::Bloom => "KINETIC PETALS",
            Self::Spiral => "SPATIAL ARRAY",
        }
    }
    pub const fn next(self) -> Self {
        match self {
            Self::Orbit => Self::Wave,
            Self::Wave => Self::Sphere,
            Self::Sphere => Self::Helix,
            Self::Helix => Self::Bloom,
            Self::Bloom => Self::Spiral,
            Self::Spiral => Self::Orbit,
        }
    }
    pub const fn defaults(self) -> Parameters {
        match self {
            Self::Orbit => Parameters {
                count: 72,
                radius: 2.0,
                twist: 1.0,
                speed: 1.0,
                palette: 0,
            },
            Self::Wave => Parameters {
                count: 81,
                radius: 2.0,
                twist: 1.0,
                speed: 1.0,
                palette: 1,
            },
            Self::Sphere => Parameters {
                count: 48,
                radius: 2.1,
                twist: 0.6,
                speed: 1.0,
                palette: 2,
            },
            Self::Helix => Parameters {
                count: 48,
                radius: 2.0,
                twist: 1.0,
                speed: 1.0,
                palette: 1,
            },
            Self::Bloom => Parameters {
                count: 36,
                radius: 2.0,
                twist: 0.7,
                speed: 1.0,
                palette: 0,
            },
            Self::Spiral => Parameters {
                count: 64,
                radius: 1.8,
                twist: 0.7,
                speed: 1.0,
                palette: 2,
            },
        }
    }
    pub const fn parameter_name(self, parameter: Parameter) -> &'static str {
        match parameter {
            Parameter::Count => match self {
                Self::Wave => "Columns",
                Self::Sphere => "Rings",
                Self::Helix => "Rungs",
                Self::Bloom => "Petals",
                Self::Spiral => "Prisms",
                Self::Orbit => "Count",
            },
            Parameter::Radius => match self {
                Self::Wave => "Spacing",
                Self::Bloom => "Reach",
                Self::Spiral => "Spread",
                _ => "Radius",
            },
            Parameter::Twist => match self {
                Self::Wave => "Wave height",
                Self::Sphere => "Wobble",
                Self::Helix => "Turns",
                Self::Bloom => "Curl",
                Self::Spiral => "Turns",
                Self::Orbit => "Twist",
            },
            Parameter::Speed => parameter.name(),
        }
    }
}
