// Pointer input on a canvas: one drag handler for the stage's canvas and the
// section views' canvases.

export interface DragHandlers {
  /** A pointer went down; return false to ignore this drag. */
  start?(): boolean | void;
  /** It moved by `dx`, `dy` CSS pixels since the last report. */
  move(dx: number, dy: number): void;
  /** It was lifted, or the browser took the gesture over (a scroll). */
  end?(): void;
}

/** Reports drags over `canvas` to `handlers`, pointer deltas only. */
export function onDrag(canvas: HTMLCanvasElement, handlers: DragHandlers) {
  let last: { x: number; y: number } | null = null;
  const end = () => {
    if (!last) return;
    last = null;
    handlers.end?.();
  };
  canvas.addEventListener('pointerdown', (event) => {
    if (handlers.start?.() === false) return;
    last = { x: event.clientX, y: event.clientY };
    canvas.setPointerCapture(event.pointerId);
  });
  canvas.addEventListener('pointerup', end);
  canvas.addEventListener('pointercancel', end);
  canvas.addEventListener('pointermove', (event) => {
    if (!last) return;
    handlers.move(event.clientX - last.x, event.clientY - last.y);
    last = { x: event.clientX, y: event.clientY };
  });
}
