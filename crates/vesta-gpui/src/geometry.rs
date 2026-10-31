/// A point in logical window pixels, with the origin at the top-left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A two-dimensional logical size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    /// Finite nonnegative extents. Zero area is valid but empty.
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width >= 0.0 && self.height >= 0.0
    }
}

/// An axis-aligned rectangle in logical window pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    #[must_use]
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    #[must_use]
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    /// Finite origins, nonnegative finite extents and representable finite edges.
    /// Positive extents must advance their corresponding edge at f32 precision.
    /// Valid zero-area rectangles remain empty and cannot be hit.
    #[must_use]
    pub fn is_valid(self) -> bool {
        Size::new(self.width, self.height).is_valid()
            && [self.x, self.y, self.right(), self.bottom()]
                .into_iter()
                .all(f32::is_finite)
            && (self.width == 0.0 || self.right() > self.x)
            && (self.height == 0.0 || self.bottom() > self.y)
    }

    #[must_use]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    #[must_use]
    pub fn contains(self, point: Point) -> bool {
        self.is_valid()
            && !self.is_empty()
            && point.x >= self.x
            && point.x <= self.right()
            && point.y >= self.y
            && point.y <= self.bottom()
    }

    /// True when `other` lies entirely inside this rect (inclusive edges).
    #[must_use]
    pub fn contains_rect(self, other: Self) -> bool {
        self.is_valid()
            && other.is_valid()
            && !self.is_empty()
            && !other.is_empty()
            && other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        if !self.is_valid() || !other.is_valid() {
            return None;
        }
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        (right > left && bottom > top).then(|| Self::new(left, top, right - left, bottom - top))
    }

    #[must_use]
    pub fn inset(self, amount: f32) -> Self {
        let amount = amount.max(0.0);
        Self::new(
            self.x + amount,
            self.y + amount,
            (self.width - amount * 2.0).max(0.0),
            (self.height - amount * 2.0).max(0.0),
        )
    }
}

/// The logical extent requested from the host window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub scale_factor: f32,
}

impl Viewport {
    #[must_use]
    pub const fn new(width: f32, height: f32, scale_factor: f32) -> Self {
        Self {
            width,
            height,
            scale_factor,
        }
    }

    #[must_use]
    pub const fn logical_rect(self) -> Rect {
        Rect::new(0.0, 0.0, self.width, self.height)
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        self.width.is_finite()
            && self.height.is_finite()
            && self.scale_factor.is_finite()
            && self.width > 0.0
            && self.height > 0.0
            && self.scale_factor > 0.0
    }
}
