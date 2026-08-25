# ScopeFolio v0.2 — Geometry / Partitioning Design Proposal

**Status:** COMPLETE — implemented, validated, frozen (v0.2.0). The formal
specification is `SPEC.md` (v0.2.0); the experimental record is frozen in
`EXPERIMENTS-ARCHIVE.md`.
**Base:** SPEC v0.1.0 (superseded by v0.2.0)
**Evidence:** `harness/RESULTS-SCOPEFOLIO.md` (phases 1–3), `harness/RESULTS-SCOPEFOLIO-PHASE4.md` (phase 4)
**Scope of this document:** geometry and partitioning semantics only. No implementation, no experiments, no changes to the v0.1.0 spec or the pi-fc-search production code.

---

## 1. Motivation

Phase 3 and phase 4 established that the *mechanism* of ScopeFolio (line → deterministic local scope, stateless, tree hidden) is sound, and that the *default geometry* (halving until leaves ≤ `partition_lines`, pl=50) is actively harmful for a 4 B agent: paging storms (dupRead 11.2), turn burn (maxTurns 100%), NOANSWER (8/12).

Growing the partition width (50 → 200 → 400) monotonically reverses every failure metric. But pl=400 is **not** a verified answer:

* On the largest ground-truth file (`extensions/index.ts`, 453 lines) the agent still faced **two** slices of ~227 lines and fell into same-slice re-read loops (9–13 reads of 2–3 distinct offsets until the turn wall).
* The v0.1.0 algorithm never actually produces "400 + a small remainder" — it bisects, so 453 lines with target 400 yields **226 + 227**, and a 1000-line file with target 400 yields **four 250-line leaves** instead of three ~333-line leaves.

The recurring pattern is that `partition_lines` is *documented* as a target but *behaves* as a strict cap combined with a depth-first halving rule that does not minimize the number of leaves and lets leaf sizes drop to ~50% of the target.

**v0.2 thesis:** re-define `partition_lines` from *strict upper bound on leaf width* to **target leaf size**, and derive the leaf layout from it as a nearest-integer, balanced, even split. "400 is right" is a *window-size* question (still open, phase 5); "a 453-line file must be one scope when the target is 400" is a *geometry* question, and v0.2 answers it.

---

## 2. The problem with v0.1.0's geometry

The v0.1.0 rule (`src/partition.rs`, SPEC §6.1): bisect `[start, end]` in half while `len > target`; leaves satisfy `len ≤ target`.

Three structural consequences:

### 2.1 It is a cap, not a target

A file only slightly above the target is split into two half-size leaves instead of being returned whole:

```text
n = 453, t = 400   →   226 + 227        (two leaves of 56% of target)
n = 401, t = 400   →   200 + 201        (two leaves of ~50% of target)
n = 453, t = 200   →   113 + 113 + 113 + 114   (four leaves)
```

The agent asked for "one scope ≈ 400 lines around this line" and received 227. The phase-4 data confirms the cost: `index.ts` at pl=400 produced two-slice re-read loops that kept Q8 below baseline on the hardest condition (phase 4 report, §6: `a1t_f400 it2: NOANSWER turns=16 paged=[index.ts×12 @2]`).

### 2.2 Leaf count is not minimized

Bisecting minimizes *depth*, not the number of leaves the agent may have to walk:

```text
n = 1000, t = 400  →  500+500 → 250×4        (4 leaves, each 62.5% of target)
n = 1200, t = 400  →  600+600 → 300×4        (4 leaves, each 75% of target)
```

A 1000-line file *could* be three ~333-line leaves (one target-sized read unit each) but v0.1.0 gives four sub-target reads. Every extra leaf is an extra boundary the agent may page across.

### 2.3 Leaf sizes fall to ~50% of the stated target

Because a leaf is only produced when `len ≤ target` *after* a halving step, every leaf of a split file lands in `(t/2, t]`. "pl=400" therefore returned 227-line scopes on the key file; the phase-4 median scope size (12 K ≈ 227 lines at ~50 B/line) matches this exactly. The parameter name oversells what the agent gets.

**Observed causal chain (phases 3–4):**

```text
small leaf → answer region spans k leaves → agent must issue k scope reads
           → reads compete with other tool results for the 64 KiB context budget
           → earlier scopes are evicted → same slices re-fetched (paging)
           → turns burned → maxTurns → NOANSWER
```

Phase 4, index.ts (453 lines), Instruct: 8 leaves (pl=50) → dupRead 11.2; 4 leaves (pl=200) → 7.0; 2 leaves (pl=400) → 2.4. Paging cost scales with **leaf count**, which is why the trend is monotone in `t` — and why the remaining failure at pl=400 is the *two* leaves of a file that conceptually fits in one.

---

## 3. Findings from phase 3 / phase 4 that constrain the design

From `harness/RESULTS-SCOPEFOLIO.md` (phases 1–3) and `harness/RESULTS-SCOPEFOLIO-PHASE4.md`:

| # | Finding | Design consequence |
|---|---------|--------------------|
| F1 | Paging (dupRead, maxSameFileReads, ScopeFolio reads/run) is **strictly monotone in leaf width** across 50→200→400 in both Thinking and Instruct, and tracks baseline as width grows. | Leaf count per file is the dominant cost driver. Minimize it. |
| F2 | At pl=400, all GT files ≤ 400 lines become single leaves and their queries reach baseline (Q6/Q10/Q12). The *only* sub-baseline case is Q8, whose 453-line target still splits into two. | A file at ~1.1× target MUST be a single scope. Small-file rule must extend past `n ≤ t`. |
| F3 | The pl=400 residual is *same-slice re-reads* (`uniqueOffsets ≈ 2`, 3–13 reads of the same two slices), a different mechanism from paging — suspected tool-result eviction. | Geometry can remove the 2-slice requirement (→ 1 scope); it cannot by itself remove re-reads of evicted content, but one large scope is re-fetchable in one call and stays coherent. Keep offset expansion available (open). |
| F4 | Scope sizes of 12–14 K median at pl=400 are same-order as normal reads (17–20 K) and cause no token regression (Instruct −11% vs baseline). | Target leaf sizes in the few-hundred-line range are affordable for 4 B agents. No need to shrink scopes for token reasons. |
| F5 | Zero ScopeFolio backend errors across all 96+ forced runs; determinism never broke. | Preserve stateless, content-independent (line-count-dependent) layout. Do not add caching, hashing, or identity. |
| F6 | Adoption probes: the visible tool was never chosen (0/15 LFM, 0/8 A1); agents perform fine without it. | v0.2 must not change the agent-facing API at all — the geometry fix must be invisible to the agent by construction. |

**Design principles derived:**

* **P1 (read unit):** `partition_lines` is the *size of one read unit* the caller wants. If the file fits in a little under one-and-a-half read units, return the whole file as one scope.
* **P2 (slice economy):** the leaf count is derived as `k = round(n/t)` — the integer leaf count nearest to the target count — not as a byproduct of recursive halving; paging cost (∝ leaf count, F1) is thereby controlled by `t` alone.
* **P3 (uniformity):** leaf sizes must be balanced (differ by at most 1 line) so every scope read behaves the same — no "tiny last slice" anomaly for the agent to react to.
* **P4 (target, not cap):** leaves may exceed the target by a bounded factor (up to 1.5× in the single-leaf case, up to 1.25× for k=2); the guarantee is on the *band*, not on the cap.
* **P5 (separation of duties):** *which leaf owns the line* (geometry) and *how far past the leaf we read* (offset) are independent, composable stages.
* **P6 (invisibility):** nothing in this change may leak into the agent-facing API, the output format, or the prompt.

---

## 4. Design Goals

ScopeFolio v0.2 MUST:

1. keep the agent-facing API exactly as v0.1.0: `read(file, line, partition_lines, offset_ratio)`;
2. treat `partition_lines` as a **target leaf size** (a read-unit size), not a hard cap;
3. return the **entire file as a single scope** whenever the file length `n` satisfies `n < 3t/2` (in particular for all `n ≤ t`);
4. otherwise split into `k = max(1, round(n / t))` leaves of **equal size, differing by at most one line**;
5. keep every leaf within `[3t/4, 3t/2)` lines for `n ≥ t` (band guarantee, §6.4);
6. keep the partition structure a deterministic, stateless, per-invocation computation from the file's line count;
7. keep the binary tree as the internal representation for scope location, never exposed;
8. separate offset expansion from partition geometry, with `offset_ratio` defined against the target leaf size;
9. preserve all v0.1.0 guarantees: determinism, content preservation, boundary clamping, error semantics.

SHOULD:

* minimize the number of leaves the agent may need to cover a region of the file;
* keep the leaf layout computable in O(1) (arithmetic), with the tree as a derived view;
* remain a small standalone Rust tool with no new dependencies.

Non-goals (unchanged from v0.1.0 §3, plus):

* automatic/per-model selection of `t`;
* semantic or structure-aware partitioning;
* any change to the agent-facing interface or prompt;
* eviction-aware re-read caching (out of scope; flagged in §16).

---

## 5. Agent-facing abstraction

Unchanged in v0.2. The agent specifies WHERE; ScopeFolio determines HOW MUCH.

```text
target_leaf_size (t)           ← caller config, stable per invocation
      │
      ▼  k = max(1, round(n / t))
leaf layout  L(n, t)           ← k balanced intervals, pure arithmetic
      │
      ▼  (canonical binary tree over the leaves — internal only)
locate leaf containing `line`
      │
      ▼  offset expansion ± floor(r·t), clamped to [1, n]
render: "file:START-END" + numbered lines
```

The agent never sees `k`, leaf boundaries, the tree, or the offset computation. The only observable effect of v0.2 vs v0.1.0 is *which lines come back*, and always the invariant: the requested line is in the returned range.

This abstraction holds because the geometry is a **pure function of (n, t)** with a closed-form layout: there is no tree-walk residue, no search heuristic, no state — so "target size → internal partition → leaf → scope → render" is total and deterministic by construction.

---

## 6. Partition geometry

### 6.1 Definitions

Let `n` = file line count (≥ 1), `t` = `partition_lines` (≥ 1, integer).

```text
k   = max(1, round_half_up(n / t))          # leaf count
b_i = floor(n · i / k)                      # boundary i, i = 0..k  (b_0 = 0, b_k = n)
leaf i  = lines [ b_{i-1} + 1 .. b_i ]      # i = 1..k, 1-based
```

`round_half_up(x) = floor(x + 1/2)`. Integer form: `k = max(1, (2n + t) / (2t))` (integer division).

### 6.2 Why this rule

* **`n < 3t/2 → k = 1`:** the whole file is one scope. This covers `n ≤ t` (the small-file rule, §8) *and* the 453/400 case from Q8 — the direct fix for the phase-4 residual.
* **`k = round(n/t)`** minimizes `|k − n/t|`: it is the leaf count whose leaf sizes are closest to the target size, i.e. it implements "`t` is the desired leaf *size*" exactly, with the leaf count as a derived value. (The alternative objective — fewest leaves subject to a cap — is the S2 family, considered and rejected in §6.3 and O2.)
* **Even boundaries `floor(n·i/k)`** make leaf sizes `floor(n/k)` or `ceil(n/k)` — balanced to ±1 line (P3), with no structurally small last leaf.

### 6.3 Comparison of candidate schemes

| Scheme | 453 / 400 | 1000 / 400 | 1200 / 400 | 800 / 400 | Leaf-size behavior | Verdict |
|--------|-----------|------------|------------|-----------|--------------------|---------|
| S1: v0.1.0 halving (cap `t`) | 226 + 227 | 250 × 4 | 300 × 4 | 400 + 400 | leaves in `(t/2, t]`; count not minimized; cap semantics | **rejected** — the phase-3/4 failure mode |
| S2: halving with stop at `len ≤ 1.5t` | 453 (single) | 500 + 500 | 600 + 600 | 400 + 400 | balanced, but leaf count is a power of 2; leaves drift to 1.25–1.5× target; redefines `t` as a *cap* (1.5t) rather than a size | viable; runner-up. Fewer slices than S3 but abandons the "read unit ≈ t" meaning (a 1200-line file at t=400 becomes two 600-line units, contradicting the caller's stated 400-line read unit) |
| S3: **round-count even split (proposed)** | **453 (single)** | **333 + 333 + 334** | **400 + 400 + 400** | 400 + 400 | sizes `floor(n/k)`/`ceil(n/k)`; band `[3t/4, 3t/2)`; target keeps its meaning as a size | **adopted** |
| S4: remainder absorption (greedy `t`-chunks: `400 + 400 + 200`) | 453 (single, via n ≤ 1.5t) | 400 + 400 + **200** | 400 + 400 + 400 | 400 + 400 | prefix-stable boundaries (multiples of `t`), but a structurally half-size last leaf; the last slice read behaves differently from all others | rejected as primary; prefix-stability noted (§16) |

S3 is the only scheme that (a) returns 453/400 whole, (b) yields "400 + 400 + 400" for 1200/400 as the design brief expects, (c) keeps every leaf within a small band of the target, and (d) has a closed form with no recursion or balancing rule to specify.

### 6.4 Invariants (all MUST hold, all testable)

For `n ≥ 1`, `t ≥ 1`:

* **I1 (coverage):** the leaves are disjoint, contiguous, and cover `[1, n]` exactly.
* **I2 (count):** `k = max(1, round(n/t))`.
* **I3 (balance):** every leaf has `floor(n/k)` or `ceil(n/k)` lines.
* **I4 (small file):** `n < 3t/2 ⟹ k = 1` (in particular `n ≤ t ⟹` single scope).
* **I5 (band):** for `n ≥ t`, every leaf has a size in `[3t/4, 3t/2)`.
* **I6 (exact division):** `t | n ⟹ k = n/t` and every leaf has exactly `t` lines.
* **I7 (purity):** the layout depends only on `(n, t)` — not on file contents, invocation history, or environment.

### 6.5 Layout examples (t = 400)

| n | k | leaves |
|---|---|--------|
| 1 | 1 | [1] |
| 138 | 1 | [138] |
| 265 | 1 | [265] |
| 400 | 1 | [400] |
| 453 | 1 | [453] — *Q8 case: whole file, one scope* |
| 599 | 1 | [599] |
| 600 | 2 | [300, 300] |
| 800 | 2 | [400, 400] |
| 1000 | 3 | [333, 333, 334] |
| 1200 | 3 | [400, 400, 400] |
| 2000 | 5 | [400, 400, 400, 400, 400] |
| 10000 | 25 | [400 × 25] |

Ground-truth files of the phase-3/4 experiment at t = 400: `utils.ts` (138) → 1, `agent.ts` (168) → 1, `context.ts` (265) → 1, `extensions/index.ts` (453) → **1** (was 2 under v0.1.0). At t = 400, v0.2 makes *all four* experiment targets single-scope.

---

## 7. Binary tree construction

The binary tree is retained as the **canonical internal structure** (SPEC v0.1.0 §16 principle), now built *over the leaf layout* rather than by recursive halving:

```text
canonical tree = balanced binary tree whose leaves are exactly L(n,t)

build(leaf index range [i..j]):
    if i == j:  leaf node [b_{i-1}+1 .. b_i]
    else:       m = (i + j) / 2           (floor)
                node with children build([i..m]) and build([m+1..j])
```

* Internal nodes are contiguous unions of whole leaves; every node remains a `[start, end]` line interval — the same `PartitionNode` shape as v0.1.0.
* Depth is `⌈log2 k⌉ + 1` levels (e.g. 100 K lines at t = 400 → k = 250 → 9 levels).
* Construction is a pure function of `(n, t)`; cost `O(k)`, negligible at realistic sizes.

**Line resolution path:** locate the leaf by walking the canonical tree from the root (compare `line` with the left child's `end`). Equivalently, in closed form: `leaf index j = ceil(line · k / n)` (1-based). The two are provably equivalent for all `1 ≤ line ≤ n`; the implementation MAY use either, but MUST agree (property test). Keeping the tree walk as the canonical path preserves the v0.1.0 lookup code shape and gives Sliding Bisection (§17, v0.1.0 spec) a ready hierarchical interval structure with zero API change.

**Is the tree necessary?** For the v0.2 read path, no — the arithmetic index suffices. It is kept because: (a) one lookup code shape across v0.1→v0.2 reduces review surface; (b) Sliding Bisection needs a hierarchy of intervals, and deriving it from the layout is free; (c) it bounds worst-case depth for future search-guided refinements; (d) at realistic `k` it is not a performance concern. The arithmetic form is documented (and tested) as the O(1) equivalent. Neither form is exposed.

---

## 8. Small-file / remainder handling

* **Rule (MUST):** `n ≤ t` → the file is a **single scope**; no partitioning occurs. This adopts the brief's small-file proposal as specification.
* **Extension (consequence of §6.1):** `t < n < 3t/2` → also a single scope. Explicitly: **453 lines at target 400 is one scope of 453 lines.** Justification: F2/F3 — this is exactly the Q8 case, and the phase-4 data show that splitting a file that conceptually fits one read unit is the last remaining geometry-induced failure.
* **Remainder:** there is no "remainder leaf" in v0.2. The remainder of `n / k` is distributed one line at a time across the layout by the `floor(n·i/k)` boundaries, so no leaf is structurally smaller than any other (contrast S4's 200-line last leaf in 1000/400).
* **Boundary stability note:** leaf boundaries move slightly when `n` changes (e.g. 1000→1001 lines shifts some boundaries by one line). This is acceptable: ScopeFolio is a computed view of the *current* file (SPEC v0.1.0 §11), and the agent's line numbers come from fresh Grep results against the same current file. (S4's prefix-stable multiples-of-`t` boundaries are recorded as a rejected alternative, §16.)

---

## 9. Line resolution

`read(file, line, partition_lines = t, offset_ratio = r)`:

1. Open `file`; determine line structure (`n` lines).
2. Compute `k = max(1, (2n + t) / (2t))` and the boundaries `b_i`.
3. Locate the leaf `[b_{j-1}+1 .. b_j]` containing `line` (tree walk over the canonical tree, or equivalently `j = ceil(line·k/n)`).
4. Expand by the offset (§10): `o = floor(r · t)`; range = `[max(1, b_{j-1}+1−o), min(n, b_j+o)]`.
5. Render the range (v0.1.0 output format unchanged).

Invariants: the requested line is always in the returned range; the range is clamped to the file; resolution is a pure function of `(contents→n, line, t, r)`.

---

## 10. Offset expansion

Two strictly separated stages (P5):

```text
partition geometry:  which leaf owns the line          →  leaf [s, e]
offset expansion:    how far past the leaf to read     →  [s−o, e+o], clamped
```

* **Definition (v0.2, MUST):** `o = floor(offset_ratio × t)` lines, applied symmetrically, clamped to `[1, n]`.
* `offset_ratio` is computed against the **target leaf size `t`** — the stable caller-supplied constant of the invocation — not against the actual leaf size (which varies by ±1 line and would make the offset a function of an internal detail) and not against the *final* scope size (which is circular: scope = leaf + 2o).
* Consequences:
  * the offset is **uniform across all leaves of a file** (same margin everywhere) — the agent sees one consistent read behavior;
  * v0.1.0's example remains true: `t = 50, r = 0.1 → o = 5`;
  * at `t = 400, r = 0.1` the margin is 40 lines each side (final scope ≈ `t + 2rt = 480` lines, unclamped);
  * `r = 0` (the phase-3/4 experimental value) means *exactly* the leaf, no expansion — geometry and offset remain independently tunable.
* Offsets never cross the "whole file" case: if `k = 1`, the offset is clamped to the file and the result is the whole file regardless of `r`.
* Re-evaluation of the *value* of `offset_ratio` (e.g. overlapping adjacent slices at boundaries) remains deliberately deferred — it is a follow-up once the window size is settled (open decision #2 of the phase-4 handover). v0.2 only fixes the *semantics*.

---

## 11. Determinism

Unchanged obligations, new layout:

* For identical `(file contents, line, t, r)` the output — range and rendered text — is byte-identical across invocations and machines.
* No randomness, no LLM inference, no external retrieval, no clock, no locale sensitivity in scope resolution.
* The layout depends only on `n` (line count) and `t`/`r` — not on content — exactly as in v0.1.0. (A file edit that changes line counts relocates leaves; the view is always of the *current* file.)
* Integer arithmetic only; the specification formulas (`(2n+t)/(2t)`, `floor(n·i/k)`, `ceil(line·k/n)`, `floor(r·t)`) are the normative definitions. (If `offset_ratio` is represented as a float, `o` is defined as `floor(float(r) · t)` with the standard float value; the CLI parser MUST document this.)

Testable properties: I1–I7 above, plus tree-walk ≡ arithmetic index for all lines, plus determinism re-run checks.

---

## 12. Statelessness

Unchanged: ScopeFolio is a computed view, not an index.

* Every invocation reconstructs `L(n,t)` and the canonical tree from the current file; nothing persists between calls.
* No cache, no partition metadata, no file hash, no session identifier (v0.1.0 §11, §12 retained).
* The statelessness guarantee is what makes the offset/paging economics *deterministic per call*: the same `(file, line)` always returns the same scope, so an agent re-issuing it gets an identical, predictable result (which is also why re-reads are at least cheap and exact — and why eviction of that result, a context-management issue, is out of ScopeFolio's scope).

---

## 13. Error semantics

Unchanged from v0.1.0 §20, with the validation set:

| Condition | Behavior |
|-----------|----------|
| file not found / unreadable | deterministic error (v0.1.0 codes unchanged) |
| `line < 1` or `line > n` | deterministic error; no silent substitution |
| `partition_lines < 1` or non-integer | deterministic error |
| `offset_ratio < 0` or non-finite | deterministic error |
| empty file (`n = 0`) | deterministic error (no valid line exists) |

No new error classes are introduced. The geometry never fails: for every valid input, `k ≥ 1` and the layout exists.

---

## 14. Examples

**E1 — Q8 case (the motivating one).** `extensions/index.ts`, n = 453, t = 400, r = 0, line = 453:
`k = (906+400)/800 = 1` → leaf [1..453] → scope `1-453`. One call returns the entire file. (v0.1.0 at the same settings: leaves [1..226], [227..453] — line 453 returned only `227-453`, the other half invisible until a second call, the observed re-read loop setup.)

**E2 — small file.** n = 300, t = 400, any line → `k = 1` → whole file, one scope, per the §8 rule.

**E3 — exact division.** n = 1200, t = 400 → `k = 3` → [1..400], [401..800], [801..1200]. Line 800 → leaf 2.

**E4 — non-divisible.** n = 1000, t = 400 → `k = 3` → boundaries 0, 333, 666, 1000 → leaves [1..333], [334..666], [667..1000]. Line 333 → leaf 1; line 334 → leaf 2 (boundary line).

**E5 — offset.** n = 1000, t = 400, r = 0.1, line = 700 → o = floor(0.1×400) = 40; leaf [667..1000]; range = [max(1, 667−40), min(1000, 1000+40)] = **627-1000** (374 lines).

**E6 — offset at file end.** n = 453, t = 400, r = 0.1, line = 453 → leaf [1..453]; range = [max(1,−39), 453] = 1-453 (whole file; the `k=1` clamp case).

**E7 — fine grain.** n = 1193, t = 50 → `k = (2386+50)/100 = 24` → 24 leaves of 49/50 lines (sizes `floor(1193/24)=49`, `ceil(1193/24)=50`). Line 597 → `j = ceil(597·24/1193) = ceil(12.007…) = 13` → leaf [b_12+1 .. b_13] = [597..646] (the target line lands exactly on the first line of its leaf).

**E8 — v0.1.0 comparison (same call, old geometry).** n = 1193, t = 50 → v0.1.0 halving gives **32 leaves of 37/38 lines** (minimum leaf 37 = 74% of target, 32 slices); v0.2 gives **24 leaves of 49/50 lines** — fewer slices and sizes within 2% of target. For `n < 3t/2` cases the difference is largest: v0.1.0 splits, v0.2 does not.

---

## 15. v0.1.0 → v0.2.0 migration considerations

1. **Parameter meaning.** `--partition-lines` keeps its name (CLI and harness stability — the forced-Read wrapper and all experiment records remain comparable) but its *semantics change*: from "maximum leaf width under halving" to "target leaf size under round-count even split". This is the single conceptual migration; everything else follows. (Optional alias `--target-leaf-lines` — see §16.)
2. **Behavior delta (per `(n, t)`):**
   * `n < 3t/2`: v0.1.0 may split (2 leaves of `n/2`); v0.2 returns the whole file. Scopes get *larger* in exactly the cases the experiments flagged as harmful splits.
   * `t | n`: identical leaves (both produce `n/t` leaves of `t` lines) — e.g. 1200/400, 800/400.
   * otherwise: leaf *boundaries move* (v0.2 even split vs v0.1.0 halving) and leaf sizes rise from `(t/2, t]` to the `[3t/4, 3t/2)` band. Boundaries are internal; the agent is unaffected by construction (P6).
3. **Default.** v0.1.0's default `t = 50` is experimentally "actively harmful". The v0.2 default is `t = 400` (phase-4 baseline-equivalence point, and under v0.2 geometry it makes every GT file of the experiment a single scope); **confirmed by 6-C** (t=600 live arm: geometrically identical, behaviorally indistinguishable — `RESULTS-SCOPEFOLIO-V0.2-6C.md`). `offset_ratio` default is 0 (**confirmed by Phase 7**).
4. **Tests.** The 55-test suite's partitioning/boundary assertions encode the v0.1.0 halving geometry and MUST be re-derived from I1–I7 plus the §22 test list (small files, exact boundaries, odd counts, large files) — the *test categories* are unchanged, the expected layouts are not.
5. **Specification.** This document is a proposal; the frozen `docs/SPEC.md` (v0.1.0) is not modified. On approval, a formal SPEC v0.2.0 supersedes §6 (partitioning), §7 (width → target size), §8 (offset base), §9 (resolution), with §16 (why a tree) and §17 (Sliding Bisection) carried over.
6. **Experiments/harness.** No harness code changes required to *run* v0.2: same binary name, same flags. New arm IDs (e.g. `a1i_v2_t400`) will be added when the minimal experiment (§17) runs.
7. **Backward compatibility.** There is no external consumer of v0.1.0 scopes (no persisted indices — statelessness); the only "compatibility" surface is human/agent expectations about scope size, addressed by §2's evidence (larger scopes are the *fix*).

---

## 16. Open questions — final dispositions (v0.2.0 complete, 2026-07-20)

All items closed; **no further geometry/offset/agent sweeps are planned**.
Final values: `partition_lines = 400`, `offset_ratio = 0`.

* **O1 — default `t`. CLOSED: 400.** 6-C (t=400 vs t=600 live, 12 runs): in the test repo the layouts are identical for every source file (max 453L < 600), and the live arms were behaviorally indistinguishable (identical scopes; grade variance is the 4B format lottery, flipping in both directions). t=1000 covered by the deterministic probe (differing layout only in `docs/SPEC.md`, `package-lock.json` — outside any GT/read path). Smaller t = smaller read units = less eviction pressure; 400 is the phase-4 baseline-equivalence point.
* **O2 — rounding rule.** Resolved by implementation (Phase 6): **half-up** as proposed; the I5 band `[3t/4, 3t/2)` holds in the property suite (tree ≡ arithmetic on a 200,000-case grid).
* **O3 — prefix stability.** Resolved by implementation (Phase 6): the adopted split uses proportional boundaries `b_i = floor(n·i/k)` (§6.1) — sizes differ by ≤ 1, no structurally small last leaf. The S4 prefix-stable alternative (multiples-of-`t` boundaries, §6.3) was rejected as the primary scheme; no cross-call boundary-drift concern fired in any experiment.
* **O4 — `offset_ratio` value. CLOSED: 0.** Phase 7 (`RESULTS-SCOPEFOLIO-V0.2-P7.md`): r ∈ {0, 0.05, 0.10} × Q8×3 (Q10×3 control), t=400. All residual rereads are class A (same scope + same line — eviction refetches); the deterministic probe shows single-leaf files return byte-identical content at every r (offset clamps to file bounds); class-A rereads unchanged across r, maxSame ≤ 3, C ≈ 0, mild token cost. **Verdict NO-GO: the residual is geometry-independent and not solved by overlap** — it is context eviction, a harness-layer concern, not a partition-geometry one.
* **O5 — tree vs flat array (implementation).** Resolved by implementation (Phase 6): the canonical tree is the resolution path (per the design, §7); the closed-form boundary/leaf_index are the reference oracle in the equivalence property test — the safest reading of the design, validated by the 11,532-call harness cross-check.
* **O6 — scope-size ceiling.** Kept as a *monitoring note only* (not an open decision): no ceiling proposed; no token-budget regression appeared at t ≤ 400 (Phase 5 F4) or r ≤ 0.10 (Phase 7, +8/+16% tokens is variance/mild, direction noted).
* **O7 — parameter rename/alias.** Resolved: `partition_lines` kept (harness/spec stability, §15.1); no alias introduced.

---

## 17. Minimal validation experiments (proposed next steps)

Ordered so that the geometry can be validated **before any agent run**, and the first agent run is the smallest discriminating one.

### M0 — Paper property matrix (no code, no LLM, pre-implementation)

Hand-compute (or one-shot script, not the tool) the table for
`n ∈ {1, 138, 168, 265, 300, 400, 401, 453, 500, 599, 600, 700, 800, 801, 1000, 1193, 1200, 2000, 10000}` × `t ∈ {50, 200, 400, 600, 1000}`:

columns: `k`, leaf sizes, min/max leaf. Assert I1–I7 row by row.
Expected discriminating rows: 453/400 → k=1; 600/400 → k=2 (300/300); 1000/400 → k=3 (333/333/334); 1200/400 → 400×3; 453/200 → k=2 (226/227 — *still two slices*, pinning the default-`t` lower bound at 303).

### M1 — Offline replay of phase-4 trajectories (no LLM, pre-implementation)

Recompute paging metrics from the *existing* `harness/results/scopefolio-results.jsonl` + trajectories: for every recorded ScopeFolio call in `a1i_f400`/`a1t_f400` (file, line), re-derive the returned leaf under the v0.2 layout (t=400).
Pre-registered assertion: **`maxDistinctOffsets(extensions/index.ts)` drops from 2 → 1 in every f400 run**, and distinct-offset counts are unchanged (already 1) for `context.ts`/`agent.ts`/`utils.ts`. This measures the geometry fix's *coverage* effect (slices that would have been requested) without a single LLM call.

### M2 — Post-implementation agent mini-matrix (smallest live experiment)

After a v0.2 reference implementation exists (separate decision; not in this document):
`a1i v0.2 t=400 × {Q8, Q10} × 3 reps = 6 runs`, compared against the existing phase-4 `a1i_f400` (v0.1.0) records. Optional discriminator: `a1i v0.2 t=200 × Q8 × 3 = 3 runs` (prediction: two-slice behavior persists on index.ts, since 453 ≥ 3·200/2 — isolates the window-size axis from the geometry axis). Total ≤ 9 runs, ~30–60 min.
Pre-registered success criterion (proposed): Q8 dupRead ≤ baseline Q8 dupRead and `maxSameFileReads(index.ts) ≤ 3` across all reps, with correctness ≥ phase-4 f400.

M0 + M1 validate the geometry's *correctness and coverage* deterministically and cheaply; only after they pass should M2 spend model time.

---

*This design was implemented and validated end-to-end (formal spec:
`SPEC.md` v0.2.0; evidence: `EXPERIMENTS-ARCHIVE.md`). This document is
the frozen design record of v0.2.*
