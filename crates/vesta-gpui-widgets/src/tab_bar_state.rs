//! Controlled stable tab order, pinning and selection.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabBarState<T> {
    active: Option<T>,
    order: Vec<T>,
    pinned: Vec<T>,
    overflow_offset: usize,
}

impl<T: Copy + PartialEq> TabBarState<T> {
    #[must_use]
    pub fn new(order: impl IntoIterator<Item = T>, active: Option<T>) -> Self {
        let order = order.into_iter().collect::<Vec<_>>();
        let active = active.filter(|candidate| order.contains(candidate));
        Self {
            active,
            order,
            pinned: Vec::new(),
            overflow_offset: 0,
        }
    }

    #[must_use]
    pub fn active(&self) -> Option<T> {
        self.active
    }

    #[must_use]
    pub fn order(&self) -> &[T] {
        &self.order
    }

    pub fn activate(&mut self, tab: T) -> bool {
        if !self.order.contains(&tab) || self.active == Some(tab) {
            return false;
        }
        self.active = Some(tab);
        true
    }

    pub fn move_to(&mut self, tab: T, slot: usize) -> bool {
        let Some(source) = self.order.iter().position(|candidate| *candidate == tab) else {
            return false;
        };
        let value = self.order.remove(source);
        self.order.insert(slot.min(self.order.len()), value);
        true
    }

    #[must_use]
    pub fn is_pinned(&self, tab: T) -> bool {
        self.pinned.contains(&tab)
    }

    pub fn set_pinned(&mut self, tab: T, pinned: bool) -> bool {
        if !self.order.contains(&tab) || self.is_pinned(tab) == pinned {
            return false;
        }
        if pinned {
            self.pinned.push(tab);
        } else if let Some(index) = self.pinned.iter().position(|candidate| *candidate == tab) {
            self.pinned.remove(index);
        }
        true
    }

    pub fn set_overflow_offset(&mut self, offset: usize) {
        self.overflow_offset = offset.min(self.order.len().saturating_sub(1));
    }

    #[must_use]
    pub fn visible_tabs(&self, capacity: usize) -> Vec<T> {
        if capacity == 0 {
            return Vec::new();
        }
        let mut visible = self
            .order
            .iter()
            .copied()
            .filter(|tab| self.is_pinned(*tab))
            .take(capacity)
            .collect::<Vec<_>>();
        let remaining = capacity.saturating_sub(visible.len());
        visible.extend(
            self.order
                .iter()
                .copied()
                .filter(|tab| !self.is_pinned(*tab))
                .skip(self.overflow_offset)
                .take(remaining),
        );
        visible
    }

    pub fn close(&mut self, tab: T) -> bool {
        let Some(index) = self.order.iter().position(|candidate| *candidate == tab) else {
            return false;
        };
        self.order.remove(index);
        self.pinned.retain(|candidate| *candidate != tab);
        if self.active == Some(tab) {
            self.active = self.order.get(index).or_else(|| self.order.last()).copied();
        }
        true
    }
}
