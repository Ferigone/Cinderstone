# Prepared integration queue message

Recipient: integration-root-20261101 / issue #1.
Assigned team issue: https://github.com/Ferigone/Cinderstone/issues/3.
This is a prepared message, not a successfully posted comment or remote handoff.

Instance: ws-20261101-7c2f; state: ready_for_integration for bounded native code.
Source: `7147da02c737360682386df4d73792cb8b03ae00`; feature branch
`teams/workstations/ws-20261101-7c2f/menus` is still local because writes fail.
Focused native flows and full Main build passed, including separate exact
source overlays on masters `404a5da` and `f40d8bf`. No Main invocation or complete
inventory.4/crafting.2/crafting.5 is claimed.

Please prioritize a bounded stonecutter/smithing Main stage, implemented
centrally or assigned through an exact narrow patch:

1. Opening: bind temporary menus to actual player, Level and block position;
   allocate window; send open before initialize. These stations do not need
   invented block entities. WorkstationAccess must check the actual block and
   player reach with additional range 4.0.
2. Recipes: supply StonecutterRecipes with actual enabled features and result
   description-ID order; supply SmithingRecipeOrder with distinct constructor
   encounter and result-selection orders. Keep actual registry/component owners.
3. Actions: route stonecutter prepare_button/prepare_click and smithing
   prepare_click using the same borrowed canonical inventory, actual creative
   state and world game time. Validate plans before and after preparing actual
   callbacks, commit once and publish ordered listeners/packets/indexed drops.
   Preserve smithing's broadcast-before-result-recompute boundaries and the
   stonecutter callback before a failed result shift insertion.
4. Lifetime: connect actual player-phase broadcast/validity and close,
   disconnect/save context, close-before-return/drop publication and old shared
   baseline transfer. Drop only owned cursor/inputs through real Item/RNG/ID
   owners. Derived result previews are not owned close drops.
5. Output: use the coordinator's existing single ordered chat/session/admin/
   disconnect stream. Workstation packet publication must not create an
   independently ordered connection output channel.

Central dependencies are active-menu dispatch, shared inventory/counter,
actual item/stat/recipe/advancement callbacks, Item/drop transactions and the
current output sequence owner. Team 4's survival/equipment grant stays separate;
initial integration can retain the supported alive creative profile while
actual death/effect contexts remain explicit. Team 2 has no direct generation
implementation dependency for the bounded native flat-world stage. No unseen
external team or automatic cross-chat wakeup is assumed.

Bounded acceptance: open both stations, select/craft/shift transfer, invalidate
actual block and close later in player phase, disconnect with owned-item
conservation, and observe source-ordered state/cursor synchronization. This is
still not aggregate row completion.

The [integration ledger](workstations-integration.md), per-family source notes
and [team status](../coordination/status/ws-20261101-7c2f.md) retain exact APIs,
source identities, commits, validation and later-family contracts. No source
archives, credentials, Java artifacts, metrics or deployments are included.
