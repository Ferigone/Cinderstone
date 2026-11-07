# Single-recipe commands (#211)

Permission2 players and console execute `recipe give|take <targets> <recipe>` for
one currently online player name and one retained recipe ID. Default minecraft
namespace is supported. Native Brigadier uses the original players-only entity
and resource-location wire types and named `minecraft:all_recipes` provider.
Completion uses actual online names and the existing1290 captured descriptors.
Parsing, target lookup and recipe lookup precede mutation. Unknown/non-op,
malformed and unavailable selector/UUID/offline targets change no book state.
Wildcard branches retain an explicit unsupported response: original recipe-manager
encounter order has not been preserved by the sorted catalogue. No wildcard
behavior is invented and full command parity is not claimed.

Main mutates the target's real ServerRecipeBook. Give reuses the existing
AwardHost: RecipeUnlockedTrigger/PlayerAdvancements/reward callbacks execute
before the ADD packet. Take preserves the original REMOVE packet, including an
empty REMOVE before zero-result failure. Special/duplicate grants and missing
removals fail with canonical domain feedback. Current rich target display names,
Integer count arguments and shared source/operator feedback policy are retained.
Only the target receives recipe packets. Existing player lifecycle save/rejoin
stores known/highlight state and recipe advancement progress; no second book or
tracker is added. The recipe-trigger baseline marks only this command's actual
set additions/removals, preserving another owner's pending recipe-unlocked work.

`tools/recipe_command_oracle.java` compiles with the existing
admin_feedback/say/msg oracle helpers using the [external pinned classpath](admin-feedback.md).
Run `cinderstone.oracle.recipe_command_oracle ORIGINAL_JAR
fixtures/recipe-command-vectors.json`. Original RecipeCommand/ResourceLocationArgument,
ServerPlayer award/reset methods, ServerRecipeBook and packet codecs execute.
Synthetic regular/special RecipeHolder values supply only id/isSpecial;
ingredients/crafting are not exercised. Empty advancement listeners and external
server/transport setup are disclosed. This is not a full Java server flow.
The differential covers two permission trees, seven focused completions and
twelve execution cases: state, result, typed feedback and recipe packet bytes.
The completion witness uses two descriptors and unambiguous prefixes; native
retains the full catalogue. Proprietary artifacts remain outside the checkout.

After `npm ci` and building, run `node tools/recipe_command_flow.mjs
target/debug/mc121-rust REPORT.json`. Three actual clients on temporary loopback25708
check typed tree/provider/permission, current catalogue suggestions, give/take,
normalization, duplicate/special/missing/malformed failures, exact selected
private/admin/ADD/REMOVE bytes, rich current target identity and exclusion of
other players. Per-recipient ordered team markers bound absence assertions.
Unsupported wildcard/selector/UUID inputs preserve known/highlight state.
A saved real `minecraft:recipes/misc/beacon` has_the_recipe criterion proves the
actual advancement callback; cold-restart INIT retains known/highlight state
and duplicate-grant failure. [Report](../validation/recipe-command-runtime.json).
Console output is checked semantically and its feedback component bytes are
covered by the differential. Existing whisper differential/server checks pass.

Wildcard order migration, selector/UUID/offline targets, new predicates/triggers,
datapack loading and full #24/#26 remain outside scope. Scope_version1,
denominator100 and counts58/53 unchanged. Hosted checks stay under #19.
