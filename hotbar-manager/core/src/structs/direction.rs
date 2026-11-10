/// Which way the inventory rows rotate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Row 3 moves into the hotbar; the hotbar moves to row 1.
    Down,
    /// Row 1 moves into the hotbar; the hotbar moves to row 3.
    Up,
}
