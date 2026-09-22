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
                                  residue report             │
                                                             ▼ combine.yml
an issue with an export  ──►  corrections.yml  ──►  a PR   hydra.map
   (hydra-mapper)            (validate, or say    against    (release asset)
                              why not)          corrections/      ▲
                                                      │           │
                                                      └───────────┘
```

`rooms/` is **committed source, not derived output that gets regenerated**:
Hydra's mapdb is still being designed (Cena's `plan/21-mapdb.md`), and it
already holds -- or will hold -- content upstream never had: floors, layout,
room identification, and whatever else the design settles on. A bot that
silently overwrote all 36,838 room files from upstream on a schedule would
overwrite that too. So only one of the three workflows runs unattended.

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
- **`corrections.yml`** runs when somebody files a *Map corrections* issue with
  an export from [`hydra-mapper`](https://github.com/Nisugi/hydra-mapper)
  attached. It checks the file is **well-formed** — its rooms exist in the
  current map, its `dirto` entries agree with each other, its placements do not
  compound, and nothing in it contradicts a correction already accepted — and
  either says why not, on the issue, or opens a **pull request**. Whether a
  correction is *right* is not something a bot can tell, so that is the review.
  The submitter's SVGs ride along on the branch so a reviewer can see the
  layout; squash-merging keeps them out of `main`.
- **`combine.yml`** runs on every push to `main` that touches `rooms/` or
  `corrections/` —
  which now only happens when a `convert.yml` PR is merged, or a room file
  is hand-edited directly — plus on demand. It is the one step safe to
  automate: a pure, deterministic build over whatever is already committed,
  publishing `hydra.map` as the `latest` GitHub release (replaced each run)
  plus a short-lived build artifact.

## Corrections

A correction is somebody's fix to the map — an exit that really goes the other
way, a room drawn on the wrong grid, a room dragged where walking it puts it.
They are made in `hydra-mapper` and submitted here as one exported file, whose
format is specified in that repo's `docs/corrections-format.md`.

**Accepted corrections live in `corrections/`, one file per submission, and are
applied to `rooms/` during the build — not written into it when the PR merges.**
That is the whole design, and it exists because `rooms/` is regenerated from
upstream: a correction baked into a room file is lost the next time
`convert.yml`'s PR merges, and lost silently, in a 36,838-file diff nobody can
read. Keeping corrections as their own reviewed input means an upstream refresh
cannot drop them — `convert.yml` re-applies them over each fresh conversion —
and a bad one is reverted by reverting its commit rather than by hand-repairing
a room file.

The directory is **append-only**: a submission is never edited in place, so two
corrections never conflict in git. What they can do is correct the same thing
differently, which is caught twice. `corrections.yml` refuses a submission that
contradicts an accepted one, while the person who made it is still in the
conversation; and the build refuses to fold two that contradict, rather than
letting filename order decide. The second only fires when two PRs merged too
close together for the first to have compared them.

## The three tools

The two converters are ported from Cena's `plan/21-mapdb.md`, which records the
design in full (crossing recognition, the shape ratchet, the binary format).

- **`cena-mapdb-convert`** — the only place upstream Ruby is ever seen.
  Recognises each scripted `;e` crossing by shape, extracts its parameters,
  and writes a named, typed exit; whatever it cannot yet recognise is left
  out and counted in the report, never guessed at.
- **`cena-corrections`** — validates a submission against the current map and
  the accepted set (`validate`), and folds the accepted set into the room files
  a build reads (`apply`). The rules it enforces are `corrections-format.md`'s
  "Validating a submission" section, quoted at the checks that implement them.
- **`cena-map-combine`** — reads the per-room files and writes one binary,
  little-endian, versioned, every string interned once. An older client
  loads a newer map without crashing: an unknown crossing or cost name loads
  as `Unknown` (impassable) rather than refusing the file.

All three depend on `cena-map` — the room record and binary format — as a **git
dependency on the client's own repo**, not a vendored copy, so there is one
definition of the format instead of two that can drift.

## Running it locally

```powershell
cargo run --release -p cena-mapdb-convert -- upstream_map.json rooms
cargo run --release -p cena-map-combine   -- rooms hydra.map
```

To check a corrections file the way the issue workflow does, and then apply
everything accepted:

```powershell
cargo run --release -p cena-corrections --bin validate -- export.json rooms .
cargo run --release -p cena-corrections --bin apply    -- . rooms
```

To run the porting ratchet against a local copy of the upstream map:

```powershell
$env:CENA_MAPDB = "path\to\upstream_map.json"
cargo test --release -p cena-mapdb-convert --test ratchet -- --nocapture
```
