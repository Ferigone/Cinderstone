# Native Player body source boundary

Target: official Minecraft Java 1.21 inner server SHA-256
`8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01`.
Mappings: original `server-1.21-mappings.txt`.

Implemented source paths:

- `aqv.m` (`ServerPlayer.doTick`) enters `cmx.l` (`Player.tick`), whose
  virtual `btn.aw`/`bsr.aw` base tick precedes `btn.J` active item use,
  `cmx.m_` peaceful food/inventory, `btn.m_` travel/freezing/push and
  `cmx.m_` item touch. Existing food code runs in the corresponding phases.
- `btn.a(exc)` water/lava/dry travel and `cmx.a(exc)` creative-flight
  velocity restoration use the existing authoritative PlayerMotion state,
  actual block friction, collision/move callbacks and fluid pushing.
- Constructor-only `Math.random` values preserve `rotA`, `timeOffs` and the
  constructor yaw values. The caller supplies three draws from the actual
  process-global Math.random owner; this module does not seed a substitute.
- `btn.swing` six-tick native swing and restart-at-half-duration gate.
- `bsr.h(bsr)` push arithmetic uses actual entity-section query order.
  The current native source has no teams, passengers or pushable non-Player
  entities. Player/item source tracking remains in RuntimeEntities.
- Player virtual fire timer setter caps creative invulnerable timers at one;
  Player fire immunity duration is twenty. Generic item defaults remain intact.
- Required positive current FULL-region proof is retained for item use and
  pickup; an unrelated pending chunk cannot substitute for this proof.
- Block entityInside executes before ENTER_BLOCK. Pressure-plate/button
  callbacks receive the actual transient body as an override, including packet
  movement while the motion owner is loaned out of its map.

`player_body_facts.java` bootstraps the original JAR only to capture all 26,684
block friction/render facts, the actual declaring class of each of 1,333
Item.inventoryTick methods and the relevant original sound IDs. Those are
facts, not claims that all Item ticks are supported.

The admission invariant remains native creative, unmounted, alive, with the
constructor-owned empty-effects lineage and supported default attributes.
Elytra, active effects, turtle helmet effects, vehicles, climbing/powder snow,
death/out-of-world damage and rain extinguishing without an actual exposure
provider are explicit errors. Full survival, XP/map update ownership, portals
and all Player lifecycle behavior are not claimed. No complete 1:1 or
performance claim is made by this port.

Final integration validation is deferred to the parent batch. No broad test
suite was added for this implementation.
