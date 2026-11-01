# Native item use — original Java 1.21

Artifact: `server-inner-1.21.jar`, SHA256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
`tools/item_use_facts.java` bootstraps that artifact and extracts the actual
use/finish/tick/release declaring classes, eating/drinking sound registry IDs
and game-event notification radii into `fixtures/item-use-facts.json`.

The dispatch catalogue has 1333 item entries. 37 default items inherit both
`Item.use` and `Item.finishUsingItem`; foods with nonempty effects remain
unsupported. Milk and honey use their actual subclass methods. Other subclass
methods are not replaced by the default Item implementation.

Source call sites:

- `LivingEntity` (`btn`) `updatingUsingItem`/`J`, lines3137–3146: compare item
  identity and rebind the actual current hand stack, including changed count
  or components.
- `updateUsingItem`/`a(cuq)`, lines3149–3156, and `shouldTriggerItemUseEffects`/`L`,
  lines3159–3162: effects occur before decrement; elapsed use must exceed
  `(int)(duration * 0.21875F)` and remaining ticks must be divisible by4.
- `startUsingItem`/`c(bqq)`, lines3185–3198, and `stopUsingItem`/`fx`,
  lines3312–3321: synchronized flags and start/finish game events; stop clears
  the using bit, retaining the last offhand flag.
- `triggerItemUseEffects`/`b(cuq,int)`, lines3236–3248, and its particle helper:
  three entity floats and one shared `Math.random` double per eating particle;
  the actual server `Level.addParticle` returns without a network broadcast.
  Eating sound consumes entity RNG; drinking pitch consumes Level RNG.
- `ServerPlayer` (`aqv`) `L_`, lines1396–1400: targeted entity event9 precedes
  the inherited finish path and its sixteen use effects.
  `GameProtocols` (`agh`) registers the bundle delimiter first, followed by
  thirty ordinary packet registrations before `ClientboundEntityEventPacket`
  (`acy`), giving its actual PLAY packet ID31 (`0x1f`).
- `Player` (`cmx`) `eat`, lines2202–2221: FoodData mutation, item-used stat,
  burp, consume criterion, inherited eating sound/effects/consumption/EAT event,
  and the infinite-materials remainder gate.
- `MilkBucketItem` (`cvd`): duration32; consume criterion and item-used stat;
  remove-all-effects; `ItemUtils.createFilledResult(..., false)`. The creative
  milk stack remains while a bucket is added to the actual inventory.
- `HoneyBottleItem` (`cuf`): duration40 and drinking animation/sound; inherited
  food finish, its own second consume criterion/stat and remove-poison; no
  glass bottle for an infinite-materials player.

`JdkMathRandom` is one shared process owner, seeded using the bundled JDK's
`java.util.Random` uniquifier multiplier1181783497276652981 and actual host
monotonic time. It is separate from Minecraft's RandomSupport owner. The
parent admission path supplies the three LivingEntity constructor draws.

Main integration is limited to the actual native creative, instant-build,
invulnerable, unmounted and empty-effects body source. The body supplies its
actual positive current-FULL region and physical position; packet use and
release acquire those same world/player owners. Game events traverse the real
certified empty registries. Registered advancements/effects and imported
unproved owners remain unsupported. Inventory changes await the ordinary menu
broadcast phase. This is not complete item use or full vanilla parity.
