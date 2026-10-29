# Why the decompile/recompile pipeline is shaped this way

The toolchain can lift a foreign ROM's 68000 code to C-like pseudo-source
(`--decompile`), recompile that C with its own backend, and — the hard part —
*prove* the result behaves identically to the original. This page explains
the design decisions behind that proof and the code generation it depends
on. For procedures see [the how-to](../how-to/rebuild-a-rom-from-decompiled-c.md);
for the byte-exact assembly path see
[round-tripping](../how-to/roundtrip-a-rom.md).

## The problem

"Compiles" is weak: the lifted C can typecheck and still misbehave — a
lifted flag condition inverted, a sub-word operation reading too many bytes,
a register argument never passed. The pipeline therefore rests on an
executable oracle rather than on static guarantees.

## The oracle: a deterministic interpreter with a narrow contract

`run_rom` executes a ROM instruction by instruction and produces a
`RunTrace`: the final 64 KB of work RAM, the sequence of VDP port writes,
the instruction count, the run's stack window, and the first execution
error, if any. `compare_traces` demands equal RAM and equal VDP write
sequences.

Two deliberate narrowings make the comparison tractable:

- **VDP writes, not timing.** The compiled C is slower than hand-written
  assembly, so the number of instructions between two VDP writes differs
  between the runs. The *sequence* of writes (address, value) is the
  observable that must match; per-write timing is excluded on purpose.
- **Per-run stack windows.** Stale stack residue above each run's deepest
  stack use is not semantic state, and the two runs sit on *different*
  stacks (see the private stack below). Comparison therefore excludes each
  run's own `[stack_floor, stack_top)` window; a single shared cutoff would
  silently hide real global differences between the windows.

## Two rebuild strategies, one substrate rule

Both builders keep the original image's *data* at its original addresses,
because lifted code references it through absolute addresses the compiled
binary must honor:

- The **hybrid** leaves the whole original image in place and appends the
  compiled functions after it (outside the original checksum range, so the
  boot self-check passes), repointing the exception vectors at the compiled
  functions. Interrupt handlers get a full register save/restore wrapper and
  return with `rte`.
- The **data-only** rebuild erases every flow-verified instruction byte.
  What survives is only what the analysis could *not* prove to be code —
  protected data windows behind absolute references, thunk islands, and
  undiscovered regions. The checksum over the boot-verified range is
  recomputed. This is the stronger claim: 100% of the executed code comes
  from the lifted C.

## The private stack

The first full rebuild crashed into a class of bug that no lifter fix could
address: the compiled code's frames are an order of magnitude deeper than
the original's stack budget (the lifted C uses memory-only codegen with
frame slots for every local), and the original's data can begin well below
its initial stack pointer. With the original SP, the compiled call chain
grows straight through the program's line buffers and globals.

The fix is structural: the builder runs a short probe of the original under
the interpreter, finds the largest work-RAM window the program never
touches below its deepest stack use, and repoints the initial stack pointer
(vector 0, outside the checksum range) at the top of that window. One
shared stack keeps nested interrupt frames consistent.

## The census method

Gates fail with a *first divergence*, and every divergence in the Tetris
work was closed with the same loop:

1. Correlate the divergence with a program location using env-gated
   interpreter instrumentation: `SMDC_WATCH` (writes to one RAM address,
   with PC), `SMDC_WATCH_VDP` (VDP writes with PC), `SMDC_TRACE_FROM/TO`
   (PC window), `SMDC_DEBUG_SPANS` (dispatch resolution).
2. Diff the lifted C against the disassembly at that location.
3. Fix the *model* — lifter fidelity, interpreter semantics, encoder
   symmetry — never the lifted source itself.

Bug classes this surfaced, in the order they matter to anyone extending the
lifter: flag conditions must survive branches (a `beq`/`bmi` pair on one
`move`), sub-word results must be sign-cast for N-flag consumers and
size-confined in registers, `subq.w`/`clr.w` must lift at their size (a
long RMW borrows through neighbouring bytes and freezes counters),
`movea.w` sign-extends into the address register, callee-mutated register
arguments are visible to callers (rendered as pointer out-parameters), a
leading `movem` save defines nothing (so real argument reads still count),
and a function that falls through into the next function's body must emit
that tail call.

## Frame budgets are test pacing, not real time

The interpreter couples VBlank firing to the instruction budget (90% of
each frame's budget). For two runs to take their VBlank-synced main-loop
passes in lockstep, the compiled rebuild's setup must complete before the
first simulated VBlank. The gates use 7.6M instructions per frame because
the compiled setup needs ~6.4M — a *pacing* choice, not a hardware model: a
real Mega Drive frame executes on the order of 10⁵ instructions depending
on instruction mix, so the simulation checks semantics, never real-time
behavior. The original is spin-wait driven and insensitive to the budget,
which keeps the same-config comparison fair.

## Code generation speed, measured

Behavioral equality forced the compiled code to be *fast enough to reach
its first main-loop pass inside the gate budget*, and the setup-instruction
count (the marker write `$F632 = $28` in the Tetris boot) became the
tracking metric:

| Change | Setup instructions |
|---|---|
| Original codegen (locals as alloca + pointer cells) | 14.8M |
| Locals accessed directly; `lea` only when the address escapes | 11.6M |
| IR copy propagation + constant folding + D0/D1 move peephole | 10.1M |
| Register-targeted ALU/load emission with block-local last-use tracking | 6.39M |

The large steps came from the *emitter*, not the IR: the IR passes alone
moved the number ~1%, because the fat was the fixed
`load D0 → load D1 → op → move out` shape of every operation. The final
step computes ALU results directly into the destination's cache register —
reusing a dying operand's register as the compute target — which is sound
with block-local last-use counts rather than global liveness.

Equally instructive are the measured **failures**. Three caching schemes
were built, measured, and reverted because LRU churn cost more than the
hits saved on this workload: caching variable slots in the data-register
cache (pointer-heavy loops invalidate them every iteration), parking
pointer locals in A2–A5 registers (five hot pointers over four registers),
and eliding per-iteration flushes in self-loop blocks (with more live temps
than registers, spill-and-reload beats continuous eviction). The lesson
that held: with a fixed block-local cache, optimize *instruction selection*
toward the registers, not cache layers on top of it.

## Limits

- The interpreter models VDP access as a write log — no VRAM semantics, no
  DMA timing. Equality is defined at the port level.
- The behavioral gates are CPU-bound tests that interpret hundreds of
  millions of instructions; they are the cost of the guarantee.
- The lifted C is compilable pseudo-C, not idiomatic source: register
  variable names (`d0`, `a4`), `goto`-shaped control flow where the
  structurer has no pattern, and absolute-addressed globals as `ram_*`
  defines.

## Source of truth

- Interpreter and comparison: `crates/smdc/src/backend/m68k/interp.rs`
- Rebuilders: `crates/smdc/src/backend/m68k/hybrid.rs`
- Lifter: `crates/smdc/src/backend/m68k/decomp.rs`
- Code generator and its caches/peephole: `crates/smdc/src/backend/m68k/emit.rs`
- IR passes: `crates/smdc/src/ir/opt.rs`
- Behavioral gates: `crates/smdc/tests/tetris_roundtrip.rs`
