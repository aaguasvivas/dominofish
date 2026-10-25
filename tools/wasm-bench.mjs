// Playout bench under Node (M1-10): loads the dominofish-wasm module with
// WebAssembly.instantiate (no dependencies), runs the seeded playouts for a
// mode, and prints the checksum and rounds per second.
// Usage: node tools/wasm-bench.mjs <wasm file> <1v1|2v2> <rounds> <seed>

import { readFileSync } from "node:fs";

const [file, mode, rounds, seed] = process.argv.slice(2);
const { instance } = await WebAssembly.instantiate(readFileSync(file), {});
const run = instance.exports.playouts_checksum;
const code = mode === "1v1" ? 1 : 2;
const start = performance.now();
const checksum = BigInt.asUintN(64, run(code, Number(rounds), Number(seed)));
const secs = (performance.now() - start) / 1000;
console.log(`wasm playouts: mode ${mode} rounds ${rounds} seed ${seed} checksum ${checksum.toString(16)} seconds ${secs.toFixed(3)} rounds/s ${Math.round(Number(rounds) / secs)}`);
