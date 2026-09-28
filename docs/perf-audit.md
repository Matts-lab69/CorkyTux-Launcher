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
3. Markdown rendering compiled 13 regexes per description (`clean_md`
   + `md_segments`, run per row while browsing Modrinth). Fix: static
   `OnceLock` patterns. Measured: 200 renders 2.06 s → 0.44 s (4.7×).
   Same for the per-launch GE-Proton version regex.
4. Themed-image registry grew with dead WeakRefs between theme
   switches (tile re-renders register hundreds). Fix: prune above
   1500 entries on push.
5. Session poller (2 s, by design), modal pumps (100 ms, die with the
   dialog), negative icon cache (1 h TTL, self-cleaning), bounded
   thumbnail decode (`max_px`), async cover slot gate: reviewed, no
   change — all terminate or are cheap by construction.

## After (same scene, new build)

- RSS 236 MB, 20 threads, CPU settling (4.3% at 1 min, was 1.4% at
  4 min pre-change; same trajectory, no regression).
- Cache 510 MB / 611 files after first prune.

## Round 2 (deep dive, same scene)

- smaps breakdown (RSS 231 MB): HEAP 65 MB, anon 37 MB, shared libs
  ~128 MB (LLVM 39, gallium 16 — GPU driver, unavoidable), binary
  13 MB. Owned memory is ~140 MB; the rest is shared.
- `TEX_CACHE` (64 decoded textures) used clear-all eviction: up to
  ~45 MB churn with mass re-decode on next scroll. Fix: true LRU
  (evict oldest) plus dedup of the order queue.
- Decode bound was flat `size*2`: covers decoded 4× the pixels needed
  on scale-1 screens. Fix: `size * scale_factor` in `load_mod_icon`.
- glibc arenas: 20+ threads grow many arenas that retain freed heap.
  Fix: `MALLOC_ARENA_MAX=4` at startup (UI is mostly main-thread).
- Experiment REVERTED: decoding PNGs in worker threads (+75 MB anon
  Private_Dirty, 232 → 312 MB settled; thread-made GdkTextures also
  risk Toolkit thread-affinity rules). Main-thread decode stays; the
  jank it would fix was never measured, the regression was.
- Result after round 2: RSS 232 MB, HEAP 65 MB, anon 37 MB —
  identical profile, no regression, markdown 4.7× faster.
- Deliberately not touched: 17 KB Games.ini re-parse every 2 s poll
  (~µs, not worth caching/staleness risk), reqwest client-per-call
  (sporadic calls only), release rebuild for numbers.
- Still open: per-cover downscale at save time, release-binary size,
  optional no-network sandbox flag (see `docs/wine-isolation.md`).
