# Native 1.21 recipe-book integration

`src/server_recipe_book.rs` is ready for integration and is exported by `lib.rs`. The final source passes `cargo check --locked --offline --lib`. Three focused comparisons passed before the final small logger/seen/settings adapter additions; the final broad differential/release pass is deferred at the user's request to prioritize porting and integration.

## State and packets

Store one `ServerRecipeBook` per player UUID. `ServerRecipeBook::default()` starts with empty known/highlight sets and eight false settings; it does not grant every recipe. Its `known()`, `highlights()` and copied `settings()` expose current state.

| Operation | API | Behavior |
| --- | --- | --- |
| Vanilla declarations | `VanillaRecipeCatalogue::get().declarations_packet()` | CB `0x77`, actual natural normal-login body, 108,270 bytes / 1,290 recipes. Send before book INIT. |
| Initial book | `book.initial_packet().encode()?` or `book.send_initial(host)?` | CB `0x41`, state INIT=0, known IDs, highlighted IDs and all settings. |
| Unlock | `book.add_recipes(&ordered_descriptors, host)?` | Skips currently known IDs and actual special holder values; adds known/highlight, calls required synchronous recipe-unlocked hook, then sends ADD=1 only for a nonempty result. Returns the actual count. |
| Remove | `book.remove_recipes(&ordered_descriptors, host)?` | Removes known and highlight for currently known IDs; always sends REMOVE=2, including an empty result. |
| Settings packet | `RecipeBookSettingsChange::decode_exact(payload)?`; `book.apply_settings_change(change)` | Type VarInt, open boolean, filtering boolean. No immediate reply. |
| Seen packet | `RecipeBookSeenRecipe::decode_exact(payload)?`; `book.seen_recipe(&seen.recipe, lookup)?` | Resolves the current recipe manager first; removes highlight only for an existing holder. Unknown manager IDs do nothing. |
| Save | `book.to_nbt()` | Compound to put under player NBT `recipeBook`. |
| Load | `book.load_nbt(compound, lookup, log)?` | Replaces settings and merges recipe/highlight IDs. Invalid/unknown entries invoke explicit synchronous typed log callbacks. |
| Copy | `book.copy_from(&other)` | Copies distinct objects; safe Rust excludes the original destructive self-alias case. |

`RecipeDescriptor { id: RecipeId, is_special: bool }` describes the actual current holder value. `is_special` must not be guessed from the ID. The default catalogue provides all 1,290 actual original descriptors, including 14 special recipes. `RecipeLookup::lookup(&mut self, &RecipeId) -> Result<Option<RecipeDescriptor>>` is the current-manager boundary; closures implement it too. `RecipeId` preserves actual empty namespace/path behavior, including `""` and `":"` normalizing to `minecraft:`.

`RecipeBookHost` has two required methods and no default advancement no-op:

```rust
fn recipe_unlocked(&mut self, book: &mut ServerRecipeBook,
                   recipe: &RecipeDescriptor) -> anyhow::Result<()>;
fn send_packet(&mut self, book: &mut ServerRecipeBook,
               packet: RecipeBookPacket) -> anyhow::Result<()>;
```

Both callbacks run at the original mutation points. Recipe rewards can reenter this same book. Precomputing/deduplicating unlock inputs or deferring awards changes behavior. A hook error preserves prior mutations, as the original thrown callback does; validate unsupported source/advancement capabilities before starting the operation. The send hook encodes with `packet.encode()?` and sends to that player. `RecipeLoadLog::issue` receives `RecipeLoadIssue`; its `message_template()` and lossless `argument()` expose original logger arguments.

## Source capabilities and remaining integration

The static declaration packet is supported only for the pinned vanilla 1.21 recipe/tag dataset. It was captured twice from unchanged original normal-login resource loading, then decoded/reencoded byte-for-byte by actual original `agc/czb` STREAM codecs. Its payload SHA256 is `bd4193d33199e1850fe20f115cc7c02be824a48bd7635603ba7e1ef353dfe6ce`. This is static data, not a dynamic datapack recipe serializer. Controlled sorted-tag declaration data is a separate fixture: its ordering and 81 holder payloads differ, so it is never substituted for natural bytes. Imported/custom/reloaded recipe/tag sources require their real manager and declaration serializer or an explicit unsupported error.

Known/highlight are logical sets. The native core serializes them in canonical sorted order for INIT and NBT. It preserves literal ordered IDs and duplicate occurrences for ADD/REMOVE and explicit packet construction. Exact Java HashSet iteration bytes for multi-ID INIT/NBT are not implemented or claimed. One original-only adversarial self-copy history is explicitly excluded from the safe Rust API.

This module provides per-player state, settings, transitions, packets and NBT. It does not register/evaluate vanilla advancements or execute reward functions. In particular, existing `inventory_changed` vanilla recipe advancements can match ordinary stone; the module does not imply default recipe unlock integration. Main still needs the explicit source guard, player book lifecycle, login declarations-before-INIT, serverbound handlers and a real synchronous advancement host. Dynamic datapack reload, complete player NBT persistence and full ServerPlayer gameplay remain separate.

Original transition evidence: 2,484 histories / 14,016 snapshots / 6,918 packets; native pre-final smoke covers 14,014 snapshots and 6,917 packets, excluding only the two-snapshot invalid self-copy history. It also covers 31 malformed codec cases and the actual 1,290-recipe catalogue/natural declaration body. The source ports for the final serverbound wrappers are in `server_recipe_book_serverbound_bytecode.txt` and `server_recipe_book_listener_bytecode.txt`; their final differential smoke is deferred.
