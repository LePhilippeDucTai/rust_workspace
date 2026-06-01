//! Rendu de la grille via une unique texture mise à jour chaque frame.
//!
//! Dessiner ~33 000 cellules avec un sprite par cellule serait coûteux. On
//! écrit plutôt directement les octets d'une `Image` de la taille de la grille,
//! affichée par un seul sprite agrandi en échantillonnage « au plus proche »
//! pour garder des pixels nets.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::components::GridSprite;
use crate::config::{CELL_PX, GRID_CX, GRID_CY, GRID_H, GRID_W, PALETTE};
use crate::resources::{Grid, GridImage};

pub struct VisualizationPlugin;

impl Plugin for VisualizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_grid)
            .add_systems(Update, update_texture);
    }
}

/// Crée la texture de la grille et le sprite qui l'affiche.
fn setup_grid(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: GRID_W as u32,
            height: GRID_H as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &PALETTE[0],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    // Pixels nets une fois agrandis (pas d'interpolation linéaire).
    image.sampler = ImageSampler::nearest();

    let handle = images.add(image);

    let mut sprite = Sprite::from_image(handle.clone());
    sprite.custom_size = Some(Vec2::new(
        GRID_W as f32 * CELL_PX,
        GRID_H as f32 * CELL_PX,
    ));

    commands.spawn((sprite, Transform::from_xyz(GRID_CX, GRID_CY, 0.0), GridSprite));
    commands.insert_resource(GridImage(handle));
}

/// Recopie l'état de la grille dans la texture (4 octets sRGB par cellule).
fn update_texture(
    grid: Res<Grid>,
    grid_image: Res<GridImage>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(image) = images.get_mut(&grid_image.0) else {
        return;
    };
    let Some(data) = image.data.as_mut() else {
        return;
    };

    for (k, &count) in grid.pile.cells.iter().enumerate() {
        let color = PALETTE[(count as usize).min(3)];
        let o = k * 4;
        data[o] = color[0];
        data[o + 1] = color[1];
        data[o + 2] = color[2];
        data[o + 3] = color[3];
    }
}
