// This file is part of sokoban-rs
// Copyright 2015 Sébastien Watteau
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::path::Path;
use sdl2::rect::Rect;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::render::{Renderer, Texture};
use sdl2_image::{LoadTexture};

use super::game::{Level, Position, Direction};

/// The Drawer struct is responsible for drawing the game onto the screen.
pub struct Drawer<'a> {
    /// The underlying SDL renderer
    renderer: Renderer<'a>,
    /// The texture containing all the tiles
    texture: Texture,
    /// The size of the screen in pixels
    screen_size: (u32, u32),
}

impl<'a> Drawer<'a> {
    /// Creates a new Drawer instance.
    pub fn new(renderer: Renderer<'a>) -> Drawer {
        let path = Path::new("assets/image/tileset.png");
        let texture = renderer.load_texture(path).unwrap();
        let screen_size = renderer.window().unwrap().drawable_size();
        Drawer {
            renderer: renderer,
            texture: texture,
            screen_size: screen_size,
        }
    }

    /// Draws a level onto the screen.
    pub fn draw(&mut self, level: &Level) {
        // Draw a full-size image onto an off-screen buffer
        let fullsize = self.get_rendering_size(&level);
        let _ = self.renderer.render_target()
            .expect("Render targets are not supported")
            .create_and_set(PixelFormatEnum::RGBA8888, fullsize);

        self.draw_fullsize(level);

        // Copy onto the screen with appropriate scaling
        let final_rect = self.get_centered_image_rect(self.get_scaled_rendering_size(&level));
        let texture = self.renderer.render_target()
            .unwrap()
            .reset()
            .unwrap_or_else(|err| panic!("Could not reset to the default render target: {}", err))
            .unwrap_or_else(|| panic!("Could not get the offscreen texture"));

        self.renderer.clear();
        self.renderer.copy(&texture, Some(Rect::new_unwrap(0, 0, fullsize.0, fullsize.1)), final_rect);
        self.renderer.present();
    }

    /// Draws a full-size image of the given level onto the current render target.
    fn draw_fullsize(&mut self, level: &Level) {
        let (cols, rows) = level.extents();
        self.renderer.set_draw_color(Color::RGB(0, 0, 0));
        self.renderer.clear();

        for j in (0..rows) {
            for i in (0..cols) {
                let pos = Position::new(j, i);
                let (x, y) = Tile::get_coordinates(&pos);

                // First draw the floor tiles
                if level.is_square(&pos) {
                    self.draw_tile(Tile::Square, x, y);
                } else {
                    self.draw_tile(Tile::Floor, x, y);
                }

                // Add the shadows
                let flags = get_shadow_flags(&level, &pos);
                for f in &[N_EDGE, S_EDGE, E_EDGE, W_EDGE, NE_CORNER, NW_CORNER, SE_CORNER, SW_CORNER] {
                    if flags.contains(*f) {
                        self.draw_tile(Tile::Shadow(*f), x, y);
                    }
                }

                // Draw the other items
                let z = y - Tile::offset();
                if level.is_wall(&pos) {
                    self.draw_tile(Tile::Wall, x, z);
                }
                if level.is_box(&pos) {
                    self.draw_tile(Tile::Rock, x, z);
                }
                if level.is_player(&pos) {
                    self.draw_tile(Tile::Player, x, z);
                }
            }
        }
    }

    /// Draws a tile at the given coordinates.
    fn draw_tile(&mut self, tile: Tile, x: i32, y: i32) {
        let (col, row) = tile.location().unwrap_or_else(|| {
            panic!("No image for this tile: {:?}", tile);
        });
        let tile = self.get_tile_rect(col, row);
        self.renderer.copy(&self.texture, tile, Some(Rect::new_unwrap(x, y, Tile::width(), Tile::height())));
    }

    /// Returns the Rect of the tile located at the given row and column in the texture.
    fn get_tile_rect(&self, col: u32, row: u32) -> Option<Rect> {
        let x = (col * Tile::width()) as i32;
        let y = (row * Tile::height()) as i32;
        Some(Rect::new_unwrap(x, y, Tile::width(), Tile::height()))
    }

    /// Returns the full size needed to draw the given level.
    fn get_rendering_size(&self, level: &Level) -> (u32, u32) {
        let (w, h) = level.extents();
        let width = w as u32 * Tile::width();
        let height = if h > 0 {
            Tile::height() + (h - 1) as u32 * Tile::effective_height()
        } else {
            0
        };

        (width, height)
    }

    /// Returns the size of the drawing scaled to fit onto the screen.
    fn get_scaled_rendering_size(&self, level: &Level) -> (u32, u32) {
        let render_size = self.get_rendering_size(&level);
        let width_ratio = (self.screen_size.0 as f64) / (render_size.0 as f64);
        let height_ratio = (self.screen_size.1 as f64) / (render_size.1 as f64);
        let ratio = f64::min(1.0, f64::min(width_ratio, height_ratio));

        let scale = |sz: u32| {
            (ratio * (sz as f64)).floor() as u32
        };

        (scale(render_size.0), scale(render_size.1))
    }

    /// Returns the Rect of an image of given dimensions so that it's centered on the screen.
    fn get_centered_image_rect(&self, img_size: (u32, u32)) -> Option<Rect> {
        let x = (self.screen_size.0 - img_size.0) as i32 / 2;
        let y = (self.screen_size.1 - img_size.1) as i32 / 2;
        Some(Rect::new_unwrap(x, y, img_size.0, img_size.1))
    }
}

/// Represents a kind of tile.
#[derive(Debug)]
enum Tile {
    /// Standard floor tile
    Floor,
    /// Wall tile
    Wall,
    /// Rock tile
    Rock,
    /// Target square tile
    Square,
    /// Player tile
    Player,
    /// Shadow tile
    Shadow(ShadowFlags),
}

impl Tile {
    /// Returns the location of the tile in the tileset texture.
    pub fn location(&self) -> Option<(u32, u32)> {
        match *self {
           Tile::Floor => Some((0, 0)),
           Tile::Wall => Some((0, 2)),
           Tile::Rock => Some((2, 0)),
           Tile::Square => Some((1, 0)),
           Tile::Player => Some((3, 0)),
           Tile::Shadow(N_EDGE) => Some((4, 0)),
           Tile::Shadow(S_EDGE) => Some((5, 0)),
           Tile::Shadow(E_EDGE) => Some((0, 1)),
           Tile::Shadow(W_EDGE) => Some((1, 1)),
           Tile::Shadow(NE_CORNER) => Some((2, 1)),
           Tile::Shadow(NW_CORNER) => Some((3, 1)),
           Tile::Shadow(SE_CORNER) => Some((4, 1)),
           Tile::Shadow(SW_CORNER) => Some((5, 1)),
           Tile::Shadow(ShadowFlags { .. }) => None,
        }
    }

    /// Returns the width of a tile.
    pub fn width() -> u32 {
        101
    }

    /// Returns the height of a tile.
    pub fn height() -> u32 {
        171
    }

    /// Returns the effective height of a tile (used for stacking)
    pub fn effective_height() -> u32 {
        83
    }

    /// Returns the offset need to draw items on the floor.
    pub fn offset() -> i32 {
        40
    }

    /// Returns the top-left corner coordinates of the tile corresponding
    /// to the given position.
    fn get_coordinates(pos: &Position) -> (i32, i32) {
        let x = Self::width() as i32 * pos.column();
        let y = Self::effective_height() as i32 * pos.row();
        (x, y)
    }
}

bitflags!(
    /// Represents the different kind of shadows that can be cast
    /// onto a floor tile.
    flags ShadowFlags: i32 {
        /// North edge
        const N_EDGE = 0x1,
        /// South edge
        const S_EDGE = 0x2,
        /// East edge
        const E_EDGE = 0x4,
        /// West edge
        const W_EDGE = 0x8,
        /// North East corner
        const NE_CORNER = 0x10,
        /// North West corner
        const NW_CORNER = 0x20,
        /// South East corner
        const SE_CORNER = 0x40,
        /// South West corner
        const SW_CORNER = 0x80,
    }
);

/// Returns the shadow flags for a particular position in the given level.
fn get_shadow_flags(level: &Level, pos: &Position) -> ShadowFlags {
    let north = pos.neighboor(Direction::Up);
    let south = pos.neighboor(Direction::Down);
    let west = pos.neighboor(Direction::Left);
    let east = pos.neighboor(Direction::Right);

    let mut flags = ShadowFlags::empty();
    if level.is_wall(&north) {
        flags = flags | N_EDGE;
    }
    if level.is_wall(&south) {
        flags = flags | S_EDGE;
    }
    if level.is_wall(&west) {
        flags = flags | W_EDGE;
    }
    if level.is_wall(&east) {
        flags = flags | E_EDGE;
    }
    if level.is_wall(&north.neighboor(Direction::Right)) && !flags.intersects(N_EDGE | E_EDGE) {
        flags = flags | NE_CORNER;
    }
    if level.is_wall(&north.neighboor(Direction::Left)) && !flags.intersects(N_EDGE | W_EDGE) {
        flags = flags | NW_CORNER;
    }
    if level.is_wall(&south.neighboor(Direction::Right)) && !flags.intersects(S_EDGE | E_EDGE) {
        flags = flags | SE_CORNER;
    }
    if level.is_wall(&south.neighboor(Direction::Left)) && !flags.intersects(S_EDGE | W_EDGE) {
        flags = flags | SW_CORNER;
    }
    flags
}
