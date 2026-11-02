# Workstation component identity boundary

Target: original Java 1.21, protocol767. Primary artifacts and hashes are
recorded in [the source ledger](workstations-integration.md). Evidence here is
direct JDK21 `javap -p` inspection, official mappings and the repository's
existing original-stream normalization certificate; REA was not used.

`FoodProperties` (`cpr`) is a Java record containing an `Optional<ItemStack>`
and possible effects. Its generated record equality includes that optional
object. `ItemStack` (`cuq`) does not override `Object.equals/hashCode`.
Independently decoded nested stacks therefore cannot be identified by equal
serialized contents. The existing inventory normalization implementation
explicitly retains four prototype food overrides whose nested stack identity
the original stream certificate did not prove redundant.

`workstations::menu::require_component_context` preserves the existing proved
prototype normalization, then rejects any surviving explicit component20
override. Stable implicit prototypes and proved primitive-only prototype
elisions remain admitted. This is a bounded supported context, not an identity
implementation. No inventory codec, reference allocator or recipe owner is
introduced by the workstation team.

Shared `MenuSync` checks authoritative top/player/cursor snapshots and relevant
client predictions before comparison or baseline mutation. Alive-connected
input removal checks before inventory insertion. Direct dead/disconnected
removal does no component equality: it still transfers the exact owned cursor
and input values to ordered drops and clears their previous ownership. That
raw helper exception does not authorize a prepared controller to compare
unresolved references for stale-plan validation. Controllers which capture
whole-owner snapshots retain their explicit admission/host requirements,
including during close. A real owner must handle any unsupported removal
context without discarding its items.

The native focused flow checks explicit mushroom-stew override rejection,
proved bread prototype admission, unchanged synchronization after rejected
predictions, unchanged alive-close state after rejection, and ordered raw
dead-close ownership transfer. It is a boundary regression, not original-Java
gameplay or full custom-component compatibility.

Requested central contract: canonical component objects must preserve actual
nested reference ownership through prototype reuse and copies, and retain
distinct objects on independent decode. Wire hashes and per-menu identity
counters are insufficient. Until that owner exists, arbitrary custom food
component equality is unsupported explicitly.
