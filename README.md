# ScopeFolio
ScopeFolio is a deterministic file-reading tool that exposes a requested line range through a virtual partitioned view of a file.

## Documentation

The specification and its rationale live in `docs/`:

```text
SPEC_v0.2.0.md: current normative specification (what the implementation must satisfy)
SPEC-V0.2-DESIGN.md: why this specification (design record, comparison of candidate rules)
EXPERIMENTS-ARCHIVE.md: the experimental evidence behind its decisions (frozen)
SPEC_v0.1.0.md: historical reference only (superseded by v0.2.0)
```

## Usage

### CLI

```bash
# Read the scope around line 597 (defaults: --partition-lines 400, --offset-ratio 0)
scopefolio read --file src/lib.rs --line 597

# Explicit target leaf size and contextual offset
scopefolio read --file docs/SPEC_v0.2.0.md --line 296 \
    --partition-lines 400 --offset-ratio 0.1
```

On success the range header and numbered lines go to stdout:

```text
src/lib.rs:401-800

401 | ...
597 | ...
800 | ...
```

On failure a deterministic error code (`FILE_NOT_FOUND`, `INVALID_LINE`,
`INVALID_PARTITION_LINES`, `INVALID_OFFSET_RATIO`, `IO_ERROR: ...`) is
written to stderr and the exit code is 1.

### Library

```rust
use scopefolio::read;

let result = read("SPEC_v0.2.0.md", 296, 400, 0.0)?;
println!("range {}-{}", result.start_line, result.end_line);
println!("{}", String::from_utf8_lossy(&result.content));
```
