# Beacon runtime reference ledger

This stage adds borrowed-owner kernels in `beacon_block_entity.rs`,
`beacon_effects.rs` and `beacon_data.rs`. It does not register a native beacon
block entity, attach a ticker, apply effects in Main or complete beacon gameplay.
The final unpublished composition base is
`26c4d2ec13b885e51025e105f1edeb0fe78f6309`; the three source files were preserved
unchanged when root advanced the initial `fb3` preparation base.
The existing [beacon menu ledger](workstations-beacon.md) remains the reference
for the separate temporary payment, lock, cursor and menu synchronization owners.

## Pinned primary evidence

Evidence is direct JDK21 `javap -p -c` inspection and official mappings of the
original Java 1.21 server, protocol767. It is not REA or an original-Java gameplay
run. No raw disassembly or Java artifact is redistributed.

- Inner server JAR SHA256:
  `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
- Official server mapping SHA1:
  `31c77994d96f05ba25a870ada70f47f315330437`.
- `BeaconBlockEntity` (`dqc`): tick130–213, updateBase216–240,
  setRemoved245–247, applyEffects250–272, playSound275–276,
  getBeamSections279, storeEffect293–296, loadEffect300–309,
  loadAdditional314–324, saveAdditional328–339 and setLevel419–421.
- Its ContainerData (`dqc$1`): get92–96, set102–112, count116;
  beam section (`dqc$a`)399–413; `DyeColor` (`cti`)50–68;
  `FastColor.ARGB32.average` (`axy$b.c`)70–74.
- `BeaconMenu` (`cpy`) effect/data conversion147–152;
  `MobEffectInstance` (`brz`) constructors, `EntityGetter` (`dcn`) class queries,
  `Level` (`dcw`) sound overloads and `ConstructBeaconTrigger` (`be`)18–19.

## One actual BE and its source tick

`BeaconFields` borrows the same raw levels, canonical primary/secondary holders
and transient `BeaconScanState` that the actual menu/data owner must use.
`BeaconScanState` is not another block entity or power/payment owner. Its Java
constructor state has two empty beam lists and lastCheckY0. The outer real
BlockEntity.setLevel runs first; `after_set_level(actual_min_build_height)` then
sets lastCheckY to that minimum minus1. Opening a menu must not reset this scan.

`BeaconTickHost` supplies actual WORLD_SURFACE height, block states, contextual
`getLightBlock(Level,pos)`, canonical BEDROCK identity, the BEACON_BASE_BLOCKS tag
and BeaconBeamBlock/DyeColor behavior. Missing chunks, tags, dynamic-light or
world owners are errors. The admitted Level position and four-layer base extent
must fit the native world coordinate domain; arbitrary i32 endpoint tuples are
not certified.

One tick scans at most10 positions, including the actual surface-height bound.
Colors are packed texture-diffuse **ARGB integers**, not floating RGB triples.
While the checking list has zero/one section, a colored block creates another
section directly. Later equal colors extend the last section; differing colors
create the per-channel integer-floor average, including alpha. Heights retain
Java-int arithmetic. A noncolored block extends only an existing section and
only when contextual light blocking is below15 or its actual block is BEDROCK;
these tests preserve source short-circuit order. An obstruction clears checking.
The pyramid inspects at most four complete layers, x outer/z inner, through the
actual base tag and actual minimum build height.

The ordering is material:

1. Incrementally scan into checking; capture the old raw levels.
2. On gameTime%80==0, the **old committed beam** decides whether to recompute
   levels. Positive levels plus that old beam run captured-argument effects,
   then ambient sound.
3. Only after that phase, a completed scan replaces committed beam values and
   resets lastCheckY. The old/new raw-level comparison decides activation or
   deactivation. Activation sound precedes its separate criterion query.

An empty old committed beam does not reset levels. Obstruction and raw levels
can therefore remain out of step; no eager repair was added. The source beam
getter masks its committed list only when levels==0, not whenever levels<=0.
The primary data setter instead observes the unmasked committed list.

## Effects, criterion and sound

`apply_effects_staged` uses captured levels and canonical holder references.
The effect query is the unit block AABB inflated by levels*10+10 and expanded
upward by the actual Level dimension height. Range/duration arithmetic wraps as
Java int before conversion; duration is (9+levels*2)*20. The actual Player.class
query applies NO_SPECTATORS, body-box intersection and source encounter order,
without an added alive/creative/immunity or radial-distance filter.

Every queried player receives a fresh actual MobEffectInstance primary request
before any secondary request. Duration/amplifier and ambient/visible/showIcon
flags are explicit; all three flags are true. Levels>=4 plus equal holders gives
primary amplifier1; otherwise it is0. The same captured player list is reused
for a distinct secondary at levels>=4, amplifier0. Source compares the captured
holders again after all primary callbacks. The host invokes the actual
addEffect(instance,null-source) path and ignores its Boolean; an ignored return
is not permission to skip merging, attributes, packets or effect ticking.

`activate_staged` sounds first, then separately queries ServerPlayer.class in
the normalized box from (x,y,z) to (x,y-4,z), inflated by (10,5,10). This is not
the effect AABB and has no unit-block +1. Each CONSTRUCT_BEACON invocation rereads
the actual levels after prior callbacks. Actual listener predicates, player
context, rewards and any callback RNG remain with the real criterion owner.

`BeaconSoundHost` delegates the actual BEACON ambient/activate/deactivate/power-
select event to Level.playSound(null,pos,event,BLOCKS,1f,1f). The sound wrapper
uses block centers and exactly one nextLong from **Level.threadSafeRandom**
before seeded broadcast, including an empty audience; no pitch draw belongs to
this call. Main already retains/checkpoints that stream through
`RuntimeEntities::next_sound_seed` and `sound_random_state48`. A fake Item batch,
new stream, player RNG or Level.random substitute is unnecessary. Binding the
actual beacon phase, sound definition, audience and publication is still required.
An unavailable stream remains an error. The one-draw fact concerns the sound
wrapper; real effect/criterion callbacks retain their own RNG obligations.

Source setRemoved plays unconditional deactivation sound **before** superclass
removal. This lifecycle method remains outer-owned; the new tick kernel does not
claim beacon removal or silently equate menu closing with BE removal.

## ContainerData and partial persistence

`BeaconDataHost` resolves the actual built-in mob-effect holder IdMap/keys and
canonical VALID_EFFECTS membership. Missing registry ownership is an error,
separate from an actual unresolved lookup. Data count is3:

| Index | Source field and setter behavior |
| --- | --- |
| 0 | Raw Java-int levels; assignment has no clamp or tier validation. |
| 1 | Primary: server plus nonempty raw committed beam sounds power-select **before** decoding/filtering/assignment, including unchanged, absent or invalid choices. |
| 2 | Secondary: decode/filter/assign, without selection sound. |
| Other | Getter0; setter no-op. |

A data power is actual holder ID+1, with0 for absent/unmapped; decode uses ID-1
and actual lookup, retaining Java wrapping. This is distinct from the
serverbound SetBeacon packet's presence-bit/optional direct registry-ID codec.
VALID_EFFECTS filtering is not a pyramid-level or primary/secondary tier check.

`set_data` owns a standalone BE transaction. `set_data_in_stage` is for the
existing menu's enclosing real transaction: do not nest publication between its
primary, secondary, payment removal and later blockEntityChanged. These helpers
do not add menu broadcasts, dirty notifications or another payment owner.

`load_effect_fields[_in_stage]` loads only primary/secondary keyed holders;
missing, wrong-type, invalid-key, unresolved or nonvalid effects become absent.
It does **not load Levels**, even on an existing BE, or invoke ContainerData
setters/sounds. Full source load order remains superclass → power helper →
conditional actual CustomName JSON → actual LockCode → one outer publication.
The ResourceLocation adapter rejects non-ASCII UTF-16 keys without lossy
replacement; its65535-unit native NBT admission bound is explicit.

`save_effect_fields` stages only keyed power strings followed by raw Levels on
a detached target compound. Absent/direct-unkeyed holders do not erase existing
keys, matching storeEffect. Full save/updateTag/updatePacket, metadata and
components remain outside: superclass → power/Levels helper → optional Name →
Lock. No complete BE serializer or new Name/Lock implementation is claimed.

## Mandatory owner publication and remaining integration

`BeaconTransactionHost::begin_stage/publish_stage` must stage the actual BE,
world/registry/tag binding, living effects, criterion rewards, sound RNG and
ordered outbox together. `require_actual_stage(action)` checks a detached,
owner-bound action capability before public staged helpers observe or mutate
anything; a caller Boolean/shallow clone is insufficient. Publication validates
the same actual BE lifetime/version/context and adopts once after all fallible
preparation. It must not leak callbacks or save/drain network output before the
outer ownership interval completes. Independent constructor/factory entropy
requires its real acquisition boundary; this trait does not promise global
rollback or permit rewinding a process RNG. Observations do not authorize replay.

Concrete missing bindings at this base include:

- Native beacon creation/type record, common fields, Level attachment, ticker,
  chunk/removal lifetime and full NBT/network update owner. Existing native BE
  storage covers chest/barrel and furnace families; palette identity is not a BE.
- Actual ordered player queries and canonical live effect-map/addEffect owner.
  Existing `effects/instance` and retained-instance ticking primitives are not a
  Main active-effect map or an application/attribute/packet pipeline.
- A real construct-beacon criterion bridge. Generic empty-criteria admission
  does not itself implement this callback or certify unavailable rewards as empty.
- Main beacon block-use/menu binding, World/BE tick call phase, registry/source
  admission, atomic sound/effect/criterion publication and persistence hooks.
  The existing bounded native player profile has no active-effect producer;
  beacon buffs cannot be enabled by silently retaining its no-effects certificate.

The existing Main connection decoder already validates optional direct effect
IDs against its pinned built-in registry export. That wire validation is not a
beacon dispatcher, a canonical runtime holder provider or living-effect owner.

No Main calls, positive beacon/player-effect gameplay or aggregate workstation
completion are claimed. Root's final isolated all-target compiler check passed
on base `26c4d2ec13b885e51025e105f1edeb0fe78f6309`. The focused
`workstations::beacon_` library filter passed six cases: three geometry/arithmetic
checks and controlled beam/data ordering, atomic-stage and partial-persistence
flows. Callback/holder recorders are **synthetic fixtures**, not actual native
world, live-effect, criterion, Main or original-Java execution proof. Root also
reported changed-source format/diff checks passing. Logs are
`/tmp/ws-beacon-runtime-final-check.log` and `/tmp/ws-beacon-runtime-native.log`.
This ledger's author ran no build, server or gameplay oracle.

After the preserved base advance, root corrected two fixture-only RNG method
calls to the real Rust `next_i64` API; functional implementation was unchanged.
The following final source snapshots include that fixture correction:

| Source | SHA256 |
| --- | --- |
| `beacon_block_entity.rs` | `37c75b8f0dac1181b4ab43463d7c092b2fc184239a0d73a146e7dce42e7474ab` |
| `beacon_effects.rs` | `ebbc73a18421f4dbfb9df64c23a35786dfdb82c953dcfa985ce174236a4f8e93` |
| `beacon_data.rs` | `546263fa6dcb4be841452abea596dd52129624a3924152de444fb77ec5c2604b` |
