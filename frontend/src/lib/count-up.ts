// A number that counts up from zero the first time it scrolls into view.

/** How long the count takes, in ms. */
const DURATION = 1400;

/** Ease-out-expo, matching --ease-out-expo in layout.css. */
const ease = (t: number) => (t >= 1 ? 1 : 1 - 2 ** (-10 * t));

export function countUp(
  node: HTMLElement,
  params: { value: number; format: (n: number) => string },
) {
  let { value, format } = params;
  let frame = 0;
  node.textContent = format(value);

  if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
    return { update: (next: typeof params) => (node.textContent = next.format(next.value)) };
  }

  node.textContent = format(0);
  const observer = new IntersectionObserver(
    ([entry]) => {
      if (!entry?.isIntersecting) return;
      observer.disconnect();
      const start = performance.now();
      const tick = (now: number) => {
        const progress = ease((now - start) / DURATION);
        node.textContent = format(value * progress);
        if (progress < 1) frame = requestAnimationFrame(tick);
      };
      frame = requestAnimationFrame(tick);
    },
    { threshold: 0.6 },
  );
  observer.observe(node);

  return {
    update(next: typeof params) {
      ({ value, format } = next);
      node.textContent = format(value);
    },
    destroy() {
      observer.disconnect();
      cancelAnimationFrame(frame);
    },
  };
}
