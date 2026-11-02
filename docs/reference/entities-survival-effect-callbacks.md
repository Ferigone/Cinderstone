# Concrete effect callbacks implementation handoff

Instance es-20261101-1425-a7c3; pre-edit claim root reports issue4comment6566386887.
Base ab410119874a261608f3e06b952af6fcb94db256, own worktree cinderstone-effect-callbacks.
ONLY repository file written by this agent: src/effects/callbacks.rs. Root owns exports,
reference/status docs, Git/publication and full Cargo; effects_lifecycle owns refresh.rs.

Source pins/type/order are in scratch/nov2/effect-callbacks-source-contract.md.
Evidence direct original1.21/protocol767 javap + official mappings, not REA/new probe.
Server SHA1 450698d1863ab5180c25d7c804ef0fe6369dd1ba; mappings SHA1 31c77994d96f05ba25a870ada70f47f315330437; inner SHA256 8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01.

## APIs and implemented source bodies

EffectUpdate<K>{entity_id:i32,effect:K,amplifier:i32,duration:i32,flags:u8}; effect_update(id,&EffectInstance<K>,blend): actual packet constructor28–48 field order and flags1/2/4/8. This is immutable source constructor data, not a packet codec or a connection/registry owner.

LivingEffectCallbacksHost<K> required associated actual Error/Actor/Entity/Lease/Definition/AttributeMap/Level/PassengerList/PassengerCursor/Player/Connection, no default implementations. Short with_instance closure reads exact retained object; owner borrows end before all callbacks. virtual_send_effect_to_passengers preserves public helper dispatch. Definition modifiers, dirty refresh, definition Added/Started and packet sends are complete required actual-owner operations.

living_on_effect_added(actor,effect,cause,host)->Result: Living1049–1054 dirtytrue; current level/server -> definition -> actual attrs -> fresh amp -> addModifiers -> virtual passenger helper; NO refreshDirty.
living_on_effect_updated(actor,effect,reapply,cause,host)->Result:1065–1075 dirtytrue; reapply&&server -> retained definition remove(attrs read1), add(attrs read2,current amp after remove), actual refresh; fresh server query then virtual passenger helper.
living_on_effect_removed(actor,effect,host)->Result:1078–1088 dirtytrue; server removeModifiers/refresh, then inline directpassenger Remove packets. It does not remove map entries.
send_effect_to_passengers(actor,effect,host)->Result:1057–1062 captures actual immutable list+iterator once, tests ServerPlayer per member in order; connection before current actorID and current retained-instance constructor data; blendfalse per packet. No all-viewer audience or reused prebuilt packet.

ServerPlayerEffectCallbacksHost<K> extension adds actual actor connection/LEVITATION/tickCount/position/start fields/criterion.
server_player_on_effect_added/updated/removed compose exact selected Living parent (Player inherits it, no separate override), then own current connection/ID/effect snapshot; blendtrue Added, false Updated; Remove ID+holder. Added levitation time->position, Updated no reset, Removed clearposition only; EFFECTS_CHANGED criterionlast, original cause foradd/update andnull forremove. No inserted own clientside gate.

instance_on_effect_added/started(actor,effect,host)->Result:brz338/248 resolve holder.value then read current amp, actual definition dispatch. Added optional sound and template modifier bodies remain mandatory native providers, not default successes. Sound contract explicitly captured Level before final XYZ/soundSource, excludednull/volume1/pitch1, actual Level.threadSafeRandom seed in playSound423–424.

EffectVisibilityHost<K> required actual map/lease/definition/Metadata/Particle/List/Values/ValuesCursor plus actual stream collection provider. collect_effect_particles executes real stream/spliterator-toList traversal and invokes the supplied native visible-filter/definition-particle mapper; no manufactured Java HashMap implementation.
are_all_effects_ambient(values,host)->Result<bool>:930–936 ordinary actual live values iterator; only visible&&!ambient vetoes; no CME catch.
update_synchronized_effect_particles(actor,host)->Result:863–870 actual stream.filter(isVisible).map(getParticleOptions).toList; metadata receiver10 after listcomplete, set10; capture metadata receiver11 BEFORE fresh current values/allAmbient, then set11.
remove_effect_particles(actor,host)->Result:940–941 capture metadata receiver before immutableemptyList, set10ONLY (ambient11 preserved).
living_update_invisibility_status(actor,host)->Result:851–860 actual empty->virtual clearParticles then invisfalse; nonempty->actual hasINVIS, invissetter, private syncparticles.
server_player_update_invisibility_status(actor,host)->Result:1702–1708 spectator->virtualclearParticles then invistrue; else exact parent body.
update_effect_visibility(actor,host)->Result:3123–3124 sets actual effectsDirtytrue only; distinct from passenger helper.

base_definition_on_effect_started(actor,amp): complete MobEffect107 literalreturn only when that actual selected base body is bound; never an unavailable definition default.
AbsorptionEffectStartedHost actual absorption get/set. absorption_on_effect_started(actor,amp,host)->Result: Absorption22–24 invokes selected emptyparent, current absorption, Javawrapped 4*(1+amp) ->f32 ->Java Math.max NaN/signedzero semantics ->actual setter/clamp/internalPlayer owner. No local absorption state.

## Validation and limits

rustfmt applied. Actual-file scratch/nov2/callbacks-focused.rs includes repository EffectInstance, AttributeInstance and callbacks module. Standalone rustc1.85 -Dwarnings ordinaryrlib and focused test compilation/flows pass. Existing source/helper modules stay untouched; no fullCargo by this worker.

Controlled hosts use actual native EffectInstance and AttributeInstance; real native modifier/dirty-listener/cache/permanent order executes. Listener recomputes actual attribute and mutates retained effect; Updated rereads changedamp, refresh reenters sameactor Added with no effect borrow held. Captured immutable passenger list survives graph change, while perpacket ID/effect fields reread; parentpassenger outputs precede self/blend/levitation/criterion. Client/periodic/definition-after-resolution and latepacket failure prefixes checked. Visibility fixture invokes native filter/mapper then metadata callback changes current values, proving fresh ambient traversal and receiver ordering; empty/spectator preserveambient11. Absorption checks Javafloat NaN/signedzero/wrappedint reward through actual required setter boundary.

The stream/iterator/registry/metadata/packet/connection/world definitions are controlled schedules/sinks, NOT production HashMap/spliterator/actor/Main implementations or original-Java runtime proof. Definition virtual providers, attribute-template iteration/actual registrations, base added-sound body, passenger graph, registry codecs, packet publication, effects map/hash/arena, save/load and Main remain real mandatory integration owners. Native refresh body is companion refresh.rs via callback (no copy/queue); privateglow/tickEffects are existing tick.rs (no duplicates). Arbitrary Java shared hidden-object alias graphs remain existing instance representation limit. Missing ownerError and earlier committed prefixes are preserved; no transactional retry or fake success. No whole effects/entity/survival completion claimed.
