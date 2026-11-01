# Native BLOCK/redstone continuation — 1.21

Artifact: official server-inner-1.21.jar, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
This is a bounded native port, not full redstone or full vanilla parity.

## Sources and actual owners

- `tick.rs` is the already ported LevelTicks (`eyq`). `block_ticks.rs` reuses its
  real tick callback with the collected `toRunSet` alive during reentrant work.
  Block singleton identity is the first state ID (`placement_block_min_state`),
  so fire AGE and repeater properties cannot create extra scheduled identities.
  On callback failure, the executed prefix remains real; remaining collected
  ticks retain trigger time, priority and sub-tick insertion order.
- `tools/redstone_facts.java` bootstraps the unchanged JAR and captures 1060 block
  declarations for weak/direct signal, neighborChanged, onPlace and onRemove,
  plus actual BlockSetType button duration/arrow sensitivity, pressure-plate
  sensitivity/maximum weight/sounds and Lever sound ID. The corresponding
  `fixtures/redstone-facts.json` is the used runtime catalogue.
- Static source methods were inspected using javap against `ddm` (SignalGetter),
  `dkl` (Lever), `dgi` (Button), `dfm`/`dlw`/`dpp` (pressure plates),
  `dmc` (lamp), `dia`/`dmg` (diode/repeater), `dmd`/`dme` (torch),
  and `dlj` (observer signals). `dcx` proves the no-volume sound overload uses
  volume=1 and pitch=1: buttons and pressure plates use this; levers explicitly
  retain volume0.3 and pitch0.6/0.5.

## Connected implementation

Main fields own a single global BLOCK queue and a native redstone runtime.
Fire placement, scheduled fire and lava's random ignition all schedule into
that same queue. The legacy fire queue is adopted once during cold restore;
it is no longer the live independent scheduler. Chest/barrel 5-tick rechecks
run through the same queue via their actual Main callback.

Signal queries preserve weak/direct distinctions and solid-conductor relay.
Native callbacks implement levers, buttons, lamp on/off delay, repeater delay
2/4/6/8, locking, source topology priorities, pulse extension, placedBy1-tick
scheduling, and torch inversion/burnout. Pressure plate code reads actual
player/item AABBs; weighted plates count all eligible entities regardless of
BlockSetType's ordinary-plate sensitivity. Reentrant neighbor work uses a
stack of multi-neighbor continuations and the configured chaining limit.

Main's ordered interaction effects retain fluid/fire/redstone completion
markers separately; a failed prefix is not replayed as an already-executed
callback. Native source proofs accept only owned callbacks. Global queue use
is sticky even if all pending work has subsequently finished.

## Explicit limits

Wire signal/topology and observer lifecycle are not connected in this batch;
queries requiring them fail rather than inventing zero power. Comparator
output requires its real BE owner and remains unavailable. Piston/TNT and
other unported receivers/lifecycle callbacks remain unsupported. Redstone
writes reaching an unowned liquid neighbor callback fail explicitly.
Pressure plate ItemEntity `entityInside` needs its actual composed item host;
the player-body override alone is not proof of that item callback. The native
source contains only player/item actor kinds, allowing button Arrow queries
to establish an empty result only while that source history remains valid.
Imported world histories never acquire these capabilities from an empty diff.
Torch burnout history is a transient Level owner, initialized for the actual
cold level construction; no claim is made about process-suspension snapshots.

A joint compiler check is performed by Root. Comprehensive gameplay comparison
is deferred per the user's port-first instruction. No test count measures this
port's completion.
