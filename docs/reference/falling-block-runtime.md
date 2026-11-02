# FallingBlock runtime and native order owners

This stage adds a genuine FallingBlock owner to the existing native shared entity index, level tick list and ChunkMap sender sequence. It does not substitute an ItemEntity, independently allocate IDs or reseed an entity. Main world admission, block callbacks and physical ticking are separate integration responsibilities; source publication alone is not a working Anvil placement claim.

## Actual state and registration

`MainFallingBlockRuntime` owns one boxed `FallingBlockState` per admitted object. `ConstructedFallingBlock` retains that same Box before the actual world-add attempt. An accepted add moves the Box into the canonical map without changing its address and returns the actual new generation-qualified shared handle. A false add retains the original object in the unregistered token. `object_state_mut` therefore resolves the same constructed object for Anvil's subsequent hurt configuration in either case. No replacement object or fake registered handle is returned.

The actual world host must check constructor, index, UUID, callback and publication capabilities before acquiring process constructor entropy. The runtime's preflight only verifies its actual ID/capacity owners; it is not a successful world admission or a configured registry certificate. Real registration uses `RuntimeEntities::register_falling_block`, updating the one shared section index, tracking order and level tick list according to actual chunk visibility. The `move_falling_block` and `remove_falling_block` methods run at the literal position/discard callbacks. A stale generation cannot remove a replacement entity. `reap_removed` only destroys a removed Box after source callbacks have finished using it.

The canonical actor remains in the map while physical ticking borrows its `state` and separate `publication` fields. `broadcast_tracked` uses this publication owner's actual current recipients for the source ChunkMap block update; it does not make a second distance query. Save, UUID checks and physical queries read the same owner.

## Sender source evidence

The pinned Minecraft Java 1.21 inner server SHA-256 is `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`; official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`. Evidence is direct JDK21 `javap`, plus the team's explicitly reported unchanged-original bootstrap observations, not REA MCP execution. The observed built-in FallingBlock type is registry ID40, range10, interval20 and trackDeltas=true. State dimensions and constructor/NBT boundaries are documented in [falling-block-state.md](falling-block-state.md).

Original `ChunkMap` (`aqb`) tick, mapped lines1136–1166, calls `ServerEntity.sendChanges` after an actual section change or while the distance manager reports the entity-ticking range. Fresh direct bytecode inspection confirms that gate at offsets166–194. The new Falling sender requires the actual range predicate as an argument and retains its last section; there is no default-true fallback. Observer pairing does not itself advance the movement clock. Original `ServerEntity` (`aqt`) constructor75–109, sendChanges112–252 and sendPairingData271–325 supply the retained tracker and pairing phases. The native type sends AddEntity, optional nondefault metadata and explicit motion for nonliving pairing, including zero motion. It drains actual metadata/impulse flags once through its shared sender.

Audience callbacks and unusual observer transitions remain bounded by the surrounding native tracking integration. The older external XP sender is not modified by this stage, and its separate source limitations are not claimed fixed by the Falling sender gate.

## One atomic native entity cohort

`PersistentEntityOrder` captures the actual native registration sequence, per-section encounter sequence and currently eligible level tick sequence for Item, XP and FallingBlock. Snapshot references include their original generations and kinds; `validate_entries` checks the complete physical cohort, finite positions, unique IDs/generations and exact section membership before restore mutates an owner. The persistence coordinator stores actual XP/Falling bodies and this order inside the same checksummed `EntityCheckpoint` as items, source history, world image, time, retained Level/sound RNGs and shared entity counter.

Cold restore registers genuine restored bodies with fresh handles and then validates/rebinds the captured order over those actual owners. It requires no live players and entirely hidden, empty tick/sender owners. Actual section storage is rebuilt in captured encounter order; no sorted-ID approximation is used. The saved eligible native subsequence is prioritized only as real confirmed chunk callbacks activate those members, preserving new Players' positions in the shared tick list and its existing copy-on-write iteration. Once every captured member has first activated or been removed, ordinary insertion order resumes. Removal of a still-hidden captured member cannot keep a completed priority cohort active through a later callback.

This is a native continuation policy. Network sender baselines, live players and chunk readiness start fresh. It does not restore an old fastutil sender table with ghost players or force hidden actors into ticking. Missing old atomic cohorts remain an explicit legacy migration boundary; historical mixed order is not invented. The authoritative present cohort, including EMPTY, is separate from that legacy path.

## Validation boundary

Owned code passed whitespace inspection. Root performs the focused compiler check against the exact composed source and reports runtime evidence separately. No new comprehensive suite, test-count progress metric, production FallingBlock gameplay, full Java compatibility or checklist completion is asserted here.
