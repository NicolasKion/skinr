// Loads only advance on Blue's ticks. The renderer's frame loop pumps them
// once its canvas is on a page; until then whoever waits on a load pumps.
import { blue } from '@carbonenginejs/runtime/blue';

/** How often Blue is pumped while something waits, before the frame loop runs. */
const PUMP_INTERVAL_MS = 16;

let waiting = 0;
let timer: ReturnType<typeof setInterval> | null = null;

/** Keeps Blue ticking until `work` settles; overlapping waits share one timer. */
export async function pumped<T>(work: Promise<T>): Promise<T> {
  waiting += 1;
  timer ??= setInterval(() => blue.os.PumpOS(), PUMP_INTERVAL_MS);
  try {
    return await work;
  } finally {
    waiting -= 1;
    if (waiting === 0 && timer !== null) {
      clearInterval(timer);
      timer = null;
    }
  }
}
