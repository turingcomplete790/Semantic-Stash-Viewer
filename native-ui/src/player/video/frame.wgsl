// mpv's frame as a quad filling the widget (iced sets the viewport to the widget's bounds).

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    let xy = corners[index];
    var out: VertexOut;
    out.position = vec4<f32>(xy, 0.0, 1.0);
    // mpv renders for GL's bottom-left origin, so row 0 of the frame is the bottom of the
    // picture: sample upward from the bottom of the widget.
    out.uv = vec2<f32>((xy.x + 1.0) * 0.5, (xy.y + 1.0) * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(textureSample(frame, frame_sampler, in.uv).rgb, 1.0);
}
