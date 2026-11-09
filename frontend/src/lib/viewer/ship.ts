// Builds an EveShip2 from the SOF's model values. Since alpha.12 the SOF no
// longer loads a hull's model curves inline: it leaves a CjsExternalRef (the
// res path and the interface it must implement) for the caller to load and
// splice in. Built as is, the ship then calls Update__Ticks on that
// placeholder and every frame it is in fails (the capsules have one).
import { blue } from '@carbonenginejs/runtime/blue';
import { CjsSchema } from '@carbonenginejs/runtime/schema';

/* eslint-disable @typescript-eslint/no-explicit-any */

/** The ship's slots the SOF can leave as an external reference. */
const CURVE_SLOTS = ['modelRotationCurve', 'modelTranslationCurve'] as const;

/** The SOF's placeholder type for a graph it did not load. */
const EXTERNAL_REF = 'CjsExternalRef';

/**
 * Builds the ship without its external curves, then loads each and puts it
 * in its slot. The ship is drawn meanwhile, just not animated by them; a
 * curve that fails to load leaves its slot empty.
 */
export function buildShip(values: any) {
  // oxlint-disable-next-line no-underscore-dangle -- the SOF's values name their class in `_type`
  const refs = CURVE_SLOTS.filter((slot) => values[slot]?._type === EXTERNAL_REF).map(
    (slot) => [slot, values[slot].resPath as string] as const,
  );
  const ship = CjsSchema.from(
    'EveShip2',
    refs.length ? { ...values, ...Object.fromEntries(refs.map(([slot]) => [slot, null])) } : values,
  );
  for (const [slot, path] of refs) {
    blue.resMan
      .LoadObject(path)
      .then((curve: any) => {
        if (curve) ship[slot] = curve;
      })
      .catch((error: unknown) => console.warn(`Could not load ${path}`, error));
  }
  return ship;
}
