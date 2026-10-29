mod camera;
mod game;
mod generation;
mod input;
mod physics;
mod render;
mod save;
mod voxel;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    game::run()
}
