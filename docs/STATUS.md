# Milestone status

| Milestone | Status | Branch | Approved tag |
|-----------|--------|--------|--------------|
| M0: Bootstrap and dependency verification | Implemented, awaiting independent review (PLAN.md section 10, step 3). **Not approved.** | `milestone/m0` | none |
| M1: Map and camera | Not started | | |
| M2: Units and movement | Not started | | |
| M3: Economy and construction | Not started | | |
| M4: Combat and fog of war | Not started | | |
| M5: Interface and audio | Not started | | |
| M6: Skirmish AI and game flow | Not started | | |
| M7: Content, balance, performance, and packaging | Not started | | |

## M0 implementation report (2026-10-03)

### Validation commands (PLAN.md section 9)

Run from the repository root on macOS 27.0.1, Apple M5, Rust 1.98.1, in this order, after the last code change:

| Command | Exit | Result |
|---------|------|--------|
| `cargo fmt --all -- --check` | 0 | no diff |
| `cargo clippy --workspace --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 27 passed, 0 failed, 0 ignored: `game` unit 7, `game` `tests/m0_spike.rs` 6, `sim` unit 2, `sim` `tests/m0_bootstrap.rs` 12, doc-tests 0 |
| `cargo build --release -p game` | 0 | |
| `cargo run --release -p game -- --smoke-test 600` | 0 | log: `smoke test: 600 frames, 198 simulation ticks, spike map created: true` (a display was available) |

### Acceptance criteria

| Criterion | Evidence |
|-----------|----------|
| Every assumption has a recorded outcome with evidence | `docs/DECISIONS.md`: A1 true (with API mappings), A2 true (with an I8 caveat for M1), A3 false as stated, recorded fallback applied, A4 resolved (pin 1.98.1, Bevy requires 1.95.0), A5 true. F1 to F3 re-checked. |
| All versions are pinned | Every direct dependency uses `=x.y.z` (`crates/sim/Cargo.toml`, `crates/game/Cargo.toml`); `Cargo.lock` committed; toolchain pinned in `rust-toolchain.toml`. |
| All validation commands pass | Table above. |
| Test: tick counter equals N after N runs | `tick_counter_equals_n_after_n_schedule_runs` (N = 1, 2, 20, 1000), `tick_counter_increments_by_exactly_one_per_run`, `tick_counter_starts_at_zero` |
| Test: same seed and empty command log give equal hashes | `same_seed_and_empty_command_log_give_equal_hashes_at_every_tick` (every tick of 500) |
| Test: different seeds that consume randomness give different hashes | `different_seeds_consuming_randomness_give_different_hashes` (every tick of 500), `hash_reflects_consumed_randomness` |
| Scope: workspace, `SimPlugin` with `SimSchedule`, tick counter, `CommandQueue`, `state_hash` | `crates/sim/src/{schedule,command,hash,id,rng}.rs` |
| Scope: `game` binary opens a window; `--smoke-test <frames>` | `crates/game/src/{main,cli,smoke}.rs`; smoke run above; CLI errors exit with code 2 (unit tests in `cli.rs`; also run by hand: `--smoke-test 0`, `--smoke-test`, `--bogus`) |
| Scope: isometric Tiled map spike | `crates/game/src/spike.rs`, `assets/maps/spike/`; tests in `crates/game/tests/m0_spike.rs`; on-screen result **requires user verification** |

### Deviations from PLAN.md

1. `SimSchedule` has a tenth stage, `SimSet::EndOfTick`, after the plan's nine, to increment the tick counter (DECISIONS D3).
2. `Command` has no variants in M0 (DECISIONS D5), so `CommandQueue::push` cannot be exercised by a test until M2.
3. Pinned Bevy 0.19.1 (the plan names 0.19); F1's date differs from crates.io by three days (DECISIONS F1).
4. Files not listed in PLAN.md section 4: `.cargo/config.toml` (asset root, DECISIONS D1), `crates/game/examples/gen_spike_tileset.rs` (tileset generator), `crates/game/tests/m0_spike.rs` (spike and projection tests).
5. The `cargo init` stub (`src/main.rs` printing "Hello, world!", and a root `Cargo.toml` package named `MyGame001`, both never committed) was replaced by the workspace layout of section 4.

### Known debt (P2 candidates)

1. On macOS, dev-profile links of `game` print a linker warning (`__eh_frame section too large ... compact unwind table`). It is a warning only; clippy and release builds are unaffected.
2. The first smoke run logged one `WARN bevy_winit::state: Skipped event Destroyed for unknown winit Window Id` at shutdown; the final run did not. Exit code was 0 in both.
3. `game` uses a constant simulation seed (`MATCH_SEED = 0`) until match setup in M6.
4. `crates/game/src/spike.rs` is spike code to be replaced by the M1 map loading and rendering.
5. The smoke test fails if the spike map has not been created by the last frame, so very small frame counts (a few frames) fail by design (DECISIONS D6).
6. `bevy_ecs_tiled` panics on staggered and oblique maps; the M1 loader must validate maps first (DECISIONS A2 caveat, invariant I8).
7. Packaged builds must ship `assets/` next to the executable (DECISIONS D1, M7).

### User verification

| Item | Outcome |
|------|---------|
| 1. `cargo run --release -p game` shows the spike map: 16×16 green diamond map, **red** tile at the **top** corner (cell 0, 0), **blue** tile at the **right** corner (cell 15, 0), 3×3 **gold** ore patch left of and slightly below the centre, tiles edge to edge | **Verified by the user, 2026-10-03.** |
| 2. Platforms | The user decided that running on macOS is sufficient (DECISIONS U1). Verified on macOS only; Windows and Linux were not tested. |
| 3. Toolchain pin 1.98.1 (Bevy minimum is 1.95.0) | Not explicitly confirmed; the user asked to proceed without objecting. The reviewer may raise it. |

### Next step

M0 is ready for independent review (PLAN.md section 10, step 3) of the cumulative diff `main..milestone/m0`.
