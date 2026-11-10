import { describe, expect, it } from 'vitest';
import { SPHERE_SPAN } from '#lib/viewer/poses.js';
import { narrowFit, narrowPlacement, textCover } from './narrow';

const phone = { width: 390, height: 760 };

describe('narrowFit', () => {
  it('shrinks the ship until its sphere fits the width', () => {
    const fit = narrowFit(phone);
    expect(fit).toBeLessThan(1);
    expect(phone.height * SPHERE_SPAN * fit).toBeCloseTo(phone.width * 0.9);
  });

  it('leaves it alone where it fits already', () => {
    expect(narrowFit({ width: 900, height: 600 })).toBe(1);
  });
});

describe('narrowPlacement', () => {
  it('lifts the ship above the text and pulls the camera back to fit', () => {
    const placement = narrowPlacement(phone);
    expect(placement.x).toBe(0);
    expect(placement.y).toBeLessThan(0);
    expect(placement.framing).toBeCloseTo(1 / narrowFit(phone));
  });
});

describe('textCover', () => {
  it('is nothing while the text sits below the ship', () => {
    expect(textCover(phone, [{ top: 700, bottom: 1200 }])).toBe(0);
    expect(textCover(phone, [null])).toBe(0);
  });

  it('grows as the text scrolls up over the ship, to full', () => {
    const rising = textCover(phone, [{ top: 300, bottom: 800 }]);
    expect(rising).toBeGreaterThan(0);
    expect(rising).toBeLessThan(1);
    expect(textCover(phone, [{ top: 50, bottom: 550 }])).toBe(1);
  });

  it('lets go again once the text has passed above the ship', () => {
    expect(textCover(phone, [{ top: -900, bottom: -400 }])).toBe(0);
  });

  it('takes the most covering of several texts', () => {
    const one = textCover(phone, [{ top: 300, bottom: 800 }]);
    expect(
      textCover(phone, [
        { top: 700, bottom: 1200 },
        { top: 300, bottom: 800 },
      ]),
    ).toBe(one);
  });
});
