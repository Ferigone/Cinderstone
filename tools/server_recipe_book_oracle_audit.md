# Original 1.21 recipe-book oracle

This oracle executes the shipped `ServerRecipeBook` (`avu`), `RecipeBook` (`avs`), `RecipeBookSettings` (`avt`), and `ClientboundRecipePacket` (`aee`) methods. It uses the pinned inner server JAR, SHA-256 `8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`, under JDK 21.0.13. The Java sources are test harnesses; they are not a server runtime dependency. This report describes original-class evidence. The native module has a separate comparison report.

`fixtures/server-recipe-book-vectors.json` contains 2,484 histories, 14,016 operation snapshots, and 6,918 packet snapshots. Its SHA-256 is `12f6491521064c4d35fbc4aee90aeb835beb8545f89937bd7ba9c7ff6bc27ac2`. The original replay also verifies 1,290 recipe facts, including 14 actual `Recipe.isSpecial()` results, and a separate 31-case malformed-codec corpus. Every tested recipe-book operation leaves the controlled level RNG unchanged.

## Execution and boundaries

The harness calls the actual `Recipe.CODEC` through `RecipeManager.apply` on all 1,290 recipe JSON resources extracted from the JAR. Ingredient tags in this controlled bootstrap use the prior crafting fixture's expanded item-ID-sorted membership. That establishes recipe existence and actual `isSpecial` behavior; it does not establish natural Vanilla tag order.

Original `avu`/`avs`/`avt`/`aee` classes are never transformed. ASM creates outer environment subclasses for the player, connection, world, server, and advancement award boundary. A controlled forwarding `dt` subclass observes the original recipe predicate's captured `RecipeHolder`, then invokes the original `dt`/`dv` protected implementation. The static `an.g` environment reference points to this observer only inside the oracle process. Registered-listener tests execute original criterion matching and LootContext construction; the final `PlayerAdvancements.award` is an explicitly observed outer callback. The harness does not implement default advancement rewards, registration, persistence, or dynamic datapack trigger serialization.

Snapshots record known/highlight sets, all eight setting flags, logical and serialized NBT, exact packet payloads, return/error values, synchronous callback state, external settings copies, and level RNG state. Packet IDs come from the original `GameProtocols` visitor: recipe-book packet 65 (`0x41`), declarations packet 119 (`0x77`). Payload fixtures exclude packet ID and network framing unless a field explicitly says otherwise.

## Observed transitions

- `addRecipes` checks current membership and the actual holder value's `isSpecial` flag at each iteration. It adds known and highlight entries, appends the outgoing ID, invokes the recipe-unlocked trigger, then increments its return counter. A nonempty outgoing list produces ADD after the loop. The bytecode is `avu.a(Collection, aqv)`, offsets 44–125.
- Callbacks run before the next input item. Removing the current ID in the first callback lets input `[a,a]` add twice: return 2, literal ADD `[a,a]`. Reentering add with the current ID produces no nested ADD. Reentering with a different ID sends its nested packet before the outer ADD. A callback failure preserves preceding mutations and suppresses the unfinished outer ADD.
- `removeRecipes` removes known and highlight entries only for IDs currently known, preserves collection order in its outgoing list, and sends REMOVE even when that list is empty. See `avu.b(Collection, aqv)`, offsets 44–85.
- Known and highlight sets are independent. Highlight-only special recipe IDs are permitted. Direct known insertion through a holder filters special values. Holder equality uses ID alone; specialness comes from the holder value, including explicitly constructed alias holders.
- `fromNbt` replaces settings, then merges known and highlight lists into existing sets; it does not clear the old sets. Known insertion filters special recipes, highlight insertion does not. Unknown and invalid IDs log and are skipped. Loading an empty compound resets settings but retains existing sets. See `avu.a(ub,czd)`, offsets 0–49 and its private list loader.
- `getBookSettings`, sent book settings, and distinct-object copy operations copy all four type records. Later settings changes do not alter earlier packet objects. The one Java-only self-copy adversarial case clears state and throws on a partially cleared settings map; safe Rust borrowing need not expose that alias operation.

Resource ID cases include empty namespace/path, repeated separators, uppercase, spaces, NUL, accented characters and isolated UTF-16 surrogates. NBT settings cover all six numeric tag types, low-byte wrapping, negative fractions, signed zero, subnormal values, infinities, NaNs, and wrong tag types. For example, actual Int/Long 256 is false while Float/Double -0.5 is true. Values are captured from original methods, not calculated by the generator.

The separate codec fixture confirms Java's unused high fifth-byte VarInt bits alias state, collection count, and string length. It also records sixth-byte overflow, truncated fields, negative counts/lengths, and trailing data. These are output-codec semantics; a native decoder may require complete consumption as an explicit wrapper policy.

## Ordering and NBT

ADD/REMOVE and explicit ordered packet-constructor inputs compare literal IDs and original STREAM bytes. INIT packets emitted from Java HashSets compare logical sets; exact wire comparison is retained only when each set has at most one ID. The same policy applies to serialized NBT recipe/highlight lists. This avoids requiring Java HashSet iteration order in the native representation.

The actual ten top-level NBT keys are captured in `server-recipe-book-facts.json` and checked separately. Empty Java `ListTag` values serialize element type 0; nonempty recipe lists use string type 8. NBT fixtures retain original named-root `NbtIo.write` bytes.

## Natural declaration prerequisite

`check_java_recipe_declarations.py` launched the unchanged original outer JAR in a content-verified full copy of the canonical world and captured play packet `0x77` during an offline no-action login. It retained compressed framing, decompressed packet bytes, and the body separately. Source JARs and canonical world stayed unchanged; the server shut down cleanly. Two independent captures produced identical raw fixtures.

The natural declaration body is 108,270 bytes, SHA-256 `bd4193d33199e1850fe20f115cc7c02be824a48bd7635603ba7e1ef353dfe6ce`. `server_recipe_declarations_check.py` decodes it using actual original `agc.STREAM_CODEC`, checks all 1,290 IDs/classes/special flags, and reencodes byte-for-byte with zero unread bytes. Each actual `czb.STREAM_CODEC` holder payload is hashed independently. Compared by recipe ID with the controlled sorted-tag declaration, 1,209 holder payloads agree and 81 differ; list order also differs. The captured natural bytes are a pinned dataset, not a native dynamic recipe serializer.

## Reproduction

```sh
python3 tools/server_recipe_book_generate.py
python3 tools/server_recipe_book_codec_generate.py
python3 tools/server_recipe_book_original_check.py
python3 tools/server_recipe_book_load_logs_generate.py
python3 tools/check_java_recipe_declarations.py
python3 tools/server_recipe_declarations_check.py
```

The first two commands regenerate the transition fixtures. `server_recipe_book_original_check.py` replays frozen fixtures without replacing them. The log tool is a separate forwarding logger-boundary corpus. The natural capture starts an original Java server and changes only its private world copy; it has no dependency on native Rust or Cargo. Primary reports are `evidence/server-recipe-book-original-oracle.json`, `evidence/server-recipe-book-load-logs.json`, `evidence/server-recipe-book-natural-declarations.json`, and `evidence/server-recipe-book-declarations-stream.json`.
