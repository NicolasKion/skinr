// A ship's translation curve that holds it still at one position.
import { ITriVectorFunction } from '@carbonenginejs/runtime/global';

/** A fixed world position in the shape space objects read their translation curve in. */
export function placement(position: number[]) {
  return new Placement(position);
}

/**
 * A ship's translation curve that holds still at `position`. The ship reads
 * it through the runtime's vector function interface every frame: the value,
 * and its first and second derivatives for motion, both zero here.
 */
class Placement extends ITriVectorFunction {
  constructor(readonly position: number[]) {
    super();
  }

  private at(out: number[]) {
    out[0] = this.position[0];
    out[1] = this.position[1];
    out[2] = this.position[2];
    return out;
  }

  Update__Ticks(_time: number, out: number[]) {
    return this.at(out);
  }

  GetValueAt__Ticks(_time: number, out: number[]) {
    return this.at(out);
  }

  GetValueDotAt__Ticks(_time: number, out: number[]) {
    return still(out);
  }

  GetValueDoubleDotAt__Ticks(_time: number, out: number[]) {
    return still(out);
  }
}

function still(out: number[]) {
  out[0] = out[1] = out[2] = 0;
  return out;
}
