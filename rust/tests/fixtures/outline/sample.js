#!/usr/bin/env node
/**
 * Small utilities.
 */

const fs = require("fs");

/** Joins two paths. */
function join(a, b) {
  return a + "/" + b;
}

export const DEFAULTS = { retries: 3, verbose: false, names: ["a", "b", "c", "d", "e", "f"] };

const double = (x) => x * 2;

/** An event hub. */
export class Hub extends Emitter {
  static count = 0;
  #secret = 1;
  handler = (event) => {
    this.emit(event);
  };

  /** Starts it. */
  async start(options = {}) {
    return options;
  }

  static create() {
    return new Hub();
  }
}

export default class {
  run() {}
}

function broken() {
  let x = ;
}

/** After the error. */
function recovered(x) {
  return x;
}

(function () {
  /** Inside a wrapper. */
  function wrapped(n) {
    return n;
  }
})();
