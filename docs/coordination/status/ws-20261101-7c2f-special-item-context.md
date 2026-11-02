# Special item recipe context handoff

Instance: `ws-20261101-7c2f`; team: WORKSTATIONS; state: ready for integration.
Assigned [issue #3](https://github.com/Ferigone/Cinderstone/issues/3),
[pre-edit claim](https://github.com/Ferigone/Cinderstone/issues/3#issuecomment-6567338678).
Base: `68fba409b757eae8287b301228303ae2510a205d`.
Branch: `teams/workstations/ws-20261101-7c2f/special-item-context`.
Target: Minecraft Java 1.21, protocol 767.
Implementation commit: `2f08851eeca49f32e5b379c689b917a34b333445`.

## Exact owned changes

- New `src/workstations/special_item_context.rs`.
- One mechanical declaration in the owned `src/workstations/mod.rs`.
- New `docs/reference/workstations-special-item-context.md` and this status.

No existing crafting evaluator, Main, item codec, registry, world/player/entity
owner, manifest, public status/checklist, journal or deployment configuration is
changed. Separate documentation PR101 refreshes the team's historical index;
it does not implement this source stage.

## Callable API and source scope

`SpecialItemRecipeContext::new(&actual_owner)` captures its actual snapshot;
`evaluate(book, id, input)` supports only the suspicious-stew, decorated-pot and
armor-dye serializers resolved through `RecipeBook::recipe_type`.
`SpecialItemRecipeOwner` requires actual Item/DyeItem/StewEffects handles,
owner identity/generation validation, current tags/class relationships, typed
components/prototypes and identity-certified copies and Item remainders.

Matching and assembly preserve ordered predicates, all-input first stew-effect
holder lookup, pot decoration direction order, both armor-copy phases and Java
integer/float color arithmetic. Existing default and repair/trim evaluators
remain separate; this context has no implicit static fallback.

The [source ledger](../../reference/workstations-special-item-context.md)
records direct pinned JDK21 `javap` evidence and exact required services. No REA
execution, Java recipe execution, actual configured registry or Main/gameplay
witness is asserted. The current shared item wire/prototype codec domain is
still required even when the actual provider can resolve a dynamic handle.

## Required integration and call phase

Central must bind one retained actual Item/Block/tag/component owner and recipe
snapshot to `SpecialItemRecipeOwner`, then select this context in its actual
recipe-manager preview path. Missing owners must error. Snapshot keys identify
the real owner and generation, not equal tables or fabricated counters.

The current Main still uses its existing default evaluators. No production
caller of this new context exists in this stage. It consumes no inputs, creates
no second result owner and runs no player/world/effect/advancement callback.
The actual ResultSlot must retain its canonical aliases, validate current
inputs and snapshot, reserve the destination and run source callbacks. Its
current-manager remainder query belongs after crafted/recipe callbacks;
`CraftResult.remaining` from preview is not a take-time cache.

Component 32 needs actual effect holders/durations; it does not apply a living
effect. Component 51 needs actual Item identities and BRICK optional semantics.
Component 24 uses actual typed color and DyeColor texture values. Structural
copy checks supplement the mandatory real identity certificate; equal bytes do
not replace it. Unsupported copy/component/registry contexts remain errors.

## Verification and publication

Frozen source SHA-256:
`773584f063e0d2d743f6d4bf754e7c98fa3cf75c9e449985ec9b42cac02048bf`.
Isolated `cargo check --all-targets` passed in 1m24s. The focused native
stew/pot/dye flows passed on that exact source, covering ordered role overlap,
first nonflower supplier, logical AIR, directional decoration order, dye tag
precedence, both copy counts and missing/replaced owner rejection. They are
controlled owner checks, not an actual registry or Java/Main gameplay proof.
Independent pinned-primary/API review found no concrete blocking defect.
Formatting, whitespace and staged-index repository hygiene passed. Current
master advanced through independent loot PR89 to
`dfd4c99e92d815b24fc30ca8eb8a9ac1a134f3fb`; its clean local composition
`d428b18b5ddce8167bebd400a37c9f38e0a01a46` with this source has tree
`11fd56d312a3967cf1c8c75719598a831a19de9e`, source subtree
`c68d39077795657d702f82ae305baece986446b1`. An isolated all-target compiler
check passed on that composition in 26.06s. This does not assert it is the
eventual merge tree; the actual merge is read back separately. No master merge
or force push was performed on the source branch. Required GitHub CI and
publication read-back follow in the PR and issue.

Observed capacity is seven total slots. This continuation peaked at five
active agents, including the concurrent team-index author. All source, review
and ledger assignments are completed; root alone publishes and verifies.
No background work or cross-chat polling is guaranteed after a turn stops.
`inventory.4`, `crafting.2` and `crafting.5` remain aggregate partial deliveries,
with no checklist or denominator change.
