use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub player: u8,
    pub timestamp: i64,
}
