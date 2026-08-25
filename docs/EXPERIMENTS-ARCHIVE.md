# ScopeFolio — experiment archive: design rationale (frozen 2026-08-24)

**Status: FROZEN.** Phases 1–7 of the ScopeFolio experiment program are closed.
No further geometry sweeps, offset sweeps, or agent experiments are planned.
This document records the program **as the rationale for the v0.2.0 design** —
not as a reproducibility recipe. The experimental harness, result JSONLs,
prompt captures, endpoint configuration (LLM server URLs/keys), and
throwaway scripts remain **local-only archive artifacts and are NOT part of
the implementation surface**: they are not to be ported into new
implementation environments. A new tree needs only `src/`, `tests/`, and `docs/`.

Provenance (git): `2b56819` P1 · `ec4a812` P2 · `133d6be` P3 · `a73b05b` P4 ·
`ffdffea` P5 · `c3e8f3e`/`bf0964d`/`d3fe3dc`/`87375d6` P6 · `2339c25` P7 ·
`221ef67` freeze. Local result reports: `harness/RESULTS-*.md` (see §4 note).

**The system under study.** ScopeFolio is a stateless, deterministic resolver:
`read(path, line) → scope` where a *scope* is the leaf of a file partition
that contains `line`, rendered as numbered lines. It exists because small
agents fail at raw text files with *oversized, wandering, duplicative reads*
(SPEC §1: "the agent's failure mode is context management, not parsing").
Everything below tests that premise and pins the parameter choices.

---

## 1. The causal chain (Phases 3–7)

```text
pl=50 (v0.1.0 default)
   │  P2/P3: the 50-line window is a CONSTRAINT, not a stabilizer.
   │  Agents page: dupRead ×6 (1.9→11.2), maxTurns 25%/8%→100%, NOANSWER-heavy.
   ▼
paging pathology (established on 2 models × 2 modes, P2+P3)
   │  P4: sweep pl=50→200→400, binary untouched (CLI param only).
   │  Every paging/turn metric falls monotonically toward baseline.
   ▼
geometry improvement (P4: pl=400 ≈ baseline; the pathology was the slice size)
   │  But v0.1.0 halving still split the 453-line target into two ~227-line
   │  slices at pl=400, and agents pinned at the seam (line 226 → always the
   │  first slice) re-reading it 8–10× (class-A boundary pin, f400 replays).
   │  P5: redesign to v0.2 even-split geometry (n < 3t/2 ⇒ ONE leaf);
   │  offline replay of the f400 trajectories: pin disappears, refetch 8–10→2,
   │  zero boundary reads, zero errors. GO.
   ▼
v0.2 geometry (P6: implemented, property-gated; M2 live GO — dupRead 4.3 vs
   6.9, maxSameFileReads 2.0 ≤ 3, pagedFiles 4.0 vs 6.3, correctness 2/3 vs 1/3)
   │  Residual: agents still re-read — but now 100% class A (same scope AND
   │  same line, always L1). No paging, no line probing. B=0, C≈0.
   │  P7: preregistered offset sweep r ∈ {0, 0.05, 0.10}. Structural probe:
   │  offset clamps to file bounds ⇒ single-leaf files byte-identical at all r.
   │  Result: class-A rereads unchanged (4.3→5.7→6.3 = model variance),
   │  maxSame ≤ 3, C ≈ 0, mild token cost. NO-GO.
   ▼
O4 offset sweep → NO-GO (offset cannot fix what the offset cannot touch)
   ▼
remaining reread = context eviction (harness concern, out of ScopeFolio scope)
```

Reading: the program first *localizes* the failure (P2/P3: slice size, not
capability), then *fixes* it at two levels of the geometry (P4: leaf size;
P5/P6: split rule), then *falsifies* the last candidate fix inside the tool
(P7: overlap), which relocates the residual to where it actually lives — the
agent harness's context management, not the read resolver.

---

## 2. Phase summaries

### Phase 1 — passive adoption (LFM-2.6B, v0.1.0, pl=50; 30 runs)

* **Purpose.** Does an agent voluntarily adopt a passively offered
  `ScopeFolio` tool, and does usage stabilize its reads?
* **Conditions.** Baseline vs ScopeFolio (named tool + prompt rule),
  4 queries + hallucination trap × 3 reps, maxTurns 15.
* **Results.** ScopeFolio called **0 times in 15/15** treatment runs; the
  arms are statistically indistinguishable (correctness 6 vs 5/15, NOANSWER
  9 vs 10).
* **Verdict.** Hypothesis *unanswered* (the primitive was never exercised),
  but a clean finding: a 2.6 B model does not reach for a fourth tool even
  when the ideal trigger (Grep → concrete line) occurs on every query.
* **Rejected hypothesis.** "Named tool + prompt rule ⇒ adoption" (0/15).
* **Design reflection.** Adoption is not the primitive's concern; test the
  semantics by *forcing* (Phase 2). The read-side instability the tool
  targets (duplicate/wandering reads, "Grep found it, Read got lost") is real
  and present in *both* arms.

### Phase 2 — forced-Read capability (LFM-2.6B, pl=50/off=0; 12 runs)

* **Purpose.** Isolate scope-retrieval semantics from tool adoption: harness
  rewrites every `Read(path, offset)` into a ScopeFolio-backed read of
  `(path, line=offset)`; agent unaware.
* **Results.** The **mechanism was flawless**: 87 scope-backed reads, 1
  benign backend error (offset past EOF, surfaced in Read's own wording),
  0 path escapes, 0 format confusion; scopes exactly per v0.1.0 semantics
  (median 2 K, max 3 K). Behavior: correctness unchanged (C+P 5/12 =
  baseline), but dupRead 1.8→4.8, avg turns 11.8→15.3, **maxTurns 33%→83%**;
  tokens flat (+5.2%) — the model read the same material in more, smaller
  pieces. Two runs never read anything (localization failures).
* **Verdict.** NOT SUPPORTED at this geometry.
* **Finding.** "A 50-line scope is a constraint, not a stabilizer." The
  scarce resource is **turns**, and paging spends them; class-E death
  mid-paging appeared for the first time.
* **Rejected hypotheses.** "Small bounded scopes stabilize exploration" (at
  pl=50). "Localization (Grep/Glob) is the bottleneck" — it is not; `fr=0`
  runs were identical to baseline wandering.
* **Design reflection.** The primitive's effect is confounded with its
  geometry at the tested point → sweep geometry next (Phase 4); offset
  tuning stays deferred (nothing to tune at a failing geometry).

### Phase 3 — stronger agent + Thinking/Instruct (A1-4B, pl=50/off=0; 48+8 runs)

* **Purpose.** Is the Phase-2 failure model-specific? Disentangle four
  factors: primitive, agent capability, thinking mode, geometry.
* **Results.** Forced pl=50: **maxTurns 100% in both conditions**, dupRead
  1.9→11.2 (Instruct) / 1.6→9.8 (Thinking), NOANSWER 8–10/12; all five
  pre-registered criteria FAIL in both conditions. Meanwhile
  `a1t_base` (Thinking, normal Read) is the **best arm in the whole
  program**: 11/12 CORRECT, dupExpl 0.2, maxTurns 8% — it already does the
  "Grep → few targeted Reads → answer" flow. Token growth under thinking is
  **prefill-dominated** (~99% of tokens are prompt/history; thinking adds
  ~1.5 K completion tokens, +1.3%) — its benefit is navigation, not tokens.
  Adoption probe: 0/8 voluntary calls, but the competent model simply
  ignores the tool and still performs.
* **Verdict.** NOT SUPPORTED (both conditions).
* **Findings.** (1) The 50-line failure signature is identical across
  2.6 B/4 B and Instruct/Thinking; **the better the navigator, the more it
  pages** — magnitude tracks competence. (2) 50 lines ≈ 1/6–1/8 of the
  natural read unit (baseline median 17–20 K ≈ 300+ lines).
* **Rejected hypotheses.** "The failure is an artifact of the weak model."
  "offset_ratio is the first knob to turn." (At off=0 the tool already
  fails; there is no effect to tune.)
* **Design reflection.** The open question is **geometry, not capability**.
  Next: the forced variant at the natural window size implied by the data
  (baseline median Read ⇒ ~200–400 lines), CLI parameter only.

### Phase 4 — partition geometry sweep (A1-4B, pl = 50/200/400; 48 new runs)

* **Purpose.** Is the paging pathology geometric? ScopeFolio binary
  untouched; only `--partition-lines` varies. Target files: 453/265/168/138
  lines → pl=400 makes three of them single leaves, and `index.ts` two
  ~227-line leaves (v0.1.0 halving).
* **Results.** Strictly **monotone toward baseline** on every paging/turn
  metric, in both conditions:
  dupRead (Instruct) 11.2 → 7.0 → 2.4; maxSameFileReads 10.7 → 7.8 → 2.9;
  maxTurns 100% → 75% → 25% (Thinking: 9.8→6.4→3.8; 100%→58%→25%).
  At pl=400 Instruct is statistically close to baseline: C+P 9/12 vs 8/12,
  final answer 75% vs 67%, tokens **−11%** — the cleanest win in the program.
  Scope sizes (12–14 K median) become the same order of magnitude as normal
  reads.
* **Verdict.** **The paging pathology was the slice size.** pl=200 is a
  dead intermediate (still 75%/58% maxTurns, above-baseline tokens).
* **Finding (residual).** At pl=400 the residual is *not* paging:
  `uniqueOffsets ≈ 2` with 3–13 reads of the *same two slices* — a mild
  class-A loop plausibly fed by 64 KiB tool-result eviction. And v0.1.0's
  halving still splits the 453-line target at pl=400 — the split *rule*
  itself has a structural defect (later pinned down as the boundary pin).
* **Rejected hypothesis.** "pl=200 suffices."
* **Design reflection.** pl=400 becomes the reference point and the v0.2
  default candidate; the next defect to remove is the *split rule*, which
  must make `n < 3t/2` a single scope (SPEC-V0.2-DESIGN §6).

### Phase 5 — v0.2 geometry pre-validation (no LLM; M0 + M1)

* **Purpose.** Validate the redesigned geometry on paper and offline before
  any implementation or live run (validation-before-agent discipline).
* **M0 (paper property matrix).** Invariants I1–I7 (complete partition,
  leaf band `[3t/4, 3t/2)`, monotone boundaries, single-leaf below 3t/2,
  round-half-up count, stability, offset clamp) hold across the full
  n × t grid. **PASS.**
* **M1 (offline replay of the f400 trajectories).** Re-resolve every
  phase-4 call under v0.2: `index.ts` 453L/t=400 → one leaf [1–453]
  (was [1–226]+[227–453]); the boundary-pin calls (line 226 → first slice,
  re-fetched 8×/10×) now re-resolve to the *same* scope → refetch 8–10 → 2
  with **zero boundary reads** and **zero errors**; all other files
  byte-identical.
* **Verdict.** GO — the even-split rule removes both the halving artifact
  (odd splits, mid-file seams) and the pin.
* **Design reflection.** Implementation (Phase 6) is behaviorally safe;
  any live regression would indict the validation, not the geometry.

### Phase 6 — v0.2 implementation + live validation + target sweep

* **Purpose.** Implement the design; prove it live against the f400
  baseline; reconfirm the default target size (O1).
* **Implementation.** Canonical tree as the resolution path; closed-form
  `boundary(n,k,lo)` as the *reference oracle*; equivalence enforced by a
  property test over a 200,000-case (n,t,k,i) grid plus a 11,532-call
  independent cross-validation harness (0 mismatches). 80-test suite.
* **M2 (live, t=400, Instruct, Q8 primary × 3, Q10 control × 3).**
  vs phase-4 f400: dupRead 6.9 → **4.3**, maxSameFileReads 3.0 → **2.0**,
  pagedFiles 6.3 → **4.0**, correctness 1/3 → **2/3** (C3), tokens −6.6%.
  All four pre-registered gates PASS → **GO**. Q10 (control) dropped
  2/3→1/3 — root-caused as the 4B model's *format-compliance lottery*
  (NOANSWER from omitted/empty final answers, not read behavior); the
  geometry was identical.
* **6-C (O1: t-sweep).** Deterministic probe: only two non-source files
  change layout between t=400 and t=600; every source file is single-leaf at
  both. One live arm (t=600, Q8×3 + Q10×3): geometrically identical scopes;
  behaviorally indistinguishable (grades flip in *both* directions — lottery).
  t=1000 covered by the deterministic probe alone (its layout differences are
  confined to the same two non-read files). **O1 closed: t = 400.**
* **Rejected hypotheses.** "Larger targets (600/1000) improve behavior" —
  no signal at any level (layout, scopes, behavior).
* **Design reflection.** v0.2.0 frozen: even-split geometry, `t=400`
  default, tree canonical, offset base `floor(r·t)`, property-gated.

### Phase 7 — O4: offset_ratio sweep (t=400, r ∈ {0, 0.05, 0.10}; 9 new runs)

* **Purpose.** Preregistered single decision: does overlap expansion
  mitigate the residual reread (eviction-driven), or is it
  geometry-independent? (Explicitly: no optimal-r search.)
* **Structural determinant (no LLM).** v0.2 offsets are `floor(r·t)`
  clamped to file bounds; for a single-leaf file the expansion is a no-op by
  construction. Probe of all 61 workdir files at r ∈ {0, .05, .10}:
  **118/122 points byte-identical** (differences only in the two multi-leaf
  non-source files, boundary lines only). Every query-relevant file is
  single-leaf → the offset changes *nothing* the agent reads.
* **Trajectory re-parse.** All v0.2 rereads are **class A: same scope AND
  same line (always line 1)** — the signature of content evicted from
  context being re-requested. B (same scope, other line) = 0;
  C (paging) ≈ 0. The v0.1.0 f400 contrast shows what a *geometry* problem
  looks like: an 8–10× pin at the seam plus one genuine page.
* **Results.** Class-A rereads 4.3 → 5.7 → 6.3 (variance, content
  identical); maxSame ≤ 3; C ≈ 0; tokens +8%/+16%; correctness 2/3→1/3 on
  Q8 while the control Q10 flips the *other* way (1/3→2/3) on identical
  content — the 4B lottery again.
* **Verdict.** **NO-GO.** O4 closed: the residual reread is
  *geometry-independent and not solved by overlap*.
* **Rejected hypothesis.** "Boundary overlap mitigates eviction-driven
  rereads." (It cannot: eviction refetches identical content, and on
  single-leaf files the content is invariant under r.)
* **Design reflection.** `offset_ratio` final value **0**. The residual
  belongs to the agent harness's context layer (scope pinning / re-read
  budgeting) — not to the read resolver. ScopeFolio's role stays
  "deterministically peek at the needed file region."

---

## 3. Design decisions locked in by the experiments

| # | Decision | Locking evidence |
|---|----------|------------------|
| 1 | **Target-leaf-size semantics** — `partition_lines` is the *target* leaf size (round-half-up count, leaves in `[3t/4, 3t/2)`, single leaf below 3t/2), not v0.1.0's "max leaf width under halving" | P3: effect confounded with geometry at the tested point → P4: pl sweep monotone ⇒ leaf *size* is the causal variable → P5: I1–I7 hold on paper → P6: property suite (200 K-case grid, 11,532-call cross-check) |
| 2 | **Even-split geometry (S4)** — k = round(n/t); first k−1 leaves equal; last absorbs the remainder | P4: v0.1.0 halving still split the 453-line target at pl=400 → boundary pin (8–10× refetch at the seam, f400 trajectories) → P5 replay: pin gone, refetch 8–10→2, zero boundary reads → P6 M2: gates C1–C4 PASS |
| 3 | **Default `t = 400`** | P3: natural read unit ≈ 17–20 K ≈ 300+ lines → P4: pl=400 ≈ baseline (tokens −11%, the cleanest win) → P6/6-C: t=600 live indistinguishable, t=1000 deterministic no-op → keep 400 (smallest t that single-leafs the workload = least eviction pressure) |
| 4 | **`offset_ratio = 0`** | P7: residual rereads are class A on byte-identical single-leaf content (structural clamp probe: 118/122 points invariant) → sweep shows no reduction, mild token cost → offset cannot be the eviction fix |
| 5 | **Stateless deterministic resolver** — same `(file, line, params)` → same scope; no persisted state; scope identity = `(file, start_line, end_line)` | P2: mechanism flawless (87 scope reads, 1 benign error, 0 escapes, 0 format confusion) → P5: determinism is what makes offline replay (M1) and pre-validation possible at all → P6: cross-validated against an independent implementation, 11,532 calls, 0 mismatches |
| 6 | **Tree canonical** — the canonical tree is the resolution path; the closed-form boundary function is the reference oracle (not the implementation) | P6/O5: both satisfy the equivalence property; the tree-as-path reading matches the design's stated preference and keeps the arithmetic test oracle independent |
| 7 | **Eviction is out of ScopeFolio scope** — ScopeFolio "deterministically peeks at the needed file region"; keeping content *in context* (pinning, re-read budgets) is the agent harness's responsibility | P4: at pl=400 the residual is same-slice re-reads under 64 KiB tool-result eviction → P6: same signature survives the geometry fix → P7: offset cannot touch it (NO-GO) → residual relocated to the harness layer, not the resolver |

---

## 4. Honest limitations (read before citing numbers)

* **n is small.** 3 reps per (arm, query); 12 per arm in the A1 phases.
  No cell is statistically reliable; the strength of the program is the
  *monotone, multi-model, multi-mode replication of signatures* (e.g. the
  pl=50 failure on 2.6 B and 4 B, Instruct and Thinking), not significance.
* **One repository, two small models, one turn budget.** Conclusions are
  about this class of agent/workload; a different repo (many large files)
  or a larger context window could shift the optimal t (watch-item O6).
* **The 4B model is a format-compliance lottery.** NOANSWER↔CORRECT flips on
  *byte-identical* inputs across arms (Q10 flipped both ways in 6-C/P7).
  Grade differences between geometrically identical arms are variance, not
  effect — this is why the v0.2 gates weight read-behavior metrics
  (dupRead, maxSameFileReads, pagedFiles) as primary and correctness as
  non-inferiority.
* **The committed reports** (`harness/RESULTS-*.md`) contain per-run tables
  and reproducibility recipes tied to the (retired) LLM endpoints. They are
  local archive; the *conclusions* above are what travel with the design.
  Endpoints, harness code, result JSONLs, and throwaway scripts are **not
  to be ported** into new implementation trees.

## 5. Open items after the freeze

None gating. `O6` (scope-size ceiling for large t + r) remains a *monitoring
note only*: no ceiling is imposed; revisit if token-budget regressions
appear on workloads with many multi-leaf files. All of O1/O2/O3/O4/O5/O7 are
closed (dispositions in SPEC-V0.2-DESIGN §16).

**The program's one-line answer:** the agent's read pathology was
partition geometry (slice size, then split rule) — fixed by v0.2's
even-split target-size geometry at `t=400`; what remains is context
eviction, which is the harness's job, not the resolver's.
