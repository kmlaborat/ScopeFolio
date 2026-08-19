# ScopeFolio Specification

**Status:** Draft v0.1.0

## 1. Overview

ScopeFolio is a deterministic file-reading tool that exposes a requested line range through a virtual partitioned view of a file.

It is designed for coding agents that can reliably identify relevant lines using deterministic tools such as `grep`, but may struggle to translate those line numbers into an appropriate `Read` range.

ScopeFolio therefore separates:

* **location** — performed by the agent using search tools
* **scope construction** — performed deterministically by ScopeFolio
* **file reading** — performed by ScopeFolio

The agent only needs to provide a target line.

```text
grep
  │
  │ line = 597
  ▼
ScopeFolio
  │
  ├─ construct partition tree
  ├─ locate partition containing line 597
  ├─ apply offset
  └─ return text
```

The partition structure is an implementation detail and is never exposed to the agent.

---

# 2. Design Goals

ScopeFolio MUST:

1. provide deterministic access to a region around a requested line;
2. accept a single target line as the primary navigation primitive;
3. construct its partition structure from the file on demand;
4. maintain no persistent state;
5. require no file hash or identity mechanism;
6. hide partition-tree navigation from the agent;
7. support configurable partition width;
8. support configurable contextual overlap around the selected partition;
9. work independently of `grep`, `glob`, or any particular agent framework;
10. return ordinary text suitable for direct insertion into an LLM context.

ScopeFolio SHOULD:

* minimize unnecessary file content returned to the model;
* preserve useful surrounding context around the target line;
* make repeated reads of nearby locations inexpensive in terms of agent reasoning;
* remain simple enough to implement as a small standalone tool.

---

# 3. Non-Goals

The initial implementation does NOT attempt to:

* provide semantic search;
* perform fuzzy search;
* perform embedding-based retrieval;
* maintain a persistent partition index;
* maintain a cache;
* identify files by hash;
* expose partition IDs to agents;
* decide which line the agent should search for;
* replace `grep`;
* replace ordinary unrestricted file reading;
* optimize partition width automatically.

Semantic or fuzzy localization may be explored separately in the future.

---

# 4. Terminology

### 4.1 Target Line

The line supplied by the caller.

```text
line = 597
```

The target line is the agent's primary navigation coordinate.

### 4.2 Partition

A contiguous region of a file represented by a node in the partition tree.

### 4.3 Leaf Partition

The smallest partition produced by the partitioning algorithm.

The initial implementation targets approximately `partition_lines` lines per leaf.

### 4.4 Partition Tree

A binary tree representing recursive subdivisions of the file.

The tree is an internal implementation detail.

### 4.5 Offset

Additional surrounding lines returned around the selected partition.

Offset exists to prevent an otherwise arbitrary partition boundary from cutting off useful context.

---

# 5. Core Model

Given a file:

```text
SPEC.md
1
2
3
...
1193
```

and:

```text
partition_lines = 50
```

ScopeFolio constructs a binary partition tree.

Conceptually:

```text
SPEC.md
│
├── P0
│   ├── ...
│   └── leaf
│
└── P1
    ├── ...
    └── leaf
```

The exact internal tree structure MUST NOT be exposed as part of the public interface.

Given:

```text
line = 597
```

ScopeFolio determines the leaf containing line 597 and returns that leaf plus the requested contextual offset.

The caller does not need to know whether line 597 belongs to `P0`, `P1`, or any other internal node.

---

# 6. Partitioning

## 6.1 Binary Partitioning

The initial implementation uses recursive binary partitioning.

Given a line interval:

```text
[start, end]
```

ScopeFolio splits it into two child intervals until each leaf is approximately `partition_lines` lines wide.

For example:

```text
1 ───────────────────────────── 1193
              │
       ┌──────┴──────┐
       1            597       1193
       │              │
   ┌───┴───┐      ┌───┴───┐
```

The implementation MAY choose the exact split point according to a deterministic balancing rule.

The resulting tree MUST be deterministic for the same file contents and configuration.

---

# 7. Partition Width

The public parameter is:

```text
partition_lines
```

It represents the target number of lines in each leaf partition.

Default:

```text
partition_lines = 50
```

Examples:

```text
--partition-lines 50
```

```text
--partition-lines 25
```

Smaller values produce finer-grained partitions.

Larger values produce larger reading scopes.

`partition_lines` is a target, not necessarily an exact line count.

The implementation MAY adjust boundaries to satisfy the binary partitioning algorithm.

---

# 8. Offset

ScopeFolio supports contextual expansion around the selected partition.

The public parameter is:

```text
offset_ratio
```

Default:

```text
offset_ratio = 0
```

When:

```text
partition_lines = 50
offset_ratio = 0.1
```

the selected partition is expanded by approximately 10% of its target width on both sides.

Conceptually:

```text
        contextual offset
       <-------------->
       ┌──────────────────────────────┐
       │                              │
       │       selected partition     │
       │                              │
       └──────────────────────────────┘
       <-------------->
        contextual offset
```

The resulting range MUST be clamped to the actual file boundaries.

For example:

```text
partition width = 50
offset ratio    = 0.1

offset ≈ 5 lines
```

resulting in approximately:

```text
[start - 5, end + 5]
```

---

# 9. Target-Line Resolution

The primary operation is:

```text
read(file, line)
```

The algorithm is:

1. Open the target file.
2. Determine its line structure.
3. Construct the binary partition tree.
4. Find the leaf partition containing `line`.
5. Expand the partition according to `offset_ratio`.
6. Clamp the resulting range to the file.
7. Return the selected lines.

The target line MUST be contained in the returned range unless the requested line is outside the valid file range.

---

# 10. Boundary Conditions

If the requested line is near the beginning:

```text
line = 3
```

the lower offset is clamped to line 1.

If the requested line is near the end:

```text
line = 1190
```

the upper offset is clamped to the final line.

If the requested line is invalid:

```text
line < 1
```

or:

```text
line > file_line_count
```

ScopeFolio MUST return a deterministic error.

---

# 11. Statelessness

ScopeFolio MUST be stateless.

Each invocation independently derives its partition structure from the current file.

It MUST NOT require:

* persistent indexes;
* databases;
* cache files;
* partition metadata;
* file hashes;
* session identifiers.

This is intentional.

ScopeFolio is a **computed view**, not an index.

```text
file
 │
 └──► ScopeFolio
        │
        ├── partition
        ├── locate
        └── read
              │
              ▼
            result
```

After the operation completes, no ScopeFolio state is required to remain.

---

# 12. File Identity

Unlike AnchorScope, ScopeFolio does not require content identity.

AnchorScope uses deterministic anchoring because its purpose is to protect a modification target against changes.

ScopeFolio does not modify the file.

Its purpose is simply:

> Given the current file and a line number, show the appropriate local scope.

Therefore file hashing and anchor validation are outside the initial scope.

---

# 13. Output

The output SHOULD clearly identify the returned range.

For example:

```text
SPEC.md:574-625

574 | ...
575 | ...
...
597 | target content
...
625 | ...
```

The output MUST preserve the original file contents.

ScopeFolio MUST NOT modify:

* whitespace;
* indentation;
* line endings;
* encoding;
* source text.

The line-number presentation is metadata added to the returned representation and MUST NOT alter the underlying file.

---

# 14. Agent Interaction Model

The intended interaction is:

```text
Agent:
  Grep "FASTCONTEXT_TIMEOUT_SECONDS"
        ↓
  result: config.ts:42
        ↓
  ScopeFolio(config.ts, line=42)
        ↓
  relevant local scope
```

The agent does NOT need to reason about:

```text
partition ID
tree depth
parent node
child node
sibling node
partition boundary
```

This is a core design requirement.

### Principle

> **The agent specifies WHERE. ScopeFolio determines HOW MUCH.**

---

# 15. Relationship to Grep and Glob

ScopeFolio does not replace deterministic search tools.

The intended workflow is:

```text
Glob
  ↓
identify files

Grep
  ↓
identify relevant lines

ScopeFolio
  ↓
retrieve the appropriate local scope

Agent
  ↓
reason about the retrieved content
```

This addresses the specific failure observed in small coding agents:

```text
Grep → line identified
       ↓
       agent must decide how much Read to perform
       ↓
       oversized Read / repeated Read / wrong Read
```

ScopeFolio moves the second decision from the model into deterministic infrastructure.

---

# 16. Why a Tree?

A simple fixed-width sliding window could provide:

```text
line 597
   ↓
Read 572-622
```

and is sufficient for the simplest implementation.

However, ScopeFolio retains a binary partition tree because it provides a foundation for future scope-resolution strategies while keeping the tree completely hidden from the agent.

The initial tree is therefore an **implementation mechanism**, not an agent-facing abstraction.

Future versions MAY use the tree for:

* adaptive partition sizes;
* semantic partitioning;
* search-guided refinement;
* RAG-assisted localization;
* fuzzy scope resolution.

These extensions MUST NOT require agents to understand the tree.

---

# 17. Sliding Bisection

Sliding Bisection is NOT part of the initial ScopeFolio partitioning algorithm.

The initial implementation uses simple binary partitioning.

Sliding Bisection MAY be introduced later when ScopeFolio needs a localization signal beyond an explicit line number.

Potential future inputs include:

* fuzzy search;
* semantic similarity;
* embeddings;
* RAG retrieval;
* structural similarity.

The distinction is:

```text
Current ScopeFolio:

line number
    ↓
binary partition tree
    ↓
scope


Future ScopeFolio:

search / semantic signal
    ↓
Sliding Bisection
    ↓
partition tree
    ↓
scope
```

---

# 18. Determinism

For the same:

* file contents;
* target line;
* `partition_lines`;
* `offset_ratio`;

ScopeFolio MUST produce the same output range.

No LLM inference, randomness, or external retrieval is permitted during scope resolution.

---

# 19. Configuration Defaults

Initial defaults:

| Parameter         | Default |
| ----------------- | ------: |
| `partition_lines` |    `50` |
| `offset_ratio`    |     `0` |

Example:

```bash
scopefolio read SPEC.md --line 597
```

Equivalent to:

```bash
scopefolio read SPEC.md \
  --line 597 \
  --partition-lines 50 \
  --offset-ratio 0
```

A caller may request:

```bash
scopefolio read SPEC.md \
  --line 597 \
  --partition-lines 25 \
  --offset-ratio 0.1
```

---

# 20. Error Handling

ScopeFolio MUST return deterministic errors for:

* file not found;
* unreadable file;
* invalid line number;
* invalid `partition_lines`;
* invalid `offset_ratio`.

It MUST NOT silently substitute another file or another line.

---

# 21. Implementation Constraints

The reference implementation SHOULD:

* be implemented in Rust;
* have no external service dependencies;
* have no persistent state;
* have no database;
* have no hash index;
* have no LLM dependency;
* expose a minimal CLI/library API.

The implementation SHOULD favor correctness and simplicity over premature optimization.

---

# 22. Testing

Tests MUST cover at least:

### Partitioning

* small files;
* files smaller than `partition_lines`;
* exact partition boundaries;
* odd line counts;
* uneven final partitions;
* large files.

### Line resolution

* first line;
* last line;
* partition boundary;
* line immediately before boundary;
* line immediately after boundary;
* middle of a partition.

### Offset

* zero offset;
* positive offset;
* offset at file beginning;
* offset at file end;
* large offset.

### Determinism

The same input and configuration MUST produce identical ranges and output.

### Content preservation

Returned content MUST exactly correspond to the selected source lines.

---

# 23. Future Extensions

The following are intentionally deferred:

### 23.1 Adaptive Partition Width

Determine partition width based on:

* syntax;
* file structure;
* model context capacity;
* observed retrieval success.

### 23.2 Semantic Partitioning

Use code structure such as:

```text
module
 ├── function
 ├── function
 └── class
```

to produce semantically meaningful scopes.

### 23.3 Sliding Bisection

Use a search/relevance signal to recursively identify the most relevant scope.

### 23.4 RAG Integration

Use embeddings or other retrieval signals to guide partition selection.

### 23.5 Multi-line Queries

Allow a caller to provide multiple target lines and return their minimal enclosing scope.

These extensions MUST preserve the principle that the complexity remains inside ScopeFolio rather than being transferred to the agent.

---

# 24. Design Principle

ScopeFolio follows a simple principle:

> **Do not teach the agent how to navigate the partition tree. Give the agent a line, and let the tool find the scope.**

This makes ScopeFolio complementary to AnchorScope.

```text
AnchorScope
    ↓
stable scope for WRITE


ScopeFolio
    ↓
appropriate scope for READ
```

Both are based on the broader concept of **Scope**, but solve opposite sides of the agent's instability problem.

**AnchorScope:** *Where is it safe to write?*

**ScopeFolio:** *What is the appropriate scope to read?*
