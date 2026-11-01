# Loom and cartography reference ledger

This is a bounded native state/assembly port, not a complete menu or Main
integration claim. No checklist definitions or counts change.

## Inspected source

Primary evidence is the original Java 1.21 inner server JAR, SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, and
matching official server mappings, SHA-1
`31c77994d96f05ba25a870ada70f47f315330437`. The methods below were inspected
with JDK 21 `javap -p -c`. REA was unavailable; this is static bytecode
inspection, not an observed vanilla menu execution. No Java artifacts or raw
disassembly are included.

- `LoomMenu` (`cra`), `slotsChanged` lines 166–214,
  `getSelectablePatterns` 151–157, `clickMenuButton` 142–147,
  `setupResultSlot` 288–303, `quickMoveStack` 230–278, `removed` 283–285;
  anonymous input/output containers `$1/$2` and result slot `$6`.
- `CartographyTableMenu` (`cqb`), `slotsChanged` 111–120,
  `setupResultSlot` 123–153, `quickMoveStack` 162–209, `removed` 214–218;
  anonymous input/output containers `$1/$2` and result slot `$5`.
- `MapItem` (`cvc`), `onCraftedPostProcess` 338–346,
  `scaleMap` 349–356 and `lockMap` 359–366; `MapPostProcessing` (`cxt`).

## Loom behavior and ownership

`LoomState` holds the three owned temporary inputs and one derived result. The
caller transfers input ownership with `set_input`; it receives the previous
owned input. Banner/dye input rules follow the pinned vanilla item classes via
canonical item IDs; template membership comes from the actual registry host.

The host supplies the captured banner-pattern registry's tag encounter order.
An absent template uses `no_item_required`; a `BannerPatternItem` uses its own
tag; absent tags give an empty list. The port does not invent registry contents.
The standalone selected DataSlot initially contains zero; the first empty-input
change sets it to -1. A single selectable pattern is selected automatically; otherwise the previously
selected holder is relocated by holder identity, or selection resets to -1.
Normal input changes reject a sixth existing layer by clearing result/selection.
Source button handling itself checks list bounds and calls `setupResultSlot`;
it does not repeat that layer limit. The port preserves that distinction.

The result copies exactly one banner, retaining its existing components and
layers, then appends the selected real pattern holder and the dye's color. A
successful take consumes one banner then one dye, with each change recomputing
the result; the pattern template remains owned and unconsumed. There is no
recipe/advancement callback fabricated for this non-recipe workstation. Its
sound is at most once per actual level game-time tick (the initial last-time
field is zero), after input consumption and before the base result-slot callback.

The protocol MenuType ID is 18. Menu slots are inputs 0–2, result 3 and shared player slots 4–39. Shift result
moves to the player inventory in reverse order. Input transfers use forward
order. Player banners/dyes/templates target only their matching input; other
items move between main inventory 4–30 and hotbar 31–39. A matching full input
does not cause an invented fallback transfer.

## Cartography behavior and ownership

`CartographyState` holds the map/addition inputs and a derived result. Input
rules admit filled maps, and paper/empty maps/glass panes respectively. Map IDs
come from the item's component; scale and locked state must come from the actual
captured world's saved-map storage through `CartographyMaps`. An unavailable
owner is an error, distinct from a genuine lookup that finds no saved map. The
original setup method immediately returns on missing saved data and retains its
current preview; the native state does the same.

Paper copies one map with `MapPostProcessing.SCALE` (component 28 value 1), only
when unlocked and below scale 4. Cartography does **not** reject exploration maps
in this branch. Glass panes copy one unlocked map with `LOCK` (value 0). Empty
maps copy two maps even when the saved map is locked. Other combinations clear
the result.

Taking a result removes one map and then one addition, calls the actual item's
`onCraftedBy`, then emits the tick-guarded sound and base slot callback. The
`CartographyHost` contract requires the real callback: its MapItem postprocess
removes component 28, allocates a new saved-map ID and stores a scaled/locked
record when applicable. No synthetic map ID, saved record, or no-op production
host is provided. The host's outer transaction must reserve the output
destination and roll back its own errors. The native owner stages input changes
until the callback succeeds.

The protocol MenuType ID is 23. Menu slots are inputs 0–1, result 2 and shared player slots 3–38; main inventory
is 3–29, hotbar 30–38. Result shift-move is reverse, input return is forward.
The source shift-result path invokes the item callback before attempting the
inventory transfer, and its result-slot onTake invokes it again; map postprocess
removes its marker on the first call. A future click adapter must preserve those
callbacks and their real world transaction, including the failed-transfer case.
The state `take_result` implements the onTake path; it does not claim that
shift-result callback phase is integrated.

## Production integration still required

Root/menu adapters must provide actual window/state IDs, cursor, click modes,
client prediction and slot baselines, item/container slot stack limits, menu
opening, result-destination
reservation, validity against the captured block identity and player position,
player-phase broadcast/close, sounds at the real block and the base Slot callback.
Loom also needs its real registry/tag host; cartography needs saved-map mutation
and IDs. Neither host exists in Main for these new states at this stage.

On removal, the external menu's cursor callback runs first. Drain only owned
inputs in source order; the result is discarded and never returned/dropped as
another owned item. For cartography the result is explicitly removed before
clearing the inputs. These APIs alone do not implement full menu closure,
input-return overflow, disconnect handling, or all workstations.

## Focused native verification

`cargo test --locked --lib workstations::loom::tests` passed the canonical
banner/dye consumption, result ownership and same-tick sound flow.
`cargo test --locked --lib workstations::cartography::tests` passed marker
selection from saved-map context, failed-host rollback and duplicate-map input
consumption/close ownership. These are native module flows with explicit test
hosts, not original-Java execution, protocol-client observations or Main
integration proof. Existing unrelated compiler warnings remain.
