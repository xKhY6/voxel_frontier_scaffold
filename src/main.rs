mod ai;
mod audio;
mod camera;
mod crafting;
mod game;
mod generation;
mod input;
mod lighting;
mod physics;
mod render;
mod save;
mod voxel;
mod weapons;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    game::run()
}
