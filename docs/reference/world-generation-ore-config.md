# Configured ore leaf catalog

Target: pinned original Minecraft Java 1.21, protocol 767. This ledger comes from
direct `javap` and official mappings, not REA. The original inner server SHA-256
is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
the official mapping SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.

`fixtures/world-generation-ore-configs.json` contains the original 32 configured
ore JSON entries, with individual raw-source SHA-256 digests. Thirty use
`minecraft:ore`; `ore_ancient_debris_large` and `ore_ancient_debris_small` use
`minecraft:scattered_ore`. This is a closed ore subset of the original 196
configured features, not a complete configured/placed feature registry. No
placement modifiers or FeatureSorter data are included.

## Configuration and rules

Official `OreConfiguration` (`eeq`, constructor lines 22–38 and codec lines
12–16) stores ordered `targets`, integer `size`, and float
`discard_chance_on_air_exposure`. Its JSON codec requires all three fields,
permits size **0 through 64**, and permits discard probability **0 through 1**.
The target list uses an unrestricted list codec, so an empty list is legal.
The actual packaged sizes range from 2 through 64. Direct Java constructors
supply defaults in some overloads; the JSON codec does not inherit those
constructor defaults.

`TargetBlockState` (`eeq$a`, codec lines 45–48) pairs a RuleTest and exact
BlockState. Target encounter order is preserved. For example, `ore_iron`
first targets `stone_ore_replaceables` with iron ore, then
`deepslate_ore_replaceables` with deepslate iron ore. Original state properties
remain part of the output: redstone ore targets specify `lit=false`, and
infested deepslate specifies `axis=y`.

The closed catalog uses only `BlockMatchTest` (`emv`) and `TagMatchTest` (`env`).
Their actual `test(BlockState,RandomSource)` methods call block identity or tag
membership without reading the random argument. They do not compare one exact
property state. Native `OreRuleTest::matches` therefore accepts every valid
property state of the corresponding block. Its canonical representative state
and every candidate/output state are validated through the existing 1.21
canonical registry; unknown states error.

The only block-match identity in the packaged data is netherrack. The four
actual tag identities are `base_stone_overworld`, `base_stone_nether`,
`stone_ore_replaceables`, and `deepslate_ore_replaceables`. Membership reuses
`placement-block-tags-1.21.json`, with explicit source/version/tag/member
validation before calling the existing membership helper. This prevents its
unknown-tag-to-false behavior from hiding an unavailable ore tag. Membership
normalizes property variants to block identity. The four original tag-source
digests are also listed in the new fixture.

Original Java execution must bind actual holder tags. Basic registry bootstrap
alone leaves those holder memberships unbound. The preceding carver reference
probe binds the pinned original tag pack through actual TagFile CODEC,
TagLoader.build, and Registry.bindTags; ore reference execution must retain
that setup. Native tag predicates must never supply the Java reference result.

## Callable API and ownership

`feature::config::ConfiguredOreFeature::{Ore,ScatteredOre}` owns an
`OreConfiguration`, exposed by `configuration()`. `OreConfiguration` retains
ordered `Vec<TargetBlockState>`, size and original narrowed `f32` discard
probability. Its public `validate()` checks caller-constructed values before
host/RNG activity. It rejects invalid sizes, nonfinite/out-of-range discard
probabilities, and unknown output/predicate states, while accepting source-legal
zero size and empty targets. Configuration and rule validation do not fork,
reseed or consume a random source.

`builtin_configured_ore(name)` returns a shared immutable entry from the 32-name
closed catalog; unknown feature/type/tag/block/state identities fail. Lookup
by name requires an actual unchanged-registry and unchanged-tag certificate
from its caller. A familiar name alone does not establish that fact.
`builtin_configured_ore_from_snapshot(name,actual)` additionally requires exact
JSON-value equality with the packaged configured-feature snapshot, so a changed
config under an original name fails. It does not certify external tag owners.
This check expects the original packaged data shape, not arbitrary equivalent
serialization with newly expanded defaults.

The private packaged-data decoder rejects extra fields as closed-scope policy;
that is stricter than a claim that all original general-purpose codecs reject
unknown object keys. Random RuleTests, arbitrary tag identities, arbitrary
configured feature types and resource-pack overrides are unsupported. Public
controlled leaf configurations may use either deterministic rule over validated
canonical block states, but do not replace the closed registry certificate.

## Validation and delivery boundary

Focused native checks exercise the source-legal zero/empty boundaries, reject
bad probabilities and identities, preserve iron target order/redstone output
properties, verify deepslate axis variants match by block/tag identity, and
reject a changed known-name snapshot. The separate original/native ore leaf
material probe is the behavioral delivery witness; catalog size and decoder
checks are not placement evidence.

This module supplies configuration and deterministic target predicates only.
Ore geometry, AIR exposure checks, actual host writes, caller RNG ownership,
retained terrain integration and original runtime comparison belong to their
respective leaf owners. It does not advance a terrain image to FEATURES, run
placed modifiers, establish a decoration seed/feature index, schedule neighbor
work, or certify a server generation status.
