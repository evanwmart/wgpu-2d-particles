// main.rs
use wgpu_2d_particles::run;


fn main() {
    pollster::block_on(run());
}