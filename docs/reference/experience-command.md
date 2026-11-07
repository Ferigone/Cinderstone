# Player experience commands (#213)

Permission2 players and console execute `experience` and the `xp` redirect with
add/set/query, points/levels and default points. Add accepts signed amounts;
set is nonnegative. One current online name, native case-insensitive lookup and
self-target are supported. The native tree preserves multiple-player add/set
versus single-player query argument flags and live name completion. Selectors,
UUID/offline targets fail explicitly. Full parsing and lookup precede mutation;
invalid point-set, syntax, missing targets and denied non-ops leave actual XP,
metadata score and dirty cache unchanged.

Main stages the existing ExperienceData/metadata score, checks actual effect
owners, then commits to the retained target. Successful mutation writes the
existing lastSentExp field to-1 even for unchanged total/state; the normal
player tick emits the actual target-only XP bar packet. Query stays private and
leaves the cache clean. Typed feedback and current rich target names use the
shared source/operator policy. Existing save/rejoin stores level, total, exact
f32 progress and metadata score. No second XP, score, packet cache or RNG owner
is introduced.

The existing point arithmetic now preserves Player.giveExperienceLevels' int
overflow reset before subsequent division. The shared orb/command level-up path
keeps its real cooldown and distinct level sound RNG. Only positive level
transitions enter it; the positive overflow-to-zero case retains the original
zero-volume sound and RNG consumption. Native alive-creative readiness/default
equipment and sound-owner availability are checked before mutation. Imported
worlds, unavailable body/equipment/effect profiles and nonfinite resulting XP
remain explicitly unsupported; broader survival/movement/mending work stays open.

`tools/experience_command_oracle.java` compiles with the admin_feedback/say
helpers using the [external pinned classpath](admin-feedback.md). Run
`cinderstone.oracle.experience_command_oracle ORIGINAL_JAR
fixtures/experience-command-vectors.json`. It executes original ExperienceCommand,
ServerPlayer setters/dirty fields and Player point/level arithmetic. Metadata
score and Level.playSound are recording sinks. Original XP codec explicitly
models a next dirty tick; the full Java tick/transport is not executed. Two
permission trees, eight completions and one29-step sequence compare result,
float bits/score/dirty state, domain feedback and bar bytes. The sequence covers
one int boundary and restores ordinary state. Sound sinks record key/category/
volume/pitch at controlled tick ages; they do not claim world RNG execution.
Proprietary artifacts remain outside the checkout.

After `npm ci` and building, run `node tools/experience_command_flow.mjs
target/debug/mc121-rust REPORT.json`. Three temporary loopback25709 clients check
the decoded original tree projection, completion, target-only bar bytes, typed
private/operator feedback, real owner probes after failures, alias/case/self
routing and actual sound metadata. Identical nearby broadcast sound packets and
before/after actual sound RNG states prove use of the existing owner; the seed
is not compared to the original recording sink. Ordered per-client markers bound
audience assertions. Admission's direct XP packet and constructor-dirty first
tick both finish before command assertions. Cold restart retains XP/score;
query emits no additional bar update. [Report](../validation/experience-command-runtime.json).
The focused command differential (including shared point arithmetic) and server
checks pass. The actual-container server regression checks that inner equipment
validation errors propagate before command commit. General syntax-error context
decoration is not claimed.

Full #24/#57/#65, selector/UUID/offline targets, new orbs/mending/effects/vehicles
and imported/unavailable profile expansion remain outside scope. Scope_version1,
denominator100 and counts58/53 unchanged. Hosted checks stay under #19.
