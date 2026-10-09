// The fitting window look: the rendered ship redrawn as glowing blue edges.
//
// The runtime has no wireframe mode on WebGPU (no polygon fill mode in the
// API, and its barycentric route is not ported), so this works on the
// finished frame instead: after the runtime submits, the canvas texture is
// copied aside and a full-screen pass writes edges back into it. Edges come
// from the frame's alpha (the silhouette) and luminance (panel lines and
// greebles), which reads much like EVE's own hologram.

/* eslint-disable @typescript-eslint/no-explicit-any */

/** WebGPU's usage flags; the DOM typings here do not declare them. */
export const gpuUsage = globalThis as unknown as {
  GPUBufferUsage: Record<'UNIFORM' | 'COPY_DST' | 'MAP_READ', number>;
  GPUMapMode: Record<'READ', number>;
  GPUTextureUsage: Record<
    'TEXTURE_BINDING' | 'COPY_DST' | 'COPY_SRC' | 'RENDER_ATTACHMENT',
    number
  >;
};

const SHADER = /* wgsl */ `
struct Params {
  amount: f32,
  time: f32,
  _pad: vec2f,
};

@group(0) @binding(0) var frame: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: Params;

@vertex
fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
  // One triangle that covers the screen.
  let uv = vec2f(f32((index << 1u) & 2u), f32(index & 2u));
  return vec4f(uv * 2.0 - 1.0, 0.0, 1.0);
}

fn texel(p: vec2i) -> vec4f {
  let size = vec2i(textureDimensions(frame));
  return textureLoad(frame, clamp(p, vec2i(0), size - 1), 0);
}

fn luma(c: vec4f) -> f32 {
  return dot(c.rgb, vec3f(0.299, 0.587, 0.114));
}

@fragment
fn fs(@builtin(position) position: vec4f) -> @location(0) vec4f {
  let p = vec2i(position.xy);
  let original = texel(p);
  if (params.amount <= 0.0) {
    return original;
  }

  // Sobel over luminance and alpha.
  var gl = vec2f(0.0);
  var ga = vec2f(0.0);
  let kx = array<f32, 9>(-1.0, 0.0, 1.0, -2.0, 0.0, 2.0, -1.0, 0.0, 1.0);
  let ky = array<f32, 9>(-1.0, -2.0, -1.0, 0.0, 0.0, 0.0, 1.0, 2.0, 1.0);
  for (var i = 0; i < 9; i++) {
    let c = texel(p + vec2i(i % 3 - 1, i / 3 - 1));
    let l = luma(c);
    gl += vec2f(kx[i], ky[i]) * l;
    ga += vec2f(kx[i], ky[i]) * c.a;
  }
  let detail = smoothstep(0.06, 0.32, length(gl));
  let outline = smoothstep(0.15, 0.9, length(ga));
  let edge = max(detail * 0.75, outline);

  // A faint body so the hull reads as glass, with slow scanlines.
  let scan = 0.85 + 0.15 * sin(position.y * 1.6 + params.time * 2.0);
  let body = original.a * 0.10 * scan;

  let line = vec3f(0.42, 0.78, 1.0);
  let glow = vec3f(0.85, 0.95, 1.0);
  let color = mix(line, glow, detail * outline);
  let alpha = clamp(edge + body, 0.0, 1.0);
  // Premultiplied, like the canvas.
  let hologram = vec4f(color * edge + line * body, alpha);
  return mix(original, hologram, params.amount);
}
`;

export class Hologram {
  private pipeline: any;
  private uniforms: any;
  private copy: any = null;
  private bindGroup: any = null;
  private context: any;

  constructor(
    private device: any,
    canvas: HTMLCanvasElement,
    private format: string,
  ) {
    // The context the runtime configured; getContext returns the same one.
    this.context = canvas.getContext('webgpu');
    const module = device.createShaderModule({ code: SHADER, label: 'hologram' });
    this.pipeline = device.createRenderPipeline({
      label: 'hologram',
      layout: 'auto',
      vertex: { module, entryPoint: 'vs' },
      fragment: { module, entryPoint: 'fs', targets: [{ format }] },
      primitive: { topology: 'triangle-list' },
    });
    this.uniforms = device.createBuffer({
      size: 16,
      usage: gpuUsage.GPUBufferUsage.UNIFORM | gpuUsage.GPUBufferUsage.COPY_DST,
    });
  }

  /** Redraws the frame the runtime just submitted, `amount` of the way to the hologram. */
  apply(amount: number, time: number) {
    const target = this.context.getCurrentTexture();
    const { width, height } = target;
    if (!this.copy || this.copy.width !== width || this.copy.height !== height) {
      this.copy?.destroy();
      this.copy = this.device.createTexture({
        label: 'hologram source',
        size: [width, height],
        format: this.format,
        usage: gpuUsage.GPUTextureUsage.TEXTURE_BINDING | gpuUsage.GPUTextureUsage.COPY_DST,
      });
      this.bindGroup = this.device.createBindGroup({
        layout: this.pipeline.getBindGroupLayout(0),
        entries: [
          { binding: 0, resource: this.copy.createView() },
          { binding: 1, resource: { buffer: this.uniforms } },
        ],
      });
    }
    this.device.queue.writeBuffer(this.uniforms, 0, new Float32Array([amount, time, 0, 0]));

    const encoder = this.device.createCommandEncoder({ label: 'hologram' });
    encoder.copyTextureToTexture({ texture: target }, { texture: this.copy }, [width, height]);
    const pass = encoder.beginRenderPass({
      colorAttachments: [
        { view: target.createView(), loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 0] },
      ],
    });
    pass.setPipeline(this.pipeline);
    pass.setBindGroup(0, this.bindGroup);
    pass.draw(3);
    pass.end();
    this.device.queue.submit([encoder.finish()]);
  }

  /**
   * Copies the frame on the canvas into a buffer and resolves with its
   * pixels. The copy is encoded now, so later renders do not affect it;
   * drawImage cannot read a WebGPU canvas back here.
   */
  readback(): Promise<ImageData> {
    const target = this.context.getCurrentTexture();
    const { width, height } = target;
    // Rows in a texture-to-buffer copy are padded to 256 bytes.
    const bytesPerRow = Math.ceil((width * 4) / 256) * 256;
    const buffer = this.device.createBuffer({
      size: bytesPerRow * height,
      usage: gpuUsage.GPUBufferUsage.COPY_DST | gpuUsage.GPUBufferUsage.MAP_READ,
    });
    const encoder = this.device.createCommandEncoder({ label: 'readback' });
    encoder.copyTextureToBuffer({ texture: target }, { buffer, bytesPerRow }, [width, height]);
    this.device.queue.submit([encoder.finish()]);

    const bgra = this.format.startsWith('bgra');
    return buffer.mapAsync(gpuUsage.GPUMapMode.READ).then(() => {
      const source = new Uint8Array(buffer.getMappedRange());
      const pixels = new Uint8ClampedArray(width * height * 4);
      for (let y = 0; y < height; y++) {
        for (let x = 0; x < width; x++) {
          const from = y * bytesPerRow + x * 4;
          const to = (y * width + x) * 4;
          const alpha = source[from + 3];
          // The canvas holds premultiplied color; ImageData wants it straight.
          const unpremultiply = alpha ? 255 / alpha : 0;
          pixels[to] = source[from + (bgra ? 2 : 0)] * unpremultiply;
          pixels[to + 1] = source[from + 1] * unpremultiply;
          pixels[to + 2] = source[from + (bgra ? 0 : 2)] * unpremultiply;
          pixels[to + 3] = alpha;
        }
      }
      buffer.unmap();
      buffer.destroy();
      return new ImageData(pixels, width, height);
    });
  }

  /** Copies a rectangle of the frame on the canvas into another canvas on the same device. */
  copyTo(context: any, area: { x: number; y: number; width: number; height: number }) {
    const source = this.context.getCurrentTexture();
    const target = context.getCurrentTexture();
    const encoder = this.device.createCommandEncoder({ label: 'blueprint view' });
    encoder.copyTextureToTexture(
      { texture: source, origin: [area.x, area.y, 0] },
      { texture: target },
      [Math.min(area.width, target.width), Math.min(area.height, target.height), 1],
    );
    this.device.queue.submit([encoder.finish()]);
  }
}
