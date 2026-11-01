# Java player.dat interoperability, bounded1.21 implementation

`player_nbt.rs` reads and writes gzip-compressed named NBT for DataVersion3953.
Older versions return a typed unsupported-version issue. The importer never
turns a failed existing save into a newly constructed player.

The source is the unmodified official1.21 inner server JAR, SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.

Read-only bytecode inspection used Entity(`bsr`), LivingEntity(`btn`),
Player(`cmx`), ServerPlayer(`aqv`), Abilities(`cmu`), FoodData(`cpq`),
Inventory(`cmw`) and AttributeInstance(`bus`). Current1.21 tags are lowercase
`attributes` and `active_effects`; attribute records use `id`, `base` and
`modifiers`. Legacy capitalization must not be mistaken for these fields.

The read-only `player_nbt_attributes.java` bootstrap extractor calls the actual
`Player.createAttributes()` and `AttributeSupplier.Builder.build()` methods,
then records28 actual Player base values in `fixtures/player-nbt-attributes.json`.
Unknown attributes, nondefault bases and nonempty permanent modifiers produce
typed unsupported issues. Absent attributes mean the original Player defaults.
Active potion effects require their actual runtime owner and are rejected.

The native fields include position/rotation/on-ground, motion, inventory and
armor/offhand, hotbar selection, health/air/frozen/absorption/score metadata,
food, experience, abilities, dimension, fire/fall/death/sleep values,
recipe-book compound and current-version statistics. A loaded velocity axis
whose absolute value exceeds10 becomes0, matching Entity.load's original
guard. The native world's narrower coordinate bound is checked explicitly.

NBT inventory indices0..8 map to native36..44;9..35 retain their numbers;
armor100..103 map to8..5; offhand150(byte-106) maps to45. Unknown inventory
indices and unowned record fields are retained in the original source.
Nonempty cursor or2x2 crafting/result slots must pass through the real menu
close path before Java export.

The native lifecycle carries optional original gzip and statistics JSON.
Unknown root fields, nested ability/recipe-book fields, known-slot extra
fields and unknown statistic keys survive export. An unchanged imported native
record reuses the original gzip bytes. Unknown data is retained as source data;
it is not advertised as an implemented gameplay owner.

Java statistics import admits only known current-version entries to the live
counter map, retaining unrecognized names in the original JSON. Each player.dat
and statistics file is encoded before replacement and replaced atomically.
There is no promise of a transaction spanning both files, matching their
separate Java persistence boundaries.

Full vanilla effects, modified attributes, every item component, DataFixer
upgrades, vehicle/portal lifecycle and whole-world Java/native interoperability
remain outside this implementation. Main integration and the joint compiler
result are owned by the parent agent.
