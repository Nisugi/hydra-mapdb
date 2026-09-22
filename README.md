# hydra-mapdb

The map-data pipeline for **Hydra** (working name **Cena**, the game client
this feeds): download the upstream GemStone IV map, convert it to Hydra's
room format, and combine it into the single binary the client loads.

This is a **data repo run by CI**, not client code. The client
([Nisugi/cena](https://github.com/Nisugi/cena)) never sees upstream JSON or
Ruby; it only ever loads the binary this repo publishes.

## Pipeline

```
FarFigNewGut/lich-mapdb-room  ──►  convert.yml  ──►  rooms/*.json (committed)
      data/map.json                    │                    │
                                        │                    ▼ combine.yml
                                  residue report      hydra.map (release asset)
```

- **`convert.yml`** (weekly, or on demand) downloads
  [`FarFigNewGut/lich-mapdb-room`](https://github.com/FarFigNewGut/lich-mapdb-room)'s
  `data/map.json` — checking `data/updated_at` first so an unchanged upstream
  costs one small request, not a 42 MB one — and runs `cena-mapdb-convert`
  on it. That writes one JSON file per room under `rooms/` (sharded by
  thousand, so an update is a small, reviewable diff) and commits the
  result. It also runs the **porting ratchet**
  (`crates/mapdb-convert/tests/ratchet.rs`) against the freshly downloaded
  map: if upstream added a scripted crossing this repo has never ported,
  the job fails and names it.
- **`combine.yml`** (on every push that touches `rooms/`, or on demand) runs
  `cena-map-combine` over whatever is currently committed and publishes the
  resulting `hydra.map` as the `latest` GitHub release, replaced each run,
  plus a short-lived build artifact.

The two are separate so a converted diff can be looked at before it becomes
what players download, and so hand-editing a room file directly doesn't
require a 42 MB re-download to see it combined.

## The two tools

Both are ported from Cena's `plan/21-mapdb.md`, which records the design in
full (crossing recognition, the shape ratchet, the binary format).

- **`cena-mapdb-convert`** — the only place upstream Ruby is ever seen.
  Recognises each scripted `;e` crossing by shape, extracts its parameters,
  and writes a named, typed exit; whatever it cannot yet recognise is left
  out and counted in the report, never guessed at.
- **`cena-map-combine`** — reads the per-room files and writes one binary,
  little-endian, versioned, every string interned once. An older client
  loads a newer map without crashing: an unknown crossing or cost name loads
  as `Unknown` (impassable) rather than refusing the file.

Both depend on `cena-map` — the room record and binary format — as a **git
dependency on the client's own repo**, not a vendored copy, so there is one
definition of the format instead of two that can drift.

## Running it locally

```powershell
cargo run --release -p cena-mapdb-convert -- upstream_map.json rooms
cargo run --release -p cena-map-combine   -- rooms hydra.map
```

To run the porting ratchet against a local copy of the upstream map:

```powershell
$env:CENA_MAPDB = "path\to\upstream_map.json"
cargo test --release -p cena-mapdb-convert --test ratchet -- --nocapture
```
