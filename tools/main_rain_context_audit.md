# Main rain and sound composition

`Server.tick_item_world` delegates to `LivePlayers.simulate_item_world` in
`src/main_rain_context.rs`. Startup takes both independent streams from one
`create_level_randoms` result. `RuntimeEntities` owns the actual level sound
stream; the level's ordinary random remains the weather/drop owner. The fresh
source alone creates a persistent `FreshFlatPlainsRain` temperature cache.

The rain availability adapter requires the persistent unedited lighting witness,
scheduler quiescence and the manager's current FULL certificate. The existing
whole swept collision/game-event halo certificate still runs before simulation.
The actual rain provider also checks world base states, biome, sparse edits and
its measured point window. Missing facts remain errors.

The new main tests run the same production helper with genuine forced-ticket
manager polling, `WorldContext`, `MainItemGameplay` and `RuntimeInventoryHost`.
They compare all exposed first-tick physical/metadata fields of the unchanged
Java `wet_fire10` and `clear_fire10` frames. The wet sound packet contains the
original pitch and seed `-3561856981251203394`; the sound RNG advances even with
no recipients, while Level.random stays unchanged. The separate coordinator
report covers all80 original frames and the opaque entity RNG owner.

This is a fresh unedited native plains fixture capability, not general lighting,
imported-world or whole-server compatibility. Player callbacks still have the
explicit `NativeItemCallbacksOnly` scope. No v5 validation files are changed.

Reproduce with `python3 tools/main_rain_context_check.py`; use `--release` for
the optimized profile. The checker runs all main-bin regressions and records
source, original-fixture, command and log hashes.
