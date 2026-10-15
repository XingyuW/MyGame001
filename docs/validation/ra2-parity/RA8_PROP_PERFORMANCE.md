# RA8 Prop membership measurement

The five-second native sample in `ra8-preflight/ten-player-frostline-sample1.txt` records
1408 of3626 sampled main-thread stacks in the per-tick construction of an ordered set of
every Prop ID. The sample was taken during the first all-map attempt; that attempt is
disclosed as sampled, and is not substituted for the unprofiled mean benchmark.
Sampling lasted5 seconds; its incremental CPU/timing overhead was not separately quantified.
No sampling runs during the before/after/final profile or the 256×256 mean benchmarks.

`special/advance.rs` now checks only active interaction targets with `Props::get`. It
collects expired actor IDs in the existing ordered interaction iteration before mutating
SpecialState. The original actor/target membership predicate is preserved, including Props
whose hit points have reached zero but whose removal is pending. No RNG, simulation state,
definitions copy, target-selection policy or command-validation rule is added.

Reproduce the measurement with:

```text
cargo run --release -p sim --example ra8_prop_profile
```

The harness uses normal setup_match, default MatchOptions, shipped definitions, ten Medium
controllers, Frostline enlarged to252×252, seed13 and5000 scheduled ticks. Tick measurement
excludes state hashing/event formatting; total elapsed includes both. No native sampling
runs during either measurement. The runtime/data implementation is otherwise unchanged
between the two raw captures.

| Evidence | Tick total | Mean tick | Total elapsed incl digest |
| --- | ---: | ---: | ---: |
| `ra8-preflight/prop-profile-before1.log` | 5.3972386s | 1.079447ms | 14.918685625s |
| `ra8-preflight/prop-profile-after1.log` | 1.741916029s | 348.383µs | 11.032613583s |
| `ra8-preflight/prop-profile-final1.log` | 1.752774649s | 350.554µs | 11.027596792s |

All three captures end with14342 Props, state digest `94f4c6583a68ff22`, and660 events with
digest `acbe6072fabd5e3a`. Exact state and event digest/count equality was checked, including
the final runtime after R99/R100. The initial before/after pair changes only membership
cleanup. These
measurements establish the local cost reduction and equivalence, not final gate timing.
Interaction/destruction/garrison regression results and final ten-player benchmark and
all-map completion evidence are recorded separately in the RA8 acceptance record.

The separate staged whitespace check returns2, not a pass, solely for the preserved raw
terminal blank line at `ra8-preflight/prop-boundaries-after1.log:219`. Exact diagnostic and
unfiltered stdout are in `ra8-preflight/performance-staged-whitespace-check1.json`. No
source/data/test diagnostic occurs. A separate recheck must confirm this identical set
after staging the classification and before the performance commit.
