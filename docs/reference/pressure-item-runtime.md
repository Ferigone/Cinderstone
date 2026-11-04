# Native Item pressure-plate runtime observation

One focused protocol-767 flow passed on source commit
`49643deaea532a38ae01327213c4b821cb643215`, tree
`1d76c8da562b9b01954e2f54b444188c7465b452`. The executable SHA256 was
`3cd05a5f8de8e29c8bc3e0ca3ec719638e761a0638867c8d1c2c5895ef3b15ed`.
The actual packet observations and outcome are recorded in
[pressure-item-flow.json](../validation/pressure-item-flow.json).

The synthetic offline client placed a real oak pressure plate at `(10,64,8)`
through ordinary creative placement. It requested its actual creative flight
capability, moved above the plate, and used the real negative creative-slot
drop packet to create one stone Item. No world, source history, seed, actor or
checkpoint input was edited. The real type-58 Item spawned at
`(10.5,67.31999999284744,8.5)` and activated the plate: the client observed state
`5716`, followed by BLOCKS sound `1568` at the plate center.

Player feet remained at height `66` during activation, above the plate's
`64..64.25` touch interval. The client then moved beside the plate to pick up
the same Item. Its body minimum X was `11.05`, outside the plate touch maximum
X `10.9375`. The actual Collect packet for that same Item, with count `1`,
preceded the scheduled release publication of state `5717` and sound `1567`.
There was one Item spawn and one Collect. The normal unedited shutdown save
contained one stone in the player's canonical inventory, an empty cursor, and
no remaining Item body, conserving the single dropped stack.

The server also logged one tick overload (`8143 ms` behind, `162` skipped
deadline ticks). This flow establishes callback/publication ordering; exact
elapsed game-tick delay and wall-clock performance were not instrumented.

The only server launch explicitly used `--allow-offline --bind 127.0.0.1`
with `--loopback-service-keys` and the valid empty local public-key JSON.
The server reported `source: loopback-file` and `network_refresh: false`; no
official key-source event was observed. This supplied no signed certificate
or successful certificate-validation claim.

This observation covers one native ordinary Item, one wooden pressure plate,
its actual pickup and scheduled release. It does not establish full Minecraft
parity, XP/Falling pressure behavior, weighted/stone plates, active controller
networks, arbitrary world imports, or exact cross-process random trajectories.
