# Loom take sound and default listener: original 1.21 evidence

The pinned original Minecraft Java **1.21**, protocol **767**, registers
`SoundEvents.UI_LOOM_TAKE_RESULT` as `minecraft:ui.loom.take_result`, raw
sound-registry ID **1429**. This is the raw registry ID, not a manually shifted
wire-holder value. The actual original field `avp.At` is the same object returned
by the original sound registry. Its fixed range is absent; volume 1 has range 16.
`SoundSource.BLOCKS` is actual enum ordinal 4. Source volume and pitch are both 1.

These facts are recorded in [loom-production-sound.json](../../fixtures/loom-production-sound.json).
The bounded original bootstrap query completed with exit 0 and directly observed
the resource location, raw ID, field/registry object identity, absent fixed range,
range 16 at volume 1 and BLOCKS ordinal 4. It also successfully invoked the actual
private static constructor-default listener target. No Main Loom flow or sound
packet delivery is claimed by that query.

## Pinned source and methods

The inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
official mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
Both were verified before the fresh inspection. The original server and mapping
acquisition are pinned by [source-lock.json](../../fixtures/source-lock.json).
Inspection used **direct JDK 21 `javap -p -c`**, constructor bootstrap metadata and
bounded original-class bootstrap execution. It was not an REA MCP execution.
No Java binary, raw disassembly or private runtime output is published here.

| Original mapped owner | Evidence used |
| --- | --- |
| `LoomMenu` (`cra`) | Constructor lines 36–133, `lastSoundTime` field `y`, default listener field `s`, `registerUpdateListener` lines 225–226, `lambda$new$0` line 38 |
| `LoomMenu$1` (`cra$1`) | Input `setChanged`, lines 51–54 |
| `LoomMenu$2` (`cra$2`) | Output `setChanged`, lines 60–62 |
| `LoomMenu$6` (`cra$6`) | Result `onTake`, lines 102–118; captured-level sound lambda, lines 109–114 |
| `Slot` (`crq`) | Base `onTake`, lines 45–46, calls the actual slot's `setChanged` |
| `Level` (`dcw`) | BlockPos sound overload lines 403–404; floating-coordinate sound overload lines 423–424; independent `threadSafeRandom` field `f` |
| `ServerLevel` (`aqu`) | Seeded positional sound lines 974–975, creates `ClientboundSoundPacket` and calls actual PlayerList broadcast |
| `SoundEvent` (`avo`) | Variable range calculation lines 65–68 and fixed-range observation line 72 |
| `PlayerList` (`aur`) | Actual dimension/exclusion/distance checks in positional broadcast |

## Actual result-take order and time guard

`LoomMenu$6.onTake` removes one banner first, then one dye, through their real
slots. Each removal retains its input-container callbacks. The pattern-template
slot is not consumed. If either required input is now empty, selection is set to
-1. The captured `ContainerLevelAccess` then invokes the sound callback, after
which the original base result `Slot.onTake` runs. No crafted-item statistic,
recipe-book award, advancement or XP effect is introduced by this source method.

The callback reads the captured level's **actual game time** and compares it for
equality with this menu's `lastSoundTime`. The long field begins at Java's default
**0**; the constructor does not set a different value. Equal times suppress the
call, including a first take at game time 0. Any unequal signed time invokes the
sound and only then records that time. This is equality, not an increasing-time
or wall-clock condition; moving the actual level time backwards does not imply
suppression when the value differs.

The update occurs after `playSound` returns. Native preparation must not consume
RNG or move the sound to a later generic tick. A retained `TakeSound` event
represents the actual source boundary after input-consumption callbacks and
before base Slot.onTake, not another owned item or recipe callback.

## Sound origin, audience and RNG

The callback invokes the actual Level block-position sound overload with a null
excluded player, `UI_LOOM_TAKE_RESULT`, BLOCKS, volume 1 and pitch1. That overload
adds **0.5 to X, Y and Z**, then delegates to the floating-coordinate overload.
It does not use the integer-origin level-event path used by Smithing.

The floating-coordinate overload obtains exactly one `nextLong` from the
independent **Level.threadSafeRandom** owner before calling seeded sound. It
uses neither the player's RNG nor Level.random and makes no random pitch draw.
The original seed is obtained even if the later audience is empty. A suppressed
same-time callback makes no such draw. Item-drop construction retains its own
separate original player/level random ownership.

ServerLevel uses the real registered SoundEvent's range at that source volume
and creates the actual ClientboundSoundPacket with the given seed. This sound's
absent fixed range gives radius 16 at volume 1. PlayerList excludes only the
supplied player identity, which is null here, requires the actual same dimension
key, and compares squared distance **strictly below 256** from the centered
origin. Eligible actors are not specially excluded. Unknown dimension/audience
owners must not be replaced with a fabricated global audience.

For native integration, `main_loom_patterns` is the agreed sole Rust constant
owner for `LOOM_TAKE_SOUND_ID=1429`, backed by this fixture. The primary Loom
transaction owns the actual indexed sound-seed draw and existing ordered
publication. This document adds no Main code or alternate RNG owner.

## Constructor-default listener and callback boundary

The constructor's Runnable bootstrap target is the actual private static
`cra.r()`, mapped to `lambda$new$0`; its body is a sole return. The constructor
stores that Runnable in `slotUpdateListener`. The real
`registerUpdateListener` method can replace the field, so a no-op listener is a
proof only for the retained constructor-default capability, not arbitrary
custom/client listeners.

The input container override executes `SimpleContainer.setChanged`, then
`LoomMenu.slotsChanged`, then `slotUpdateListener.run`. The output-container
override executes its superclass change handling and then that Runnable, without
pretending to be an input recomputation. Base result `Slot.onTake` subsequently
calls its actual `setChanged`. These observable source boundaries must remain
in the menu event order even when the supported default Runnable itself returns
without effects. The original methods are not a license to discard registered
container listeners, counters or custom reentrant callbacks.

The existing [Loom menu source ledger](workstations-loom-menu.md) records slot,
input-result and removal sequencing. Configured banner-pattern/tag ownership is
a separate prerequisite, not supplied by this sound fixture. This evidence does
not certify a production Loom menu, public/graphical-client playback, all sound
boundary audiences, custom listeners or aggregate checklist completion.
