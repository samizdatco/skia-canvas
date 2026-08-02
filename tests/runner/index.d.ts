export {describe, test, beforeEach, afterEach} from 'node:test';

// `typeof import('assert')` (unlike a `* as` namespace import) preserves the
// module's callable `export =` signature, so bare `assert(...)` calls typecheck
type ExtendedAssert = typeof import('assert') & {
  contains(actual: unknown, expected: unknown): void;
  doesNotContain(actual: unknown, expected: unknown): void;
  matchesSubset(actual: unknown, expected: unknown): void;
  nearEqual(actual: number, expected: number): void;
}

export declare const assert: ExtendedAssert;
