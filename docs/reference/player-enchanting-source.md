# Player enchanting XP/seed body — original Java 1.21

`src/player_enchanting.rs` ports the concrete field-update body of
`Player.onEnchantmentPerformed`, its seed getter and the ServerPlayer override.
It is a callable **partial core prerequisite**, not an Enchanting Main host or
working enchanting-table integration. It creates no player seed store, callback
observer, default RNG or alternate XP owner. Root owns mechanical export and
Main wiring.

## Source inspected

The pinned original inner server JAR has SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`; official
Java1.21 mappings have SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`.
Inspection used direct JDK21 `javap -p -c` on unchanged original classes and
the official mappings. This was not an REA MCP execution, a native runtime
observation or a full original-player gameplay probe. Java artifacts and raw
disassembly remain outside the repository.

| Original owner | Mapped source boundary |
| --- | --- |
| `Player` (`cmx`) | `getEnchantmentSeed`, `gg()`, source1772; `onEnchantmentPerformed`, `a(cuq,int)`, source1776–1783 |
| `Player` fields | `experienceLevel cq`, `experienceProgress cs`, `totalExperience cr`, `enchantmentSeed ct`; inherited actual `random ah` |
| `ServerPlayer` (`aqv`) | Override `a(cuq,int)`, source483–485: base callback followed by `lastSentExp cR = -1` |
| `EnchantmentMenu` (`cqp`) | `clickMenuButton` admission and its captured-level enchanting lambda, including callback order and creative checks |
| Player persistence/lifecycle | Constructor, `readAdditionalSaveData`, `addAdditionalSaveData`; `ServerPlayer.restoreFrom` source1415–1467 |

## Concrete field semantics

The source getter returns the actual full signed32-bit seed field. It does not
mask low bits, infer a seed, advance RNG or replace zero. Source load behavior
for a zero NBT seed is a separate phase.

The callback writes these fields in order:

1. Subtract `levels` from the actual experience level using Java signed-int
   wrapping arithmetic.
2. If the **wrapped** result is negative, write level0, then progress positive
   `0.0f`, then total0.
3. Obtain one unbounded `nextInt()` from the actual **Player.random** owner and
   store the entire result in the actual enchantment-seed field.

No minimum cost, positive-seed retry or ordinary-XP recalculation is inserted.
When the wrapped level is nonnegative, progress and total are untouched,
including the exact progress float bits. Thus this operation is not
`giveExperiencePoints`, `giveExperienceLevels`, an XP-orb grant or a level-up
sound callback. It does not call a score/criterion observer. The original
ItemStack argument is not read by this body, so the pure core does not require
a synthetic stack or a placeholder item callback.

| Input condition | Source behavior |
| --- | --- |
| Level5, payment3 | Level2; progress/total unchanged; one new seed draw. |
| Level0, payment3 | Level0, progress positive0, total0; one new seed draw. |
| Payment0 | Still updates the seed; existing progress/total are retained unless the existing level is negative. |
| Negative payment | No added rejection; wrapped subtraction may increase the level. |
| `i32::MIN` level, payment1 | Wraps to `i32::MAX`; the negative reset is not taken. |
| RNG returns zero or a negative value | Store it exactly; do not retry or replace it. |

The ServerPlayer override invokes the base method first and only afterwards
sets the actual last-sent-experience cache to -1. It sends no packet itself.
This invalidation matters because normal enchantment payment can change the
level while leaving total XP unchanged; the later player tick compares that
cache with total XP before emitting an experience packet.

## Creative caller checks and callback order

The original menu calculates the paid level count as `option + 1`; this is not
the larger displayed offer cost. Its early insufficient-lapis gate consults
`Player.isCreative()`. Its later level gate can be bypassed by the actual
abilities `instabuild` field. These are distinct source checks and must not be
collapsed into a new creative exemption inside the callback.

After the captured-level callback computes an actual nonempty enchantment
list, it calls `player.onEnchantmentPerformed(originalItem, option + 1)` even
for an instabuild player. This occurs before book conversion, enchantment writes
and the separate ItemStack lapis-consumption operation. An empty enchantment
list does not invoke the callback. The outer menu still owns its actual
ingredient, level, feature, registry and table-access admission. Calling the
new core does not implement those decisions or the remainder of that source
transaction.

The menu's offer/enchantment random stream and its Level sound random stream
are separate from **Player.random**. The new function neither constructs a
RandomSource from the enchanting seed nor borrows a menu/level RNG in its place.

## Required callable owners

```text
get_enchantment_seed(&actual_seed) -> i32

on_enchantment_performed(
    &mut actual_experience,
    &mut actual_seed,
    &mut actual_player_random,
    levels,
)

server_on_enchantment_performed(
    &mut actual_experience,
    &mut actual_seed,
    &mut actual_player_random,
    levels,
    &mut actual_last_sent_experience,
)
```

`ExperienceData` is the existing `player_data::ExperienceData`; there is no
second XP structure. Main's canonical owner is
`player_lifecycle[uuid].experience`, also used by its native XP runtime. The
existing Main XP cache is `experience_runtime.last_sent_experience[entity_id]`.
A future adapter must obtain that real mutable cache entry; a local dummy -1
does not invalidate it. The code reuses the existing `RandomSource` trait and
requires a mutable caller-supplied actual player random owner. No default
method, observer stub or implicit state allocator is introduced.

Call under the actual player transaction/locks. If the surrounding menu stages
its owners, these writes and the RNG advance must participate in that same
actual stage and commit once. This core performs no hidden clone, rollback,
packet publication or cross-owner synchronization on the caller's behalf.

## Seed constructor, save/load and copy contract still required

The inspected Player constructor does not assign `ct` or make an enchantment
seed draw; the new Java field therefore begins at its genuine constructor
default0. This fact is not permission to supply zero for an unknown, restored
or already existing native player. This core creates no player or seed owner.

The source load sequence first reads XP progress/level/total, then reads the
actual `XpSeed` int into `ct`. Only if that loaded value equals0 does it take
one actual `Player.random.nextInt()` and store the result. Zero can remain zero
if that one draw returns it; there is no retry loop. Its exact position in
Player loading matters to random ownership. Do not defer this load draw until
the player opens a table, or place it in `get_enchantment_seed`.

Source save writes the actual field as `XpSeed`. `ServerPlayer.restoreFrom`
copies the old player's actual seed without a random draw after the branches
that conditionally retain XP/inventory; seed retention is not restricted to
those XP branches. Enchanting callback refresh is another distinct seed write.

Canonical live seed registration, native save migration and NBT projection are
supplied by the separately corrected PR87 implementation. A future Main host
must borrow `player_lifecycle[actual_uuid].experience`, that same lifecycle's
known canonical enchantment-seed field, `player_random[actual_entity_id]` and
`experience_runtime.last_sent_experience_mut(actual_entity_id)` under the actual
player/menu transaction. Unknown seed ownership must fail before entering this
infallible field-update body; preserved original NBT and a temporary integer
are not current seed ownership. All those fields and the RNG must participate
in the same real stage and commit once.

The corrected admission distinction is important: a disk record or pending
native-save proposal, including same-TCP reconfiguration, loads into a new
Player and applies the saved-zero draw rule. The original reconfiguration chain
constructs a new ServerPlayer and invokes saved-data loading, not
`ServerPlayer.restoreFrom`. Only a genuinely invoked restore/respawn body uses
its unconditional no-draw copy and separate XP-cache invalidation. This module
does not choose or implement any admission path; no superseded snapshot-copy
Main code was recovered with it.

The current Enchanting core's player view also needs the original distinction
between `Player.isCreative()` at the first lapis gate and abilities `instabuild`
at the later level/consumption gates. A future Main host must resolve that
contract for divergent ability profiles rather than infer both from one flag.
No fake creative exemption belongs in the field-update callback.

This recovered stage retains the concrete source body and its required live
borrow contract. Source inspection and whitespace/diff checks passed; Root owns
mechanical export, assembly and the focused compiler check. No live enchanting
flow, comprehensive suite or checklist increment is claimed.
