// Translates one dx11 effect container to WebGPU per message, off the main
// thread (effects.ts). Only the translator is loaded here, not the engine.
import { CjsWebgpuFormat } from '@carbonenginejs/runtime/resource/formats/webgpu';

import { reduce } from './effects';

export interface TranslateRequest {
  path: string;
  bytes: Uint8Array;
}

export type TranslateAnswer = { bytes: Uint8Array; indices: number[] } | { error: string };

self.addEventListener('message', ({ data }: MessageEvent<TranslateRequest>) => {
  const { path, bytes } = data;
  try {
    // The default permutation, as Tr2EffectRes asks for it when an effect loads.
    const built = reduce(CjsWebgpuFormat.buildEffect(bytes, { source: path, permutation: [] }));
    // A worker answers its page; there is no origin to name.
    // oxlint-disable-next-line unicorn/require-post-message-target-origin
    self.postMessage(built satisfies TranslateAnswer, {
      transfer: [built.bytes.buffer],
    });
  } catch (error) {
    // oxlint-disable-next-line unicorn/require-post-message-target-origin
    self.postMessage({ error: String(error) } satisfies TranslateAnswer);
  }
});
