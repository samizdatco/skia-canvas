import * as originalAssert from 'assert';
export {describe, test, beforeEach, afterEach} from 'node:test';

type ExtendedAssert = typeof originalAssert & {
  contains(actual: unknown, expected: unknown): void;
  doesNotContain(actual: unknown, expected: unknown): void;
  matchesSubset(actual: unknown, expected: unknown): void;
  nearEqual(actual: number, expected: number): void;
}

export declare const assert: ExtendedAssert;
