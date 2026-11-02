# Player enchantment-seed field owner

This stage ports the actual Java 1.21 `Player.enchantmentSeed` field and its
constructor, saved-data and `ServerPlayer.restoreFrom` boundaries into
`src/player_enchantment_seed.rs`. The isolated native admission stage now retains this single field in
`PlayerLifecycle`, captures it in native saves and projects it through Java NBT.
This does not implement a working native EnchantingMenu or complete an
inventory/crafting checklist delivery.

## Pinned source evidence

The original server and official mappings are those in
`fixtures/source-lock.json`: server SHA-1
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`, inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, mappings
SHA-1 `31c77994d96f05ba25a870ada70f47f315330437`. Evidence below comes from direct
JDK 21 `javap -p -c` inspection of those original classes, not REA MCP execution
or an observed Java gameplay flow. Raw disassembly stays outside the repository.

| Original owner or method | Observed seed contract |
| --- | --- |
| `Player.enchantmentSeed`, mapped `cmx.ct` | Signed `int`; ordinary negative values and zero are valid retained field values |
| `Player` constructor, source175–241 | Does not assign `ct` or sample a replacement; the JVM field initialization leaves the actual new field zero |
| `Player.getEnchantmentSeed`, source1772, `cmx.gg` | Returns `ct` directly; no lazy initialization or fallback draw |
| `Player.readAdditionalSaveData`, source845–889, `cmx.a(CompoundTag)` | Reads `XpP`, `XpLevel`, `XpTotal`, then `CompoundTag.getInt("XpSeed")`; if that value equals zero, assigns exactly one `Player.random.nextInt()` before loading Score and food data |
| `Player.addAdditionalSaveData`, source893–923, `cmx.b(CompoundTag)` | Writes the current field with `putInt("XpSeed", ct)` between XpTotal and Score; no draw or zero replacement |
| `ServerPlayer.restoreFrom`, source1415–1467, `aqv.a(aqv,boolean)` | Both keep-state branches converge at offset247, then copy `previous.ct` to `this.ct`; the seed copy is unconditional and consumes no RNG |
| `CompoundTag.getInt`, `ub.h(String)` | Accepts numeric tag type99 and calls `NumericTag.getAsInt`; missing or nonnumeric values return zero |
| `DoubleTag.getAsInt`, `uc.g`, and `FloatTag.getAsInt`, `ue.g` | Call the corresponding `Mth.floor`; floating values are not simply truncated toward zero |

The load draw is the existing player's inherited `Entity.random` (`bsr.ah`,
`RandomSource.nextInt`, `ayw.f`). It is not `Level.random`,
`Level.threadSafeRandom`, `Math.random` or a newly seeded temporary stream.
If the single draw returns zero, the field remains zero; there is no retry.
`restoreFrom` does not apply the saved-data zero replacement.

## Callable native owner

`PlayerEnchantmentSeed` has a private signed value and no `Default`. Its API
keeps the source operations distinct:

- `new_player()` creates only the genuine new-Player constructor state, zero.
- `from_saved_value(value)` passively retains an actual parsed saved field; it
  neither constructs a new live player nor applies the saved-zero draw.
- `get()` reads the actual field, including zero, without touching randomness.
- `field_mut()` lends that field to the separately ported
  `player_enchanting::on_enchantment_performed` body. The caller must borrow
  the same actual player's XP state and RNG; a detached integer is not a live
  seed owner.
- `load_xp_seed(source_value, player_random)` takes the actual saved
  `CompoundTag.getInt` result and applies its one conditional unbounded draw.
- `read_additional_save_data(source_compound, player_random)` combines that
  operation with the actual NBT source getter. A non-compound source is an
  explicit error before either seed or RNG changes.
- `add_additional_save_data(target_compound)` writes the actual current Int
  field, preserving unrelated compound entries through the existing NBT owner.
- `restore_from(previous)` performs the unconditional copy without a flag or
  RNG argument that could accidentally gate or resample it.

The pure `read_xp_seed_value(source_compound)` exposes the original NBT getter
without loading any player or sampling RNG. Byte/Short are sign extended, Long
narrows to the low signed32 bits, and Float/Double use the existing native
Java-floor operation. The source NBT getter's missing/nonnumeric zero result is
distinct from supplying zero for an absent native runtime owner.

## Canonical native lifecycle and migration

`PlayerLifecycle.enchantment_seed: Option<PlayerEnchantmentSeed>` is the one
actual canonical field. Its transparent serialized representation is an optional
signed integer. Serde absence or null means UNKNOWN; there is no serde default
that constructs a new zero field. `PlayerLifecycle::new`, used only for a genuine
new player, explicitly supplies `Some(PlayerEnchantmentSeed::new_player())`.
Existing native-format version1 records remain readable with an absent field.
No persistent format or checklist denominator is redefined by this extension.

`require_enchantment_seed` and its mutable equivalent return the actual owner
or an explicit error. Unknown seed ownership does not reject ordinary admission
or replace inventory, XP or any saved world/player data. Future enchanting
opening and mutating callbacks must require this owner; they may not infer zero
from absence or consult stale source NBT in a getter.

The passive Java importer projects `CompoundTag.getInt("XpSeed")` through
`read_xp_seed_value`; even a genuinely imported compound without that key has
the original getter's known zero result. It consumes no player RNG. Java export
writes the known canonical field, including zero, over preserved original NBT.
A later changed native field therefore wins over any old `java_source_nbt` seed.
`PlayerData::capture` and native serialization carry this same lifecycle field.

For an old native record whose field is absent, the admission helper may project
only an actually present XpSeed entry in preserved valid Java source NBT. It
never clobbers an already known native field, including zero. No native field
and no actual original entry means UNKNOWN. An unreadable retained provenance
also remains unknown and is logged; this seed-only migration issue does not
block otherwise supported ordinary play. Passive store scans and Java import
never perform a replacement draw.

## Actual admission and restore ordering

`main_player_data::EnchantmentSeedAdmission` explicitly distinguishes three
source operations:

| Admission origin | Seed and RNG behavior |
| --- | --- |
| `NewPlayer` | Require the genuine known constructor zero; consume no seed draw |
| `SavedData` | After optional provenance migration, load the actual known saved value; zero consumes one unbounded sample from the actual local Player RNG, nonzero consumes none; unknown stays unknown |
| `RestoredPlayer` | Retain the authoritative previous-player snapshot's field unconditionally, including known zero or unknown; perform no saved-data replacement draw |

Main first obtains the actual retained player RNG from
`random_seed::create_player_random`, after the inherited UUID's two nextLong
calls. It selects pending, native or genuine Java saved data and applies the
existing actor/inventory/lifecycle observations. Its three constructor Math
samples belong to the separate real global Math owner. Before registration or
any gameplay callback, Main invokes `admit_enchantment_seed` on its mutable local
Player RNG. `register_runtime_player` then moves that exact advanced RNG into the
existing `players.player_random` owner. There is no replacement seeded source or
extra draw in file parsing, capture, saving or getters.

Pending authoritative player snapshots use `RestoredPlayer`. Reconfiguration
also carries an explicit in-memory `PlayerData` through `PlayExit` and the
connection loop. Cleanup removes the real InventoryMenu and external menu,
saves, then captures the post-removal player state under the same admission,
world and player locks before actor removal. The next PLAY admission receives
that snapshot directly; it does not reopen the just-written disk record and
mistake known zero for a fresh saved-data load. Failed configuration still leaves
the normal persisted snapshot available for a later genuine login.

The seed copy is independent of XP/inventory retention choices, matching the
unconditional `ServerPlayer.restoreFrom` seed assignment after its keep-state
branches converge. A normal later disk admission is a genuine saved-data load:
a stored known zero then follows the one-draw rule. The old record's lack of
known seed ownership never becomes a fresh-player constructor call.

## Actual experience-packet cache binding

The existing `MainExperienceRuntime.last_sent_experience` map remains the sole
native owner of the ServerPlayer lastSentExp field. Actual runtime registration
initializes its real entry to the pinned constructor value **-99,999,999**
(`aqv.cR`, constructor offsets67–70). This differs from enchanting's later
**-1** invalidation. `ServerPlayer.restoreFrom` also unconditionally assigns
-1 to this cache at offsets286–288, after its seed copy; `RestoredPlayer`
admission performs that assignment through the same actual map entry.

`last_sent_experience_mut(entity_id)` returns a fallible mutable reference to
that real retained entry, suitable for the separately ported ServerPlayer
`onEnchantmentPerformed` override. It does not return a temporary, create a
second map or send a packet. The ordinary experience tick compares/updates the
same map; actual player removal removes its entry. An absent runtime entry is
an explicit owner error.

## Validation boundary

Direct pinned-source inspection and focused whitespace/diff inspection cover
this isolated source stage. A second reviewer independently checked the numeric
NBT conversion and the constructor/load/save/restore source contracts. Focused
`cargo check --locked --offline --lib --bin mc121-rust` passed in a separate
private target directory on isolated base
`17f1fa6f73c3fed1688d8403e03a0507f6bcb549` (23.22s first check; 3.25s after the
source-correct restoreFrom cache invalidation). This compiles the actual Main
admission/persistence/cache paths and the three newly exported core owners.
No broad tests or live admission/enchanting flow were run by this stage;
focused runtime admission/reconfiguration/save observations remain required.

The native field, persistence and actual local RNG binding are now source-wired;
this does not establish a production EnchantingMenu, complete inherited Java
player loading, arbitrary respawn policies, or whole-workstation compatibility.
No performance or checklist-completion percentage follows from this stage.
