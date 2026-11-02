# Native concrete-powder runtime observation

One protocol flow passed on commit `43a5dbf573a4c0732910c1f9f755655f12fcb1f1`,
tree `fbbc5641ef1452095c0fff36d5cb5d535fa3af57`, executable SHA256
`910eafad98e6677f8f089cb55ffed88428eae01091ffd11b1ee7546033250e22`.
Actual observations are recorded in
[concrete-powder-flow.json](../validation/concrete-powder-flow.json).

The synthetic offline client built a stone basin and filled it through a real
bucket POV use. It placed white concrete powder `12744` on a real dry support
at `(10,70,8)`, five blocks above source water `80` at `(10,65,8)`. The powder
remained stable through successive time publications. Removing its support
created one genuine type-40 FallingBlock entity, with the actual powder state
and original start position. No scene, seed, actor, native save or integrity
value was edited.

Normal shutdown captured that actor airborne at Y `69.8808`, time/tick count
`2`. A restart without PLAY left its complete physical snapshot, UUID, RNG,
carried state and counters unchanged while world time advanced. No assumption
was made that runtime handle generations survive across processes.

On the next real PLAY connection, chunk publication preceded pairing of the
same saved entity ID. Its saved position, quantized motion and start-position
metadata were restored. It resumed the fall into the actual basin and hardened
into white concrete `12728`. The actor-removal packet had ordinal `79`; the
concrete block publication followed at ordinal `80`, consistent with the
source discard-before-onLand phase. The concrete remained visible through
further time publications.

The final normal native save contained exactly one concrete block at
`(10,65,8)`, no remaining powder block or Falling actor, and no Item/XP
substitute. The next entity ID increased only for the genuine resumed Player.
Private original airborne input bytes were preserved without modification.

The initial and resumed PLAY launches logged tick overloads of `10231 ms`
and `7068 ms` respectively. This flow supplies ordering/ownership evidence;
wall-clock performance was not measured.

Every launch explicitly used offline mode, a loopback bind and the reviewed
local public-key file. Actual source events reported `loopback-file` and
`network_refresh: false`; no official key source was observed. The empty local
key set makes no signed-certificate validation claim.

This observation covers one dry player placement and its real water-landing
continuation, including native save/hidden resume. Direct-water or adjacent-
fluid player powder placement remains unsupported by the bounded FIFO profile.
This fall did not establish high-speed ray behavior, arbitrary dynamic support
shapes, mixed callback cascades, block NBT import or full Minecraft parity.
