//! Client-side kurzor: vykresluje se lokálně (0 ms latence),
//! poloha se synchronizuje lehkými pakety.

use serde::{Deserialize, Serialize};

/// Stav vzdáleného kurzoru.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorState {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
}

impl CursorState {
    pub fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y,
            visible: true,
        }
    }

    /// Vrátí true, pokud se stav změnil a má se poslat paket.
    pub fn update(&mut self, x: i32, y: i32, visible: bool) -> bool {
        let changed = self.x != x || self.y != y || self.visible != visible;
        self.x = x;
        self.y = y;
        self.visible = visible;
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_reports_changes_only() {
        let mut c = CursorState::new(0, 0);
        assert!(!c.update(0, 0, true));
        assert!(c.update(10, 0, true));
        assert!(c.update(10, 0, false));
    }
}
