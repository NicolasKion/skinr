import { describe, expect, it } from 'vitest';
import { crc32, encodePng } from './png';

describe('png', () => {
  it('computes the standard CRC-32', () => {
    expect(crc32(new TextEncoder().encode('IEND'))).toBe(0xae426082);
    expect(crc32(new TextEncoder().encode('123456789'))).toBe(0xcbf43926);
  });

  it('encodes a cropped region as a valid PNG', async () => {
    // A 3x2 image; crop the 2x1 region at (1, 1).
    const pixels = new Uint8ClampedArray(3 * 2 * 4);
    pixels.set([255, 0, 0, 255], (1 * 3 + 1) * 4);
    pixels.set([0, 255, 0, 128], (1 * 3 + 2) * 4);
    const png = new Uint8Array(await (await encodePng(pixels, 3, 1, 1, 2, 1)).arrayBuffer());

    expect(Array.from(png.subarray(0, 8))).toEqual([137, 80, 78, 71, 13, 10, 26, 10]);
    const view = new DataView(png.buffer);
    expect(new TextDecoder().decode(png.subarray(12, 16))).toBe('IHDR');
    expect([view.getUint32(16), view.getUint32(20)]).toEqual([2, 1]);
    expect(new TextDecoder().decode(png.subarray(png.length - 8, png.length - 4))).toBe('IEND');

    // The IDAT payload inflates back to one filtered row of the two pixels.
    const length = view.getUint32(33);
    expect(new TextDecoder().decode(png.subarray(37, 41))).toBe('IDAT');
    const inflated = new Uint8Array(
      await new Response(
        new Blob([png.subarray(41, 41 + length)])
          .stream()
          .pipeThrough(new DecompressionStream('deflate')),
      ).arrayBuffer(),
    );
    expect(Array.from(inflated)).toEqual([0, 255, 0, 0, 255, 0, 255, 0, 128]);
  });
});
