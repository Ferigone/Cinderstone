# DamageSource / per-Level DamageSources native stage

Ownership: issue4 comment6566884842, instance es-20261101-1425-a7c3;
observed team capacity root plus six subordinate agents. Only new
`src/combat/damage_source.rs` in
`/workspace/cinderstone-damage-owners-es-20261101-1425-a7c3`, branch
`native-damage-sources`, base96b4bad622155e2891d9d8a249f8e233721d34c8.
No shared source, export, Main, manifest, docs/status, Git or remote mutations
were performed by this child agent. Read-only `git status --short` confirms
only the new claimed source file. Root owns publication and integration.

## Pinned source and method boundary

Direct JDK21 javap of the original Java1.21 server, official mappings;
no REA execution is claimed. Inner SHA256
8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01;
mapping SHA1 31c77994d96f05ba25a870ada70f47f315330437.
Raw private scratch disassembly, never proposed for Git:

- oracle/effects-damage-source.javap SHA256
  7f55dd14927804a08863b62a73d0dd625711e65cb04c0bf8be48604e53a04e52:
  brk constructors38–59, state/getters31/35/67/76/81/107–150.
- oracle/nov2-damage-provider.javap SHA256
  f7f6a889a3d708ab8bc57dc6a73d569600f3e80fb735b65a41f8e1687aeac3fb:
  brl46–271, brm12–44, acu27–34.
- oracle/nov2-damage-holder.javap SHA256
  964c3fb67bd52c98bbb9e3839220a3524b5e709243dc102bb8eb1ae66f48ee72:
  actual reference key identity192, live tag bindings197/250–251.

Source facts implemented:

- DamageType stores immutable JavaString message_id, scaling, raw f32 exhaustion,
  effects and death-message type. The original convenience constructor defaults
  HURT / DEFAULT and WHEN_CAUSED_BY_LIVING_NON_PLAYER are preserved. No finite,
  positive or zero-normalization rule is inserted.
- DamageSource has exactly four private final field values: actual holder,
  causing reference, direct reference, nullable raw position. Constructor order
  is holder,DIRECT,CAUSING. Single-entity constructor retains that same actor
  twice; bare/position constructors have both entity references null.
- Source handle identity is Arc; no Clone/PartialEq for source state. Null-null
  isDirect is true; nonnull comparison delegates mandatory actual object
  identity rather than numeric-ID equality. Dynamic weapon reads DIRECT entity,
  with Some(EMPTY) distinct from None. Position prefers raw, otherwise current
  retained DIRECT position. Removed actor handles do not retarget by current ID.
- Exhaustion/message/scaling query the holder's current value. Tags query current
  bindings; ResourceKey comparison preserves original canonical identity.
  WHEN scaling checks CAUSING LivingEntity&&!Player; creative uses CAUSING
  Player abilities.instabuild, not invulnerability or inferred gamemode.
- DamageSources constructor obtains one actual Level RegistryAccess registry,
  then required environmental holders in exact brl48–71 order. All environmental
  getters return the identical cached source. Fresh factories lookup required
  holders from that retained registry on every invocation and allocate sources.
- Unattributed fireball228–231 uses single-entity form, making the actual fireball
  both direct and causing. Explosion255 chooses PLAYER_EXPLOSION iff both refs
  are nonnull, without a Player class test. Explosion251 reads direct before
  indirect and only then delegates. Null Explosion skips both callbacks. Explicit
  bad-respawn point263 stores raw position with both entity refs null. No RNG.
- Optional DamageEventSnapshot implements only complete base-source constructor
  fields27–34: current victim ID, same holder, current causing ID, current direct
  ID, raw position. Null IDs use -1; derived source position is not substituted.

## Exact callable APIs / actual-owner obligations

`DamageType::{new,standard,with_effects,living_scaled,living_scaled_standard}`,
getters and the three actual definition enums. No input codec or configured
registry is constructed here.

`DamageSource<H,E>::{unattributed,from_entities,at_position,from_entity}` return
Arc<Self>; field getters retain exact references. Methods damage_type,
food_exhaustion,message_id,is_tag,is_key,is_direct,weapon_item,
scales_with_difficulty,is_creative_player,source_position use mandatory fallible
hosts. Canonical source holder/type record is never inferred from an integer.

`DamageRegistryHost` requires registryOrThrow(DAMAGE_TYPE), actual ResourceKey
selector-to-key mapping/getHolderOrThrow, retained current immutable type value,
actual current tag membership and canonical ResourceKey identity. All missing
owners/required keys/unbound values fail at the callback. `DamageTypeKey` is a
selector enum for the original keys, never a protocol registry numeric table.

`DamageEntityHost` requires retained canonical Entity handle, true object identity,
actual class predicates, Player.instabuild, current actual position and nullable
virtual weapon. Entity Clone must retain the same original body after removal,
respawn or ID reuse; it must not clone state or lookup a replacement body.

`DamageSources<R,H,E>::new(host)` creates the actual per-Level cache object;
`registry`, `environmental` and all24 named cached getters; all original fresh
single/paired factories; fireball, explosion, from_explosion and
bad_respawn_point_explosion. Specialized Java parameter class constraints are
explicit caller preconditions erased to retained E; no extra runtime class
checks were invented. `ExplosionSourceHost` supplies original direct/indirect
source references. Per-Level installation/current Entity.level selection is
required from Main/world owners, not claimed here.

`damage_event_snapshot(victim,&source,host)` requires
`DamageEventSnapshotHost::entity_id` on the retained actor. Snapshot is limited
to this final base-source implementation; arbitrary subclass virtual getters
are unsupported. No encoder, registry-ID resolution, packet ID, transport,
tracking audience or publication is provided or claimed.

Each mandatory host method returns after releasing all actual mutable owner
borrows. Source/entity/registry state is not duplicated in this module. There
are no default-success callbacks, empty owner substitutions, fabricated registry,
actor, world, health, RNG, tag or scheduler states.

## Focused validation and limits

Actual owned file formatted with stable rustfmt. Production standalone library
compile and focused rustc --test -Dwarnings compile/run PASS. Harness imports
the actual crate geometry::Vec3 and nbt::JavaString from the previously built
crate rlib; no geometry/NBT stubs. Command:

    .../rustc --edition=2021 --test -Dwarnings
      /workspace/scratch/es-20261101-1425-a7c3/damage-source-harness.rs
      --extern mc121_rust=/workspace/cinderstone-entities-es-20261101-1425-a7c3/target/debug/deps/libmc121_rust-0f71457713b7cad7.rlib
      -L dependency=/workspace/cinderstone-entities-es-20261101-1425-a7c3/target/debug/deps
      -o /workspace/scratch/es-20261101-1425-a7c3/damage-source-tests
    /workspace/scratch/es-20261101-1425-a7c3/damage-source-tests

Controlled Rc<RefCell> actor/registry fixtures verify same-ID distinct identity,
borrow release at every host callback, cache/fresh identity, retained registry
after current registry changes, nullable/nonempty weapon identity, direct removal
and ID reuse, live type/tag changes, canonical key identity, raw -0/NaN values,
causer class/instabuild, all factory key selections, fireball/explosion quirks,
callback mutation between direct/indirect reads, current packet IDs/rawpos and
registry/query/packet failure prefixes. These fixtures are isolated native
component validation, not real actor/world/Main execution or a new original-JVM
probe. Existing source disassembly is the pinned original source evidence.

Frozen source SHA256
082ae30298f22a037e9a7699b395e71e8cd81600d54f6e7aa46e085d12d90043.
Independent effects_lifecycle source review APPROVED the final hash against
original brk/brl/brm/acu with no material assigned-behavior mismatch, and
independently compiled/ran final actual-file -Dwarnings focused native harness
against real crate geometry/NBT. No reviewer source edits.

Explicit remaining obligations: central canonical configured damage registry,
bound holder/tag/key identity, actual per-Level cache installation and current
level access, retained actor leases/current ID/weapon/abilities owners, bridges
to existing living/player damage hosts, actual Component localized death message
and hurt-sound owner, holder-ID packet encoder/audience/self publication, positive
and lethal production damage lifecycle. DamageType record codec, direct/unbound
holder provider behavior and arbitrary DamageSource subclasses are not supplied.
Immutable Java Vec3 coordinates use the actual existing native Vec3 value;
general Java Vec3 reference identity is not part of the exposed API. This is
explicit in the source header after the reviewer's wording clarification.
No Main proof, full survival delivery completion or full Cargo result is claimed.
