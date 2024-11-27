// shader.wgsl

struct Particle {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    var quad_offsets = array<vec2<f32>, 6>(
    vec2<f32>(-0.005, -0.01), // A   Bottom-left
    vec2<f32>(0.005, -0.01),  // B   Bottom-right
    vec2<f32>(-0.005, 0.01),  // C   Top-left

    vec2<f32>(-0.005, 0.01),  // D   Top-left
    vec2<f32>(0.005, -0.01),  // E   Bottom-right
    vec2<f32>(0.005, 0.01)    // F   Top-right
    );

    // Map the vertex index to the quad offsets
    var output: VertexOutput;
    output.clip_position = vec4<f32>(
        position + quad_offsets[vertex_index], 0.0, 1.0
    );
    output.color = vec4<f32>(0.7, 0.8, 0.9, 0.7);
    return output;
}

@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return color; // Use the color passed from the vertex shader
}
