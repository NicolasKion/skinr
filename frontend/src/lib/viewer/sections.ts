// Page sections' own views of the ship (the liveries, the SKINRs): further
// canvases showing it from their own pose with the normal render. The
// renderer draws the scene again for each visible one and copies the result
// over on the GPU, so the main camera can stay on the blueprint while they
// are on screen.
import { IDLE_SPIN, orbited } from './camera';
import { gpuUsage } from './hologram';
import { onDrag } from './input';
import type { CameraPose } from './poses';

/* eslint-disable @typescript-eslint/no-explicit-any */

/**
 * A section's view: its camera pose, whether it is on screen, and where the
 * ship sits in it. The canvas spans the whole viewport; `shift` moves the
 * ship right by that share of its width (an off-centre lens, so nothing is
 * cut off at a frame edge) and `scale` sizes it.
 */
export interface SectionView {
  pose: CameraPose;
  visible: boolean;
  shift: number;
  scale: number;
}

/** A section view's canvas and what the viewer has done to it. */
export interface Section {
  canvas: HTMLCanvasElement;
  context: any;
  view: SectionView | null;
  /** Where the viewer dragged the view to; null follows its pose. */
  angles: { yaw: number; pitch: number } | null;
}

export class SectionViews {
  private sections = new Map<string, Section>();
  /** The views' own idle spin, for a pose without a yaw. */
  private yaw = 0;
  /** The WebGPU device and canvas format the section canvases are configured with. */
  private gpu: { device: any; format: string };

  constructor(gpu: { device: any; format: string }) {
    this.gpu = gpu;
  }

  /**
   * The canvas of the section view `key`, created on first use. The page puts
   * it in its section; the renderer draws into it while the view is visible.
   */
  canvas(key: string): HTMLCanvasElement {
    return this.section(key).canvas;
  }

  /** Shows the section view `key` from its pose while `visible`, or stops it with null. */
  setView(key: string, view: SectionView | null) {
    this.section(key).view = view;
  }

  /** Whether any section canvas is on a page. */
  get connected(): boolean {
    return [...this.sections.values()].some((section) => section.canvas.isConnected);
  }

  /** The views on screen, each with its section. */
  get visible(): { section: Section; view: SectionView }[] {
    const shown = [];
    for (const section of this.sections.values()) {
      if (section.view?.visible) shown.push({ section, view: section.view });
    }
    return shown;
  }

  /** One frame of the idle spin. */
  update(seconds: number) {
    this.yaw += seconds * IDLE_SPIN;
  }

  /** A section view's camera angles: dragged, or its pose (spinning without a yaw). */
  angles(section: Section, pose: CameraPose) {
    return section.angles ?? { yaw: pose.yaw ?? this.yaw, pitch: pose.pitch };
  }

  private section(key: string): Section {
    let section = this.sections.get(key);
    if (!section) {
      const canvas = document.createElement('canvas');
      const context: any = canvas.getContext('webgpu');
      context?.configure({
        device: this.gpu.device,
        format: this.gpu.format,
        alphaMode: 'premultiplied',
        usage: gpuUsage.GPUTextureUsage.COPY_DST | gpuUsage.GPUTextureUsage.RENDER_ATTACHMENT,
      });
      section = { canvas, context, view: null, angles: null };
      this.sections.set(key, section);
      this.bindInput(section);
    }
    return section;
  }

  /**
   * Dragging a section view orbits its own camera; the main camera, docked in
   * the blueprint, stays put. Vertical swipes still scroll the page.
   */
  private bindInput(section: Section) {
    section.canvas.style.touchAction = 'pan-y';
    onDrag(section.canvas, {
      start: () => {
        if (!section.view) return false;
        section.angles = this.angles(section, section.view.pose);
      },
      move: (dx, dy) => {
        if (section.angles) section.angles = orbited(section.angles, dx, dy);
      },
    });
  }
}
