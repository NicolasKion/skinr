// The ship page reads as chapters. Each one frames the main ship its own
// way: where the stage sits (beside the chapter's text on wide screens) and
// the camera angle. The chapter crossing the middle of the screen is the
// active one.
import { CENTER, type StagePlacement } from '#lib/stage.svelte.js';
import { BLUEPRINT_POSE, HERO_POSE, SIDE_POSE, type CameraPose } from '#lib/viewer/poses.js';

export type Chapter = 'hero' | 'profile' | 'blueprint' | 'liveries' | 'more';

/** Beside a chapter's text: right of centre and a little smaller. */
export const RIGHT: StagePlacement = { x: 0.2, scale: 0.8 };

/**
 * How each chapter frames the main ship. From the blueprint on it glides
 * into the drawing's side elevation and stays there; the livery chapter
 * shows its own view of the ship (a second render), so both can be on
 * screen at once without either moving.
 */
export const CHAPTERS: Record<
  Chapter,
  { placement: StagePlacement; pose: CameraPose; dim: number }
> = {
  hero: { placement: CENTER, pose: HERO_POSE, dim: 0 },
  profile: { placement: RIGHT, pose: SIDE_POSE, dim: 0 },
  blueprint: { placement: RIGHT, pose: BLUEPRINT_POSE, dim: 0 },
  liveries: { placement: RIGHT, pose: BLUEPRINT_POSE, dim: 0 },
  more: { placement: RIGHT, pose: BLUEPRINT_POSE, dim: 0 },
};

/** From the blueprint down, the main ship sits in the drawing. */
export function isDocked(chapter: Chapter): boolean {
  return chapter === 'blueprint' || chapter === 'liveries' || chapter === 'more';
}

/** Which site bar link each chapter lights up; the hero lights none. */
export const SECTION_OF: Record<Chapter, string | null> = {
  hero: null,
  profile: 'profile',
  blueprint: 'blueprint',
  liveries: 'liveries',
  more: 'related',
};

/** Calls `enter` whenever the element comes to cross the middle of the screen. */
export function whenCrossing(node: HTMLElement, enter: () => void) {
  const observer = new IntersectionObserver(
    ([entry]) => {
      if (entry?.isIntersecting) enter();
    },
    { rootMargin: '-50% 0px -50% 0px' },
  );
  observer.observe(node);
  return { destroy: () => observer.disconnect() };
}
