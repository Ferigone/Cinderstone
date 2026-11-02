# Beacon subclass state and lifecycle handoff

Instance: `ws-20261101-7c2f`; team: WORKSTATIONS.
State: ready for integration; publication and required CI follow.
Base: `e1471304cbd3e2cb4a30d0b522682084fded0d80`.
Branch: `teams/workstations/ws-20261101-7c2f/beacon-entity`.
Assigned [issue #3](https://github.com/Ferigone/Cinderstone/issues/3),
[exact claim](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6569390389).
Target: original Java 1.21, protocol 767.

## Exact owned source

- New `src/workstations/beacon_entity.rs`.
- New `src/workstations/beacon_entity_nbt.rs`.
- New `src/workstations/beacon_entity_data.rs`.
- Three mechanical declarations in the owned `src/workstations/mod.rs`.
- New `docs/reference/workstations-beacon-entity.md` and this status file.

No Main, shared BlockEntity/CompoundTag/registry/Component/actor/world/save
implementation, public status/checklist, manifest, journal or deployment
configuration changes. Existing scan/tick/controller/staged APIs remain intact.

## Implemented native behavior

`BeaconBlockEntity<T,L,M,E,C>` owns the actual subclass fields around the shared
`NativeBlockEntityBase`, reusing `BeaconScanState`. `BeaconClass` retains actual
class effect constants and the single DEFAULT_NAME component handle. A required
canonical type/fresh-base factory constructs the inherited owner; observations
of equal fields do not certify identity. The live object is not Clone.

Native UTF-16 `BeaconLockCode` preserves the NO_LOCK singleton versus a fresh
empty but equal record. Name/implicit-component operations, setLevel attachment
before height query, sound-before-removal, and shared canUnlock/menu-factory
phases preserve source order and completed failure prefixes. Real custom-name
getString/component and player/menu owners remain required.

NBT methods accept decoder-issued `SourceCompoundView` and use actual compound
mutations, including `NativeFreshCompoundOwner`. Additional, stored-component
and metadata wrappers preserve their distinct phases. Loading never assigns
Levels; missing/wrong-typed name preserves its old reference; Lock replaces
unconditionally. Omitted save values retain old target keys. Update tags use
custom-only saves. Read-only NBT services do not claim arbitrary callback reentry.

`beacon_entity_data::{data_count,data_get,data_values,set_data_live}` borrows
the same actual fields through `BeaconEntityDataHost`. Primary sound precedes
decode/filter/assignment, retaining original Level/position arguments and field
prefixes across callback reentry, removal/replacement/reattachment and errors.
It neither stages actor graphs nor rolls back or retries completed callbacks.

## Validation and primary evidence

Frozen source SHA-256 values:

- Core: `74e3a438d664a8f6483fed01474baf1d0b7cf9201e37bf5cf698e0cbad275e36`.
- NBT: `c3aeb4167ccfdb7d996d69ee1224ae4c29b3f36eb49301fd121768e875384abb`.
- Live data: `0ee2914fe2cf8c59d68d5934b2ff1a6404b6af8731f957aa2fab56d8c34f20a7`.

Locked offline all-target compilation passed in 1m26s. The focused native
class/lifecycle, NBT and live-data flows passed. Independent pinned-primary and
API/integration reviews found no concrete blocker; formatting/diff checks pass.
These use real shared native base/EMPTY/decoder/compound algorithms with
explicitly controlled class/holder/name/Level/callback fixtures. They do not
establish an actual configured world, effect application or Main Beacon flow.

Direct pinned JDK21 javap supplied method evidence, not REA. Root also executed
unchanged original constructor/additional-NBT methods with the real bootstrap
and registry lookup, without creating a Level/server/world. The observed
DEFAULT_NAME identity, Levels/name/Lock behavior and omitted-save keys match
the source facts; this is not gameplay or full persistence evidence. Exact
artifact/helper/observation hashes and source methods are in
[the reference ledger](../../reference/workstations-beacon-entity.md).
No Java artifacts, disassembly, raw logs or private saves are committed.

## Required central contracts and call phases

There is no new production Main caller. Central must supply the canonical
BEACON type/factory, one retained class owner, actual effect/name/component
services, and retained world BE index/ticker/checkpoint. Tick adapters must
borrow these same subclass fields through the existing tick_live boundary.
Attachment/removal must invoke the subclass overrides, not only raw base
setters. Real sound uses the existing Level sound RNG, not another stream.

The stable source anonymous ContainerData(this) object must retain this same
BE in the actual arena/menu factory. No empty identity token replaces it. The
generic update-packet factory must resolve actual Level.registryAccess before
position/type/update-tag capture; this source stage supplies no fabricated
packet. Nonempty shared component codecs remain unsupported by the existing
EMPTY-only service and require their actual type/value codecs.

The old Beacon payment/menu controller's detached atomic domain does not
automatically admit these live callbacks. Either prove its restricted domain
against the actual owners or supply a separate ordered/live menu/payment path.
Do not call live actor/data operations inside an uncertified detached image.
Team4 retains actor/effect definitions and arena work; actual criterion,
world/sound/audience and one ordered packet-publication owners remain central.

Observed maximum capacity and this stage's actual peak are seven total slots.
All six source/review/documentation assignments completed; root alone now
publishes and verifies. No automatic work or cross-chat polling is guaranteed
after a turn stops. Aggregate inventory.4/crafting.2/crafting.5 remain partial;
fixed checklist definitions and metrics are untouched.
