# DominoFish formats

## Capi oracle

The oracle is Capi's engine at commit 1d8868a26ce930f478f98e64060c06b1d1a2c1f4, read from CAPI_DIR (default /Users/Adelson/Desktop/personal/capi) and never written:

```bash
tools/capi-oracle/build.sh
```

It checks that CAPI_DIR holds the pinned commit, extracts packages/engine/src with git archive into tools/capi-oracle/build/ (gitignored), and bundles index.ts with Capi's esbuild 0.21.5 into tools/capi-oracle/build/capi-engine.mjs. Oracle scripts run with /Users/Adelson/.nvm/versions/node/v20.20.0/bin/node and import tools/capi-oracle/lib.mjs, which refuses a bundle from another commit. The tools have no npm dependencies and no package.json.

### Deal layout

A deal is the 28 tiles in Capi's layout (docs/RULES.md R3): seats in order (2v2 n, e, s, w; 1v1 n, s), seven tiles each in hand order, then the 1v1 boneyard, whose last tile is drawn first. Tiles are written low pip first, as "a-b" in text and [a, b] in JSON.

### mulberry32

Copied from packages/engine/__tests__/invariants.fuzz.test.ts:17-26 at the pin, with the type annotations removed, into tools/capi-oracle/lib.mjs. Oracle scripts pass it as the rng of createInitialState and startNewRound, as Capi's fuzz does at invariants.fuzz.test.ts:357-360, so a seed reproduces Capi's deals. tools/capi-oracle/selftest.mjs checks the copy against the four whole-suit seeds of invariants.fuzz.test.ts:460 (59313, 83322, 282144, 645301), whose 2v2 deals give the 6-6 holder all seven sixes.

```js
export function mulberry32(seed) {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
```

### Fixed deals

To make Capi deal a wanted order, lib.mjs's fixedDealRng(order) returns an rng for Capi's Fisher-Yates shuffle (reducer.ts:51-58): for i from 27 down to 1 it returns (j + 0.5) / (i + 1), where j is the current index of the tile wanted at position i, and it applies the same swap to its own copy of the array. dealFixedRound calls startNewRound on a round_over state whose lastCallout is "domino" and lastPlayedBy is the wanted leader (as invariants.fuzz.test.ts:488-499 does), so the round starts on an empty board with that leader and the wanted hands and boneyard. selftest.mjs checks it on 1,000 seeded orders (500 per mode).
