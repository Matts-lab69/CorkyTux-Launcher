# Performance audit (2026-09-28, debug build)

Tools available: `ps`, `/proc`, `du`. No heaptrack/perf/valgrind/`/usr/bin/time`
on this host, so no heap profiles or flame graphs; measurements below are
RSS/threads/CPU/fd based. Release profile already exists
(`opt-level=3`, `lto=true`, `strip=true`, `codegen-units=1`); no change
needed there, and no release rebuild was done (LTO cost).

## Before (Library page, idle)

- RSS 238 MB, stable across 4 min (238316 kB → 238316 kB).
- 20 threads (mostly GPU/glib pools: `gdrv`, `gl`, `traceq`, `gdbus`…).
- CPU 13% at 25 s (startup incl. cover fetch) → 1.4% settled.
- voluntary ctxt switches ~2.4/s idle. 15 fds.
- Binary (debug): 164 MB. `target/`: 5.3 GB. `~/.cache/CorkyTux`: 650 MB.

## Findings → fixes (this audit)

1. `modicons/` cache unbounded: 650 MB / 698 files, single covers ~4 MB.
   Fix: LRU prune at startup (`prune_icon_cache`, 512 MB / 1500 files,
   `*-icon.png` + `*-icon.missing` + stale `*-raw.bin`; foreign files
   untouched, thumbnails self-regenerate). First live run: 87 files,
   139 MB freed (650 → 510 MB).
2. Log modal re-read + repainted the whole Proton log 2×/s while open.
   Fix: skip when `(mtime, len)` fingerprint is unchanged.
3. Session poller (2 s, by design), modal pumps (100 ms, die with the
   dialog), negative icon cache (1 h TTL, self-cleaning), bounded
   thumbnail decode (`max_px`), async cover slot gate: reviewed, no
   change — all terminate or are cheap by construction.

## After (same scene, new build)

- RSS 236 MB, 20 threads, CPU settling (4.3% at 1 min, was 1.4% at
  4 min pre-change; same trajectory, no regression).
- Cache 510 MB / 611 files after first prune.

## Not done (explicit)

- No per-cover downscale at save time (4 MB files remain until LRU
  evicts them); disk-only cost, RAM already bounded at decode.
- No release-binary size measurement (no rebuild done).
- No optional no-network sandbox flag (see `docs/wine-isolation.md`).
