// Some of EVE's block-compressed textures are not a whole number of blocks
// wide or tall (a 125 px wide BC7 map on the Komodo). WebGPU refuses such a
// texture, and every frame that binds it is dropped whole, so one odd texture
// blanks the ship. Carbon's own backends accept them. Until the runtime pads
// them itself, the device is wrapped so they are allocated with their size
// rounded up to whole blocks, and uploads copy the whole blocks their data
// already holds. The few padding texels sit outside the texture's UV range.

/** Block-compressed formats store texels in 4x4 blocks. */
const BLOCK = 4;
const isBlockCompressed = (format: string) => format.startsWith('bc');
const roundUp = (size: number) => Math.ceil(size / BLOCK) * BLOCK;

/** Reads a GPUExtent3D, which may be a sequence or a dictionary. */
function extent(size: GPUExtent3D | Iterable<number>) {
  if (Symbol.iterator in Object(size)) {
    const [width, height = 1, depthOrArrayLayers = 1] = [...(size as Iterable<number>)];
    return { width, height, depthOrArrayLayers };
  }
  const { width, height = 1, depthOrArrayLayers = 1 } = size as GPUExtent3DDict;
  return { width, height, depthOrArrayLayers };
}

/** Wraps `device` so block-compressed textures of any size can be created and filled. */
export function alignBlockTextures(device: GPUDevice) {
  const createTexture = device.createTexture.bind(device);
  device.createTexture = (descriptor: GPUTextureDescriptor) => {
    if (!isBlockCompressed(descriptor.format)) return createTexture(descriptor);
    const size = extent(descriptor.size);
    return createTexture({
      ...descriptor,
      size: { ...size, width: roundUp(size.width), height: roundUp(size.height) },
    });
  };

  const { queue } = device;
  const writeTexture = queue.writeTexture.bind(queue);
  queue.writeTexture = (
    destination: GPUTexelCopyTextureInfo,
    data: AllowSharedBufferSource,
    layout: GPUTexelCopyBufferLayout,
    size: GPUExtent3D | Iterable<number>,
  ) => {
    if (!isBlockCompressed(destination.texture.format)) {
      return writeTexture(destination, data, layout, size as GPUExtent3D);
    }
    // The data already holds whole blocks (its rows are counted in blocks).
    const { width, height, depthOrArrayLayers } = extent(size);
    return writeTexture(destination, data, layout, {
      width: roundUp(width),
      height: roundUp(height),
      depthOrArrayLayers,
    });
  };
}
