# Concrete powder: actual native snapshot and cold-resume admission

Target: Minecraft Java1.21/protocol767. This production stage changes only
`main_falling_block_persistence.rs` and its coordinator-owned Main call sites.
It uses the existing atomic native Item/XP/Falling cohort and complete
`FallingBlockSnapshot`; there is no new powder save array, checkpoint version,
source summary, reconstruction seed or Java entity-NBT importer.

## Real class, source and installed owners

The library's `source_class` identifies only the16 pinned ConcretePowderBlock
(`dhk`) singletons and their actual captured final concrete Block associations.
`concrete_powder_hardened_state` supplies the captured concrete default, without
color-name or palette-offset inference. Those source facts are insufficient to
admit native resume by themselves. The source basis is the pinned original
inner SHA256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`
and official mappings SHA1 `31c77994d96f05ba25a870ada70f47f315330437`; the original
`dhk` and inherited `diu` methods are documented in
[concrete-powder.md](concrete-powder.md).

Before capturing/restoring an actual powder body, the persistence owner requires:

- The actual `NativeEntitySource.require_current_world(world)` succeeds over
  the real current fixture world; a source-present boolean or final palette
  comparison does not replace that history.
- The actual restored BLOCK runtime is healthy, and its once-installed
  `NativeFallingDamageTypes` is available.
- The same runtime owns its actual `NativeConcretePowderOwner`, captured from
  the actual outgoing configuration/tags and original Block associations.
- That retained owner's binding matches the saved/actual powder singleton and
  the classifier's captured concrete association.

Vehicle/passenger state, active portal processing and TileEntityData without
their actual owners remain errors. No field is cleared to pass admission. No
new bounds, velocity, blocksBuilding, time or constructor-default policy is
invented for an otherwise valid native snapshot. Original Anvil/ordinary
admission behavior remains outside the added powder-specific branch.

## Actual save and startup phases

Main now calls `snapshot_native_falling_blocks(&players,&world)` under the SAME
actual world/player/clock interval as the existing source, fluid/BLOCK queues,
XP and atomic entity checkpoint. It validates actual registered powder bodies
and installed owners, verifies the actual indexed Falling handle/id/kind/
generation and canonical body UUID, and rejects UUID collisions with actual
live Item/XP/Falling/player owners. These temporary validation sets are not a
second persisted cohort or identity owner. It
then delegates the real Falling runtime's insertion-order
snapshot. The existing checksummed cohort and exact SavedWorld-byte binding
remain authoritative. This helper does not construct a scene, edit a saved file
or manufacture a positive checkpoint.

Cold startup retains the existing world/checkpoint/source validation. Under the
coordinator's exact startup grant, the one cohort-restore call moves after final
BLOCK checkpoint replacement, legacy-fire adoption and the real once-only
damage/powder capture/install. It remains before restored fluid-cache adoption,
Server exposure, player admission and chunk-readiness callbacks. All other
initialization ordering stays unchanged. In particular this helper does not
require a fluid shape cache or a FULL/ticking chunk before its actual existing
startup phase; the real first activated tick performs those capability checks.

The shared saved Level.random is restored before the now-later cohort call.
That call performs no random sampling: neither per-actor restore nor index,
tracker or order registration consumes it. Normal cold-Level construction is
not described as JVM suspension; the existing native continuation policy still
restores its recorded Level/sound streams and randValue separately.

## Same actual bodies, one cohort and genuine hidden activation

All item/XP/Falling IDs and UUIDs are checked against the shared next-entity
counter and complete saved order before external registration. The actual
shared index's full external registration batch is also validated. Every
Falling snapshot is restored ONCE into its actual constructed Box token during
preflight, after powder owner admission. The same retained token is subsequently
registered; it is not discarded and reconstructed by another source constructor.

| Retained native payload | Resume behavior |
| --- | --- |
| Canonical FallingBlock physical state and dimensions | Existing snapshot/physical validation remains; actual state is retained, not converted to ItemState or reset at the saved position. |
| Per-actor random_state48 and UUID | Snapshot restore inverses LegacyRandomSource's constructor scramble and samples nothing. Entity/UUID constructors and seed uniquifiers are not invoked. |
| Carried block state and original concrete association | Exact saved state remains; admitted source class and installed canonical owner are checked before registration. |
| StartPos, Time, blocksBuilding and flags | Exact saved values remain; StartPos is not inferred from current position, Time is not restarted, flags are not replaced with constructor defaults. |
| Physical caches, movement/fire/fluid state and float-bit fields | Existing full snapshot owners remain authoritative; no source state is recomputed to make a checkpoint pass. |
| Shared registration/section/tick order | Existing actual `PersistentEntityOrder` is rebound once over actual new handles; no sorted-ID approximation or second powder order owner. |

Present native cohorts, including EMPTY, remain authoritative and never append
stale legacy XP. Absent cohorts retain the old explicitly narrower XP migration
policy; missing powder records are not invented. The existing source/index
restorer requires fresh hidden owners. Real new chunk callbacks later enable
eligible members and restore the saved native subsequence. Network audiences,
sender baselines and old chunk readiness are not restored from a stale session.
Powder resumes do not bypass the first real tick's movement/clip/world owners,
and this helper does not execute an extra begin-level tick.

## Evidence and limits

The source kernels, native snapshot restore, actual Main startup/save phases
and existing mixed-order/index contracts were inspected directly. No Java
binaries, private saves, synthetic checkpoint or validation artifact was added.
No build, tests or cold-reopen gameplay witness was run by this owner; the
coordinator compiles the coherent source and validates actual scenes separately.
Concrete-powder tick/clip/fluid/hardening callbacks remain separate real owners;
this admission stage alone does not establish their complete gameplay parity,
arbitrary configured registries, Java-save imports or any checklist completion.
