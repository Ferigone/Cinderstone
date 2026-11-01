# Shared item context and sound RNG bridge

`RuntimeEntities::simulate_level_with_context(world,facts,colliders,cache,rain,host)`
accepts optional mutable `FluidShapeCache` and `RainExposure` capabilities. Both
owners are reborrowed for each provider and retained across items; neither is
cloned, reset or synthesized. Existing `simulate_level` and
`simulate_level_with_fluid_cache` wrappers preserve their context defaults.

`with_sound_random(actual_level.thread_safe_random)` consumes the actual separate
Level sound stream. `new(world_random)` leaves it unknown. A sound without its
owner errors before a Level.random draw or host sound callback. It does not
substitute a clone or a default seed. The host must serialize this owner with
its world/registry lock; concurrent CAS semantics are not claimed.

The original rainy corpus invokes unchanged complete `ItemEntity.tick` with
actual baseTick/move/rain/pitch and Level.playSound. Its eight scenes include
clear/wet transitions, float rain-threshold behavior and an initially embedded
item whose upper rain query becomes exposed later. Native composition compares
all80 complete physical/metadata states,6 ordered sound events and the entity,
Level.random and Level.threadSafeRandom states after every tick. Level.random
remains unchanged; the actual sound owner alone supplies seed nextLongs.

A separate two-item composition test reuses one genuine fresh-plains exposure
owner over multiple items/ticks and verifies rain/sound callback ordering and
actual shipped ThreadSafeLegacyRandomSource scalar nextLong vectors. This is
not an original two-item world-scene oracle. Another controlled two-item test
crosses two water-ray sites: first shallow water then water with water above.
The shared fluid cache must retain the first shallow shape for the second item,
while both items also exercise the rain capability. This edited-world test uses
explicit controlled rain input; it does not certify edited-source precipitation.
Missing rain and missing sound owners have explicit error tests, preserving the
real post-movement state and leaving Level.random untouched.

The same test filter replays2,976 old dry/submerged states,82 final entity RNG
states and five existing coordinator integration tests. The new proof adds four
composition tests. Input expectations never drive native transitions.

`python3 tools/runtime_entities_rain_check.py` checks debug. `--release` checks
Cargo release; an explicit CARGO_TARGET_DIR selects the post-v5 proof directory.
The report merges profiles only when all source hashes match. Full main-loop,
network audience, unowned source histories, concurrent RNG use and complete
server compatibility remain separate work.
