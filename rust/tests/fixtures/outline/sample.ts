/**
 * Cart helpers.
 *
 * More detail.
 */

import { Base } from "./base";

/** Tax rate applied to every order. */
export const TAX_RATE = 0.1;
const hidden = 5;
export let label: string = "a label that is long enough that it will have to be cut short somewhere";

/** Applies a percentage discount. */
export function applyDiscount<T extends number>(
  price: T,
  percent: number = 0,
): number {
  return price - (price * percent) / 100;
}

export const total = async (items: Item[]): Promise<number> => items.length;
const helper = (x: number) => x + 1;

export interface Item {
  /** Display name. */
  name: string;
  price?: number;
  describe(): string;
}

export type Id<T> = T | { id: T };

export enum Color {
  Red,
  Green,
}

/** A shopping cart. */
@Injectable()
export abstract class Cart<T extends Item> extends Base implements Iterable<T> {
  /** Items inside. */
  private items: T[] = [];
  static readonly limit = 10;

  constructor(private readonly owner: string) {
    super();
  }

  /** Adds one. */
  add(item: T): void {
    this.items.push(item);
  }

  abstract total(): number;
  get size(): number {
    return this.items.length;
  }
}

export default function (a: string) {
  return a;
}

namespace Shapes {
  export function area(r: number): number {
    return r * r;
  }
}

function broken() {
  let x = ;
}

/** After the error. */
export function recovered(x: number): number {
  return x;
}
