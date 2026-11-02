# Ordered Beacon menu method handoff

Instance: `ws-20261101-7c2f`; WORKSTATIONS; state: implementing/verification.
Assigned issue: [#3](https://github.com/Ferigone/Cinderstone/issues/3).
Exact [claim](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6569695292)
and [source correction](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6569720542).
Branch: `teams/workstations/ws-20261101-7c2f/beacon-menu-live`.
Initial dependency base: published PR120 head
`b72f205236dc1b049afd4b6241d73fa2d67f6274`, now contained in master through
actual merge `3da06b2ff048932438e2ae311a7db84b27fd6188`.
Current master is separately verified before publication/merge. No master merge
or force push is performed on this feature branch.

## Exact owned files

- New `src/workstations/beacon_menu_live.rs`.
- One declaration in owned `src/workstations/mod.rs`.
- New `docs/reference/workstations-beacon-menu-live.md` and this status.

Existing BeaconState, controller/entity/kernel, actual payment/BE/registry/menu
owners and all reserved Main/shared/configuration/manifest/checklist/journal
files remain unchanged.

## Concrete source methods

`BeaconMenuLiveHost` extends the retained `BeaconEntityDataHost` with actual
menu, payment Slot, DataSlot list, original ContainerLevelAccess and broadcast
services. It stores no second menu, payment, BE, world or registry object.

`update_effects_live` performs source payment check, primary encoding/setter,
secondary encoding/setter, removal of one CURRENT payment and the originally
captured Level-access blockEntityChanged callback. Primary sound may reenter
before secondary is encoded. It does not clear a captured payment snapshot,
roll back completed fields/callbacks, check tiers/creative mode, reselect the
current menu or broadcast separately. The packet handler owns its actual
current-menu/stillValid guards before invoking this source method.

`set_menu_data_live` resolves the real captured DataSlot, invokes that setter,
then broadcasts once. Invalid indices fail before broadcast. This differs
from raw BE ContainerData's unknown-index no-op; the initial claim's wording
was corrected against the pinned source before implementation.

## Evidence and verification

Frozen Rust SHA-256:
`67e998c7d72f7d294f3cdedb15e52c6e8f38ae7c0de5a7d7fc8ba24a7d711cb6`.
Independent direct pinned-JDK21 primary/API review found no concrete blocker.
Formatting passed. Root's compiler/native/hygiene verification and publication
receipts follow separately; they are not presumed successful by this draft.

The focused fixture uses actual shared BE/base/EMPTY/BeaconState/scan code and
controlled slot descriptors, registry, listeners, access and broadcast. Its
retained-menu marker addresses the existing PAYMENT BeaconState, not a native
BeaconMenu instance or allocated DataAccess object. It establishes no actual
menu/DataSlot/ContainerData allocation, world, packet publication or Main proof.
No Java menu/world execution is claimed. Exact source methods and artifact
identities are in [the source ledger](../../reference/workstations-beacon-menu-live.md).

## Required integration

Central must retain the actual same Menu, payment Slot/Container, DataSlot
list/ContainerData(this), BE and originally captured ContainerLevelAccess.
Root must bind real Slot.remove(1)/conditional SimpleContainer.setChanged,
blockEntityChanged, listeners/synchronizer and one ordered publication owner.
Do not redirect to the new current menu, newly attached BE Level or reused ID
after reentry. Genuine NULL LevelAccess is distinct from an unavailable owner.

The old detached atomic controller is not certified to invoke arbitrary live
methods. Prove its restricted domain or supply the real ordered/live binding;
no always-success flag, fabricated version or copied actor certifies it.
Actual stable DataAccess capture, generic packet factory, class/type/holder/name
services, world index/ticker/checkpoint, Team4 actors/definitions/criteria and
Main integration remain the central contracts from PR120.

Actual maximum capacity is seven total. This small stage used root, one source
author, one primary/API reviewer and one ledger author (peak four). All three
subordinate assignments completed; root alone verifies and publishes. No
automatic background coding or cross-chat polling follows a stopped turn.
No aggregate inventory.4/crafting.2/crafting.5 completion or metric change.
