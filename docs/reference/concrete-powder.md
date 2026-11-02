# ConcretePowderBlock hardening source kernel

`src/falling_block/concrete_powder.rs` supplies callable block-bound
ConcretePowderBlock algorithms. `src/falling_block/block.rs` additionally
certifies the finite pinned powder source class and its captured concrete
associations against the existing canonical registry. These library contracts
do not install a production Main owner, certify active world callbacks or prove
a complete concrete-powder falling lifecycle or checklist delivery. Actual Main
admission and runtime validation are reported by the integration coordinator.

## Pinned source evidence

Evidence is direct JDK21 `javap -p -c` of the original Java1.21 server and official
mappings, not REA MCP or an observed Java gameplay execution. REA tools were not
exposed in this session. The outer server SHA-1 is
`450698d1863ab5180c25d7c804ef0fe6369dd1ba`; inner server SHA-256 is
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`;
mappings SHA-1 is `31c77994d96f05ba25a870ada70f47f315330437`.
Raw JVM disassembly and Java binaries remain outside the repository.

| Original method | Preserved native contract |
| --- | --- |
| ConcretePowderBlock constructor30–32, `dhk.<init>`, offsets5–7 | Retain its actual final `concrete` Block association. The native binding carries both the invoked powder singleton and that associated concrete singleton; no color-name or palette-offset inference. |
| `canSolidify`76, `dhk.n`, offsets0–10 | Read the supplied BlockState's actual FluidState, then query FluidTags.WATER. This is a tag test, not identity with source water, a generic liquid test, a fluid-height gate, rain or an ultrawarm-dimension override. |
| `shouldSolidify`54, `dhk.a(BlockGetter,BlockPos,BlockState)`, offsets0–20 | Short-circuit on the supplied captured state's water tag before querying neighbors. Do not replace that argument with a fresh world read. |
| `touchesLiquid`58–72, `dhk.a(BlockGetter,BlockPos)`, offsets0–114 | Preserve Direction.values order, mutable cursor reads, DOWN gate, neighbor water-tag test and opposite-face support test described below. No RNG is used. |
| `onLand`36–39, `dhk.a(Level,BlockPos,BlockState,BlockState,FallingBlockEntity)`, offsets0–24 | Test the captured replaced state, then actual neighbors when required. If wet, fetch the retained concrete's default state and call actual Level.setBlock with flags3; ignore its boolean as source does. The carried state and entity argument are not read by this body. |
| `getStateForPlacement`43–50, `dhk.a(BlockPlaceContext)`, offsets0–42 | Obtain the context's actual Level, then clicked position, then current target state. Return associated concrete.defaultBlockState if shouldSolidify; otherwise invoke actual inherited placement with the unchanged context. No write is performed here. |
| `updateShape`81–85, `dhk.a(BlockState,Direction,BlockState,LevelAccessor,BlockPos,BlockPos)`, offsets0–31 | Call touchesLiquid directly. Wet returns associated concrete.defaultBlockState without a falling schedule or superclass call. Dry delegates all original arguments to FallingBlock.updateShape. |
| Inherited FallingBlock.onPlace32–33, `diu.b`, offsets0–10 | Unconditionally schedule this powder Block with actual delay2. There is no ConcretePowderBlock.onPlace override or added hardening query. |
| Inherited FallingBlock.updateShape37–39, `diu.a`, offsets0–27 | Schedule this same Block at delay2 before the inherited unchanged-state delegate. Reuse the actual shared BLOCK scheduler and subtick owner; deduplication does not suppress the source scheduling attempt. |
| Inherited FallingBlock.tick44–50 / falling53 | Reuse the existing source scheduled tick and empty inherited falling hook. It tests below/isFree before minimum build height, then calls the same actual constructor/replacement/add owner. It does not configure Anvil hurt2/40. |

## Finite source class and inherited scheduler contract

`block::source_class` recognizes `NativeFallingClass::ConcretePowder` only for
the captured `dhk` class. Its existing catalogue owner now verifies exactly
16 powder singletons, each with inherited `diu` onPlace, actual delay2, its own
`dhk` placement method and its recorded `hardened_state`. Each captured target
must resolve to a distinct canonical single-state base Block/default with the
source inherited onPlace declaration. Both powder and concrete key/range/default
bindings are checked against the **same existing** inventory registry owner.
The placement file hash must match that owner's actual pinned bootstrap proof.
No Block registry ID is inferred from state IDs or iteration position.

The association comes from `PlacementCatalog` reading the original final
`ConcretePowderBlock.b` field and that actual Block's default state. The checks
use these captured associations, never a color-name rule or a difference between
powder and concrete state IDs. The observed source domain is:

| Powder singleton | Captured concrete singleton |
| --- | --- |
| white_concrete_powder (`12744`) | white_concrete (`12728`) |
| orange_concrete_powder (`12745`) | orange_concrete (`12729`) |
| magenta_concrete_powder (`12746`) | magenta_concrete (`12730`) |
| light_blue_concrete_powder (`12747`) | light_blue_concrete (`12731`) |
| yellow_concrete_powder (`12748`) | yellow_concrete (`12732`) |
| lime_concrete_powder (`12749`) | lime_concrete (`12733`) |
| pink_concrete_powder (`12750`) | pink_concrete (`12734`) |
| gray_concrete_powder (`12751`) | gray_concrete (`12735`) |
| light_gray_concrete_powder (`12752`) | light_gray_concrete (`12736`) |
| cyan_concrete_powder (`12753`) | cyan_concrete (`12737`) |
| purple_concrete_powder (`12754`) | purple_concrete (`12738`) |
| blue_concrete_powder (`12755`) | blue_concrete (`12739`) |
| brown_concrete_powder (`12756`) | brown_concrete (`12740`) |
| green_concrete_powder (`12757`) | green_concrete (`12741`) |
| red_concrete_powder (`12758`) | red_concrete (`12742`) |
| black_concrete_powder (`12759`) | black_concrete (`12743`) |

These numbers are state IDs of actual single-state Blocks, not raw Block
registry IDs. `concrete_powder_hardened_state(state)` returns the captured target
default for an admitted powder source class. `concrete_powder_delay` is narrow
to powder, `ordinary_delay` remains narrow to sand/red-sand/gravel, and
`source_delay` covers the three finite classes Anvil/Ordinary/ConcretePowder.
They report source facts; none grants a retained Main capability.

`inherits_base_falling_hook(state)` is true only for certified `dhg` and `dhk`.
The pinned `dhk` method inventory has no falling override, and inherited
`diu.a(FallingBlockEntity)` at source53 is literally bytecode0:return. The
existing OrdinaryFallingBlockHost contract and scheduled-tick kernel retain
their callable signatures for both admitted base-hook classes. Production must
resolve the same constructed/add-attempt object before executing that empty
body. No Anvil hurt2/40 setter, second entity, constructor entropy refund or
additional RNG draw is introduced. Stable support/minimum-height branches do
not acquire falling-entity constructor entropy.

For powder, the actual Main scheduler and physics/restore admission must also
require the installed retained NativeConcretePowderOwner, matching powder and
associated concrete singleton bindings, actual damage/configuration owners and
all required stage-specific world capabilities. Unknown families remain
unsupported. A source-class match without those owners is not a successful
Main callback. Powder's wet updateShape still returns concrete without reaching
FallingBlock scheduling; broadening source_delay cannot create an extra wet
shape obligation.

## Exact neighbor traversal

`touches_liquid` starts its cursor at the original powder position. It visits
DOWN, UP, NORTH, SOUTH, WEST, EAST, matching `Direction.values()` (`ji.w`, array
offsets0–41). At the start of **every** iteration it reads the world at the
cursor left by the previous iteration. Only DOWN checks this first state for
water. If the initial original-position state is dry, DOWN skips relocation and
never reads the below neighbor. Water below dry powder alone therefore does
not pass this helper.

For an admitted direction, the cursor is reset to originalPosition+direction
using Java-compatible wrapping integer additions. The source reads that
neighbor and first checks its fluid's WATER tag. Only a wet neighbor reaches
`neighborState.isFaceSturdy(world, originalPosition, direction.opposite())`.
The position argument is deliberately the original powder position, not the
neighbor's position. A non-sturdy wet neighbor returns true immediately.
Preserving the preceding cursor reads avoids silently changing the original
BlockGetter callback/query sequence.

The three-argument isFaceSturdy call resolves to
`BlockBehaviour.BlockStateBase.isFaceSturdy`, source1282, `dtb$a.d`, which passes
SupportType.FULL at offsets4–7. Its source1286–1289 overload uses the actual
state cache when present or evaluates that support type against the same
BlockGetter, supplied position and face. A collision-full-block shortcut or an
invented dry/empty shape result is not an implementation of this operation.

## Required production owners and call phases

`ConcretePowderHost` extends the existing FallingBlock world/scheduler contract
and requires real fluid-state/tag, face-support, default-state, placement-context
and flags3 edit capabilities. It supplies no successful fallback. Missing or
unsupported owners return an error. Native preflight only checks the real
association and capabilities; it does not query the world early, draw RNG,
schedule, edit or publish. A production edit must complete admission before
mutating any world/source/scheduler/output owner.

The production binding must retain the actual powder-to-concrete Block
association and validate it against the certified library association and real
canonical registry once. The wrapper's constructor retains supplied handles;
calling it alone is not that provenance check. State getters resolve against that same
registry. Fluid tags belong to the actual world/configuration and include
waterlogged or flowing-water states when their genuine fluid is WATER-tagged.
Face-support queries retain the same dimension, BlockGetter and state/shape
cache owners, including any source dynamic-shape dependencies.

Placement must lend the original real BlockPlaceContext with its actual Level
and clicked position. The dry superclass placement and shape delegates must be
established from the real inheritance; unavailable overrides cannot be replaced
with guessed states. Wet placement/updateShape only return a state; their outer
production owner performs the corresponding world write and callbacks.

For landing, dispatch from the captured original powder Block at the existing
FallingBlockEntity source phase: successful carried-state setBlock, actual block
update broadcast, actual entity discard, then Fallable.onLand with the captured
replaced state. Lend that same retained actor through `LandingEntity`; it is
distinct from the constructor/admission handle and is not a copied physical
proxy. The hardening setBlock must use the same actual edit service and execute
its real flags3 callbacks, lighting, schedules and publication. Do not add a
second entity, drop, sound, particle or RNG draw to this block-bound callback.

## Remaining delivery and validation boundary

The central coordinator and assigned Main owners retain production owner
installation, placement/update propagation, scheduler certification and actual
landing dispatch. Real fluid/face-support/edit capabilities must be available
before these functions can be admitted in Main. The existing source
FallingBlockEntity kernel has concrete-water and high-speed raycast branches,
but this stage does not implement or certify their production world/clip hosts.
It does not remove current unsupported fluid, active callback, portal or block
entity boundaries by advertising this helper as a complete falling lifecycle.

The source dust-color getter and inherited client animation are outside this
hardening stage. No complete ConcretePowderBlock class codec, generic falling
subclass support or whole-game compatibility is claimed. The class/association
and inherited-hook source work was reviewed against the pinned bytecode,
existing captured facts and agreed Main-owner contracts. Its compiler/runtime
validation is pending the coordinator's coherent integration stage. No tests,
builds, Git operations or progress metric changes were made in this source slice.
