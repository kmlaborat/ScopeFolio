# ScopeFolio Specification

**Version 0.2.0 (final)** — 2026-08-24

Supersedes the v0.1.0 draft. The design record is `SPEC-V0.2-DESIGN.md`;
the experimental evidence that locked every parameter decision is
`EXPERIMENTS-ARCHIVE.md` (frozen). All formulas below are the *verified*
formulas: they are what the reference implementation, the independent
reference (M0 paper matrix), the property tests, and the 11,532-call
cross-validation agree on.

---

## 1. Overview

ScopeFolio is a deterministic file-reading tool that exposes a requested
line's local scope: the even-split partition leaf containing that line,
rendered as a plain text range.

It is designed for coding agents that can reliably *locate* relevant lines
using deterministic tools such as `grep`, but may struggle to translate
those line numbers into an appropriate `Read` range.

ScopeFolio therefore separates:

* **location (WHERE)** — performed by the agent using search tools
* **scope construction (HOW MUCH)** — performed deterministically by ScopeFolio
* **file reading (WHAT)** — performed by ScopeFolio

The agent only needs to provide a target line.

```text
grep
  │
  │ line = 597
  ▼
ScopeFolio
  │
  ├─ derive partition geometry (from the line count)
  ├─ locate the leaf containing line 597
  ├─ apply offset, clamp to file bounds
  └─ return the scope, byte-for-byte
```

The partition structure is an implementation detail and is never exposed
to the agent.

> **The agent specifies WHERE. ScopeFolio determines HOW MUCH and returns WHAT.**

---

## 2. Version: what changed in v0.2.0

The agent-facing API is **unchanged** (§15). What changed is the meaning
of one parameter and the geometry it drives:

| Aspect | v0.1.0 | v0.2.0 |
|--------|--------|--------|
| `partition_lines` (t) | maximum leaf width under recursive halving | **target leaf size** |
| geometry | binary halving (power-of-two-ish leaves; "small" files split in two) | **balanced even split** (§8) |
| small files | halved anyway (e.g. 453 lines → 227+226) | **single leaf below `n < 3t/2`** (453 → whole file) |
| default `t` | 50 | **400** |
| offset | `round(r × actual leaf width)` | **`floor(r × t)`** — against the target, not the leaf (§10) |
| tree | binary halving tree, implementation detail | **canonical even-split tree; resolution walks it** (§9) |
| validation | unit tests | unit + property tests + independent-reference cross-validation |

Why: experiments Phases 2–7 (see `EXPERIMENTS-ARCHIVE.md`). The v0.1
default geometry made agents page 50-line slices (turn-budget exhaustion
on two models, four conditions); leaf *size* was the causal variable
(Phase 4); the halving *rule* left a boundary pin even at `t=400`
(Phase 4/5); the even-split rule removes it (Phase 5/6); overlap was
tested and rejected as a fix for the residual (Phase 7).

---

## 3. Normative requirements (MUST)

ScopeFolio MUST:

1. interpret `partition_lines` as a **target leaf size**, not a maximum;
2. compute the leaf count as **`k = max(1, round_half_up(n / t))`**
   (integer form: `k = max(1, (2n + t) / (2t))`);
3. produce a **balanced even split**: every leaf is `⌊n/k⌋` or
   `⌈n/k⌉` lines — sizes differ by at most one;
4. return **a single leaf for `n < 3t/2`** (the whole file);
5. compute the offset as **`o = floor(r × t)`** — against the *target*
   size, never the actual leaf size — and clamp the expanded range to the
   file boundaries;
6. use defaults **`t = 400`** and **`r = 0`**;
7. be **deterministic**: same file contents, target line, `t`, `r`
   ⇒ identical output range and rendering; no LLM inference,
   randomness, or external retrieval in scope resolution;
8. be **stateless**: every invocation re-derives everything from the
   current file; no persistent indexes, caches, hashes, or session state
   (§13);
9. be **content-preserving**: returned content is byte-for-byte the
   selected source lines; rendering adds range identification and line
   numbers as metadata only (§14);
10. keep the **agent-facing API unchanged** from v0.1.0: the agent asks
    for `(file, line)` plus optional configuration, and never for
    partition structure (§15).

---

## 4. Design goals

ScopeFolio MUST:

1. provide deterministic access to the region around a requested line;
2. accept a single target line as the primary navigation primitive;
3. construct its partition structure from the file on demand;
4. maintain no persistent state;
5. require no file hash or identity mechanism;
6. hide partition-tree navigation from the agent;
7. support a configurable **target leaf size**;
8. support configurable contextual overlap around the selected leaf;
9. work independently of `grep`, `glob`, or any particular agent framework;
10. return ordinary text suitable for direct insertion into an LLM context.

ScopeFolio SHOULD:

* minimize unnecessary file content returned to the model;
* preserve useful surrounding context around the target line;
* make repeated reads of a location cheap and *identical* (same scope
  identity ⇒ same bytes), so re-fetching is a deterministic no-op
  semantically even when the harness must pay for it;
* remain simple enough to implement as a small standalone tool.

---

## 5. Responsibility boundary: ScopeFolio does NOT solve context eviction

This is a deliberate, experiment-locked boundary — not a placeholder
non-goal.

ScopeFolio answers two questions:

* **WHERE → deterministic file region**: which contiguous region of the
  file the target line belongs to (pure arithmetic on the line count);
* **WHAT → rendered scope**: the byte-exact content of that region.

It does **not** answer whether that content *stays in the agent's
context*. That belongs to the harness/runtime:

```text
ScopeFolio
    WHERE
      ↓
    deterministic file region
      ↓
    WHAT
      ↓
    rendered scope

Harness / Runtime
    context capacity
      ↓
    eviction
      ↓
    reread / retention policy
```

**Evidence** (Phase 4 → 7, `EXPERIMENTS-ARCHIVE.md`):

* Once leaf geometry was fixed (leaf ≈ natural read unit, small files
  single-leaf), agents stopped paging entirely (class C ≈ 0) — the
  geometry problem was solved inside ScopeFolio.
* The *residual* rereads were 100% **class A: same scope AND same line**
  re-requested — the signature of content evicted from the model's
  context window (64 KiB tool-result cap in the test harness) being
  re-fetched, not of any geometry defect.
* The preregistered offset sweep (Phase 7) proved this residual is
  *geometry-independent*: for single-leaf files the offset expansion
  clamps to the file bounds and the rendered scope is byte-identical at
  every `r`; the sweep showed no reduction in class-A rereads. **NO-GO.**

**Consequences (normative):**

* ScopeFolio guarantees that a re-request of an evicted scope is
  **deterministic and cheap to produce** (identical scope identity,
  identical bytes). It does not guarantee that the harness will not
  evict the content, and no setting of `t` or `r` can.
* Mitigating eviction (scope pinning, re-read budgets, tool-result
  retention) is the responsibility of the **harness/runtime context
  layer** and is out of ScopeFolio's scope.
* `offset_ratio` is therefore not a retention mechanism; its default is
  `0` (§10).

---

## 6. Non-goals

ScopeFolio does NOT:

* provide semantic, fuzzy, or embedding-based retrieval;
* maintain a persistent partition index or any cache;
* identify files by hash or require content identity;
* expose partition IDs, tree structure, or boundary arithmetic to agents;
* decide which line the agent should search for;
* replace `grep`/`glob` or unrestricted file reading;
* optimize `partition_lines` or `offset_ratio` automatically;
* **retain content in the agent's context** — eviction, pinning, and
  re-read budgeting are harness/runtime concerns (§5).

---

## 7. Terminology

* **Target line** — the line supplied by the caller (`line`, 1-based).
  The agent's primary navigation coordinate.
* **Target leaf size** — `partition_lines` (`t`). The *target* size of
  each leaf; a target, not a hard maximum (§8).
* **Leaf partition** — a contiguous region of the file produced by the
  even split. Leaf `j` (0-based) is the line interval
  `[b(j) + 1, b(j+1)]` (§8).
* **Scope** — the selected leaf, optionally expanded by the offset and
  clamped to the file. A scope is identified by the triple
  `(file, start_line, end_line)`; identical triples are identical scopes
  (same bytes, deterministically).
* **Offset** — contextual expansion `o = floor(r × t)` around the
  selected leaf, clamped to file boundaries (§10).
* **Partition tree** — the canonical binary tree whose leaves are exactly
  the even-split intervals (§9). An internal implementation detail.

---

## 8. Partition geometry (normative)

Given a file of `n` lines (`n ≥ 1`) and target leaf size `t ≥ 1`:

### 8.1 Leaf count

```text
k = max(1, round_half_up(n / t))
```

integer form (the one implemented; exact, no floats):

```text
k = max(1, (2·n + t) / (2·t))        [integer division]
```

Ties round **up** (`n/t = m + 1/2` ⇒ `k = m + 1`).

### 8.2 Balanced even split — boundary prefixes

```text
b(0) = 0
b(i) = ⌊ n·i / k ⌋        i = 1 … k
b(k) = n
```

Leaf `j` (0-based, `j < k`) covers lines:

```text
leaf j = [ b(j) + 1 ,  b(j+1) ]
```

All leaf sizes are `⌊n/k⌋` or `⌈n/k⌉` — **they differ by at most one**.

Examples (`t = 400`):

| n  | k | leaves |
|----|---|--------|
| 453 | 1 | `[1–453]` (453 < 600 ⇒ whole file) |
| 599 | 1 | `[1–599]` |
| 600 | 2 | `[1–300] [301–600]` (smallest 2-leaf case: 600 = 3t/2) |
| 800 | 2 | `[1–400] [401–800]` |
| 1000 | 3 | `[1–333] [334–666] [667–1000]` (333+333+334) |
| 1200 | 3 | `[1–400] [401–800] [801–1200]` |

The geometry depends on `(n, t)` **only** — never on file content.

### 8.3 Invariants

For all `n ≥ 1`, `t ≥ 1`:

| # | Invariant |
|---|-----------|
| I1 | **Complete partition**: `b(0) = 0`, `b(k) = n`; leaves are contiguous, in order, exact — no gaps, no overlap. |
| I2 | **Leaf size band**: every leaf is in `[3t/4, 3t/2)` lines — with one exception: |
| I3 | **Small-file rule**: if `n < 3t/2` then `k = 1`, and the single leaf has `n` lines (band waived). |
| I4 | **Balanced even split**: every leaf is `⌊n/k⌋` or `⌈n/k⌉` (differ by ≤ 1). |
| I5 | **Leaf count**: `k = max(1, round_half_up(n/t))` (ties up). |
| I6 | **Pure & deterministic**: `k`, every `b(i)`, and every leaf interval are functions of `(n, t)` only; two constructions for the same `(n, t)` are identical. |
| I7 | **Monotone**: `k(n, t)` is non-decreasing in `n`. |

The single-leaf threshold is a direct consequence of the round-half-up
rule in §8.1, not an independent partition rule.

These invariants are enforced by the test suite over a 200,000-case
`(n, t, k, i)` grid (§20).

---

## 9. Canonical partition tree (normative)

The even-split leaves are wrapped in a **canonical binary tree**:

* `k ≤ 1`: the file is a single node `[1, n]`.
* A node covering leaf indices `[lo, hi]` (inclusive) spans lines
  `[b(lo) + 1, b(hi + 1)]`.
* An internal node splits at `m = ⌊(lo + hi) / 2⌋`: left child
  `[lo, m]`, right child `[m + 1, hi]`.
* Lookup: walk from the root; descend left iff `line ≤ left.end`.

The arithmetic lookup is the **reference oracle**:

```text
leaf index j = ⌈ line·k / n ⌉ − 1
```

**Property (normative): tree lookup ≡ arithmetic lookup for every line of
every file** — enforced exhaustively by a property test
(`tests/geometry.rs`).

The tree is required even though the geometry is pure arithmetic: the
canonical shape is the foundation on which a future k-ary / adaptive tree
can be dropped in as the resolution mechanism (design decision O5). The
tree MUST remain an internal implementation detail and MUST NOT be
exposed as part of the public interface.

---

## 10. Offset

The public parameter is:

```text
offset_ratio r        (default 0; valid: finite, r ≥ 0; intended range [0, 1])
```

The expansion is computed against the **target** size, not the actual
leaf size:

```text
o = ⌊ r × t ⌋
selected scope = [ max(1, leaf.start − o),  min(n, leaf.end + o) ]
```

Notes:

* Computing against `t` (fixed) rather than the leaf's actual size makes
  the offset a function of `(t, r)` only — stable across leaves and
  files, and testable without constructing any file.
* **Single-leaf files are invariant under `r`**: the expansion clamps to
  the file bounds, so the rendered scope is byte-identical at every
  `r`. (Structurally verified across the test workdir in Phase 7:
  118/122 probe points identical; only multi-leaf file boundaries
  differ.)
* For multi-leaf files, the expansion may pull in lines from
  neighbouring leaves (clamped at the file bounds).
* `r` is **not** a retention mechanism (§5): Phase 7's preregistered
  sweep found that overlap does not reduce eviction-driven rereads and
  pays a mild token cost. The default `r = 0` is final.

---

## 11. Target-line resolution

The primary operation is:

```text
read(file, line, [t], [r])  →  scope
```

Algorithm:

1. Open the target file (UTF-8 text; invalid encoding is a deterministic
   error).
2. Determine its line count `n`.
3. Compute `k = max(1, round_half_up(n/t))` and the boundary prefixes
   `b(i)`.
4. Construct the canonical tree and walk it to the leaf containing
   `line`.
5. Expand by `o = floor(r × t)`; clamp to `[1, n]`.
6. Extract the selected lines byte-for-byte; identify the scope
   `(file, start_line, end_line)`.

The target line MUST be contained in the returned scope. If the requested
line is outside `[1, n]`, ScopeFolio MUST return a deterministic error.

---

## 12. Determinism

For the same file contents, target line, `partition_lines`, and
`offset_ratio`, ScopeFolio MUST produce the identical output range and
identical rendered bytes. No LLM inference, randomness, or external
retrieval is permitted during scope resolution.

---

## 13. Statelessness

ScopeFolio MUST be stateless. Each invocation independently derives its
partition structure from the current file. It MUST NOT require
persistent indexes, databases, cache files, partition metadata, file
hashes, or session identifiers.

ScopeFolio is a **computed view**, not an index. After the operation
completes, no ScopeFolio state is required to remain.

---

## 14. Content preservation

The returned content MUST be byte-for-byte the selected source lines.
ScopeFolio MUST NOT modify whitespace, indentation, line endings,
encoding, or source text.

The rendered output SHOULD clearly identify the returned scope
(file and line range) and present lines with their numbers, e.g.:

```text
src/extensions/index.ts:1-453
1   | ...
42  | ...
```

Line numbering and the range header are presentation metadata and MUST
NOT alter the underlying content.

---

## 15. Agent-facing interface (unchanged from v0.1.0)

The agent-facing API signature is **unchanged** from v0.1.0:

```bash
scopefolio read --file <path> --line <n>
                [--partition-lines <t>]   # default 400
                [--offset-ratio <r>]      # default 0
```

Library:

```rust
read(file_path: &str, line: usize,
     partition_lines: usize, offset_ratio: f64)
    -> Result<ReadResult { start_line, end_line, content }, ScopeFolioError>
```

The agent never sees `k`, boundary arithmetic, partition IDs, or tree
structure. It does not need to reason about partition boundaries — a
boundary-adjacent line simply resolves to whichever leaf contains it,
deterministically.

The only semantic change in v0.2.0 is the **meaning** of
`partition_lines`: *maximum leaf width under halving* (v0.1) →
*target leaf size* (v0.2). The name was kept deliberately for
spec/harness stability (design decision O7).

> **The agent specifies WHERE. ScopeFolio determines HOW MUCH and returns WHAT.**

---

## 16. Relationship to Grep and Glob

ScopeFolio does not replace deterministic search tools. The intended
workflow:

```text
Glob  → identify files
Grep  → identify relevant lines
ScopeFolio → retrieve the appropriate local scope
Agent → reason about the retrieved content
```

This addresses the observed failure of small coding agents — after
`Grep` identifies a line, the agent must decide how much to `Read`, and
produces oversized / repeated / wandering reads. ScopeFolio moves that
second decision from the model into deterministic infrastructure.

---

## 17. Configuration and defaults

| Parameter         | Default | Rationale |
| ----------------- | ------: | ---------|
| `partition_lines` |    400  | the natural read unit of the tested agents (baseline median Read ≈ 300+ lines); the point of parity with the phase-4 baseline; the smallest `t` that single-leafs the test workload (least eviction pressure). Larger `t` (600/1000) showed no benefit (Phase 6-C). |
| `offset_ratio`    |     0   | overlap does not mitigate the residual (eviction-driven) reread and costs tokens (Phase 7, NO-GO). |

Validation: `partition_lines ≥ 1` (0 is a deterministic error);
`offset_ratio` finite and `≥ 0` (intended range `[0, 1]`).

---

## 18. Error handling

ScopeFolio MUST return deterministic errors for:

* file not found;
* unreadable file;
* invalid (non-UTF-8) content;
* invalid line number (`line < 1` or `line > n`);
* invalid `partition_lines` (`0`);
* invalid `offset_ratio` (non-finite or negative).

It MUST NOT silently substitute another file or another line.

---

## 19. Implementation constraints

The reference implementation MUST/SHOULD:

* be implemented in Rust;
* have no external service dependencies, no database, no hash index,
  no LLM dependency;
* have no persistent state;
* compute the **partition geometry** (`k`, `b(i)`, leaf sizes) with
  **integer arithmetic only** — no floating point in the partition math —
  and derive the integer offset `o` deterministically from the public
  `offset_ratio` (§10);
* expose a minimal CLI/library API;
* favor correctness and simplicity over premature optimization.

---

## 20. Testing

Tests MUST cover at least:

### Geometry invariants
* I1–I7 over a broad `(n, t)` grid (200,000 cases);
* the discriminating cases: `n=453, t=400` → single leaf (v0.1 halved
  this); `n=599, t=400` → single; `n=600, t=400` → 300+300;
  `n=1000, t=400` → 333+333+334; `n=453, t=200` → 226+227.

### Tree
* tree leaves equal the arithmetic leaves, in order, for all `n × t`;
* **property: tree lookup ≡ arithmetic lookup for every line of the
  file** (exhaustive, including a 10,000-line file);
* two constructions for the same `(n, t)` are identical (I6).

### Line resolution
* first line; last line; leaf boundary; line immediately before/after
  each boundary; middle of a leaf.

### Offset
* `r = 0`; `0 < r < 1`; clamping at file beginning/end;
* single-leaf files byte-identical at every `r`;
* `o = floor(r·t)` (ties down — e.g. `t=400, r=0.1` ⇒ 40).

### Determinism & content
* same inputs ⇒ identical ranges and bytes;
* returned content exactly equals the selected source lines.

### Cross-validation
* the implementation agrees with an independently written reference on
  every `(file, line, t, r)` combination in a replay corpus
  (historically: 11,532 calls, 0 mismatches).

---

## 21. Future extensions (deferred)

Intentionally deferred; each MUST keep complexity inside ScopeFolio
rather than transferring it to the agent:

* **Adaptive partition width** (syntax/structure/model-capacity driven);
* **Semantic partitioning** (module/function/class scopes);
* **Sliding bisection** (search/relevance signal beyond an explicit line);
* **RAG integration** (embedding-guided partition selection);
* **Multi-line queries** (minimal enclosing scope for several lines);
* **k-ary / adaptive canonical trees** (drop-in per §9).

**Monitoring note (O6, non-gating):** no scope-size ceiling is defined.
`1200` lines at `t=400, r=0` is a 1.2 K-line scope; large `t` + `r`
could grow large. Revisit (a ceiling, or adaptive `t`) only if token
regression is observed in a real agent workload. Nothing in v0.2
evidence (t ≤ 400, r ≤ 0.10) triggers this.

**Explicitly out of scope (harness-side):** context retention features
(pinning frequently re-read scopes, re-read budgets, tool-result
promotion to the system prompt). These address the eviction residual of
§5 and belong to an independent harness project — not to ScopeFolio.

---

## 22. Design principles

1. **The agent specifies WHERE. ScopeFolio determines HOW MUCH.** The
   agent's single input is a target line; scope construction is
   deterministic infrastructure.
2. **ScopeFolio determines WHAT is read; the harness determines what is
   retained.** Scope resolution ends at the rendered scope; context
   lifecycle is the harness/runtime's responsibility (§5).
3. **AnchorScope is the write-side counterpart**: ScopeFolio provides
   the deterministic read context in which edits are located and
   verified.
4. **Rationale, not guesswork**: every locked parameter (`t`, `r`, the
   split rule, the tree's canonical role) is backed by the frozen
   experimental record (`EXPERIMENTS-ARCHIVE.md`).

