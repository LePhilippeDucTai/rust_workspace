use bevy::prelude::*;

pub const N: usize = 9;
pub const CELL_SIZE: f32 = 58.0;
pub const WIN_W: u32 = 900;
pub const WIN_H: u32 = 820;

pub const COL_BG: Color = Color::srgb(0.96, 0.91, 0.80);
pub const COL_GRID: Color = Color::srgb(0.39, 0.28, 0.20);
pub const COL_BLOCK_BG: Color = Color::srgb(0.55, 0.41, 0.30);
pub const COL_CELL: Color = Color::srgb(0.995, 0.965, 0.88);
pub const COL_CELL_GIVEN: Color = Color::srgb(0.93, 0.88, 0.74);
pub const COL_SELECTED: Color = Color::srgb(0.74, 0.84, 0.62);
pub const COL_HIGHLIGHT: Color = Color::srgb(0.87, 0.92, 0.76);
pub const COL_SAME_NUM: Color = Color::srgb(0.80, 0.88, 0.66);
pub const COL_TEXT_GIVEN: Color = Color::srgb(0.17, 0.30, 0.21);
pub const COL_TEXT_USER: Color = Color::srgb(0.18, 0.36, 0.55);
pub const COL_TEXT_CONFLICT: Color = Color::srgb(0.74, 0.27, 0.19);
pub const COL_TERRACOTTA: Color = Color::srgb(0.80, 0.45, 0.30);
pub const COL_LEAF: Color = Color::srgb(0.45, 0.62, 0.40);
pub const COL_LEAF_DARK: Color = Color::srgb(0.32, 0.48, 0.30);
pub const COL_TITLE: Color = Color::srgb(0.45, 0.27, 0.17);
pub const COL_SUBTITLE: Color = Color::srgb(0.55, 0.40, 0.28);
pub const COL_WHITE: Color = Color::srgb(1.0, 0.98, 0.94);
