# hydra-mapdb

The map-data pipeline for **Hydra** (working name **Cena**, the game client
this feeds): download the upstream GemStone IV map, convert it to Hydra's
room format, and combine it into the single binary the client loads.

This is a **data repo run by CI**, not client code. The client
([Nisugi/cena](https://github.com/Nisugi/cena)) never sees upstream JSON or
Ruby; it only ever loads the binary this repo publishes.

## Pipeline

```
FarFigNewGut/lich-mapdb-room  ──►  convert.yml  ──►  a PR against rooms/
      data/map.json               (manual only)      (reviewed, merged by hand)
                                        │                    │
                                  residue report              ▼ combine.yml
                                                      hydra.map (release asset)
```

`rooms/` is **committed source, not derived output that gets regenerated**:
Hydra's mapdb is still being designed (Cena's `plan/21-mapdb.md`), and it
already holds -- or will hold -- content upstream never had: floors, layout,
room identification, and whatever else the design settles on. A bot that
silently overwrote all 36,838 room files from upstream on a schedule would
overwrite that too. So only one of the two workflows runs unattended.

- **`convert.yml`** (manual only — `workflow_dispatch`) downloads
  [`FarFigNewGut/lich-mapdb-room`](https://github.com/FarFigNewGut/lich-mapdb-room)'s
  `data/map.json` — checking `data/updated_at` first so an unchanged upstream
  costs one small request, not a 42 MB one — runs `cena-mapdb-convert` on it,
  and runs the **porting ratchet**
  (`crates/mapdb-convert/tests/ratchet.rs`) against the freshly downloaded
  map (if upstream added a scripted crossing this repo has never ported, the
  job fails and names it, and no PR is opened). If the ratchet passes, it
  opens a **pull request** with the converted diff against `rooms/` — it
  never pushes to `main` itself. Review it like any other change: does this
  diff track an upstream update, or did it clobber something hand-authored?
  Merge, edit, or close it.
- **`combine.yml`** runs on every push to `main` that touches `rooms/` —
  which now only happens when a `convert.yml` PR is merged, or a room file
  is hand-edited directly — plus on demand. It is the one step safe to
  automate: a pure, deterministic build over whatever is already committed,
  publishing `hydra.map` as the `latest` GitHub release (replaced each run)
  plus a short-lived build artifact.

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
