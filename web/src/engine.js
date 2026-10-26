// The engine: the dominofish-wasm module (crates/dominofish-wasm/src/board.rs)
// behind text requests. It runs in a Web Worker so a long solve never
// freezes the page; where a worker cannot start, it runs on the page.

// The engine core: load the module, then call it with a request. The worker
// gets this function's own source, so the page and the worker run the same
// code without eval.
function engineCore() {
  let exports = null;
  return {
    async load(bytes) {
      const { instance } = await WebAssembly.instantiate(bytes, {});
      exports = instance.exports;
    },
    call(text) {
      const bytes = new TextEncoder().encode(text);
      const p = exports.board_alloc(bytes.length);
      new Uint8Array(exports.memory.buffer, p, bytes.length).set(bytes);
      const r = BigInt.asUintN(64, exports.board_request(p, bytes.length));
      const rp = Number(r >> 32n), rl = Number(r & 0xffffffffn);
      const out = new TextDecoder().decode(new Uint8Array(exports.memory.buffer, rp, rl).slice());
      exports.board_free(rp, rl);
      return out;
    },
  };
}

const WORKER_SRC = `${engineCore.toString()}
const core = engineCore();
self.onmessage = async (e) => {
  const { id, init, text } = e.data;
  try {
    if (init) { await core.load(init); self.postMessage({ id, out: "ok" }); return; }
    self.postMessage({ id, out: core.call(text) });
  } catch (err) {
    self.postMessage({ id, out: "error " + err });
  }
};
`;

function wasmBytes(b64) {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

class Engine {
  constructor(b64) {
    this.b64 = b64;
    this.pending = new Map();
    this.next = 1;
    this.worker = null;
    this.local = null;
  }

  async start() {
    const bytes = wasmBytes(this.b64);
    try {
      const url = URL.createObjectURL(new Blob([WORKER_SRC], { type: "text/javascript" }));
      this.worker = new Worker(url);
      this.worker.onmessage = (e) => {
        const done = this.pending.get(e.data.id);
        if (done) { this.pending.delete(e.data.id); done(e.data.out); }
      };
      const ok = await Promise.race([
        this.send({ init: bytes }),
        new Promise((resolve) => setTimeout(() => resolve("timeout"), 4000)),
      ]);
      if (ok !== "ok") throw new Error(ok);
    } catch (err) {
      if (this.worker) this.worker.terminate();
      this.worker = null;
      this.local = engineCore();
      await this.local.load(bytes);
    }
  }

  send(msg) {
    const id = this.next++;
    return new Promise((resolve) => {
      this.pending.set(id, resolve);
      this.worker.postMessage({ id, ...msg });
    });
  }

  // One request; resolves to { ok, body } or { ok: false, error }.
  async request(command, record = "") {
    const text = record ? `${command}\n${record}` : command;
    const out = this.worker ? await this.send({ text }) : this.local.call(text);
    if (out.startsWith("ok\n")) return { ok: true, body: out.slice(3) };
    return { ok: false, error: out.replace(/^error /, "") };
  }
}
