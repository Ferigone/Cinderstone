# Current native port status

Cinderstone targets Minecraft Java 1.21, protocol 767 and DataVersion 3953. It is
an experimental native Rust implementation. Full vanilla compatibility is not
complete.

The current integrated batch includes native player body phases for the supported
creative profile; water/lava travel; food, milk and honey use; Chest/Barrel
inventories and persistence; CraftingMenu3x3; selected redstone components; a shared
BLOCK scheduler; and selected current-version Java `player.dat`/item NBT codecs.

## Publication and deployment

Normal source files are published in the public
[Ferigone/Cinderstone repository](https://github.com/Ferigone/Cinderstone) on
`master`, with an English README and roadmap. Source archives are not committed.

Railway deploys the unpinned `master` branch automatically. The preview uses a
persistent `/data` volume and one replica. The Trixie image supplies the OpenSSL
version required when online authentication is enabled. The last verified release
reached Railway `SUCCESS` and `Online`; its native startup event confirmed
Minecraft 1.21, protocol 767, online mode and listening on `0.0.0.0:25565`.

Preview address: **`hopper.proxy.rlwy.net:24418`**. Use a stock Minecraft Java
**1.21** client. An update to accept offline accounts is being deployed with
`MC_ONLINE_MODE=false`; its deployment and offline join still need verification.
Once active, the preview will not require a Mojang session or a valid Java account.
The image default remains `MC_ONLINE_MODE=true`. Offline mode does not authenticate
nicknames, and its name-derived UUIDs do not automatically reuse existing online
player data. The
[deployment observation](validation/railway-preview.json) records the verified
scope of the earlier online-mode release. An external status probe from the development environment could not
resolve the proxy hostname (`EAI_AGAIN`), so a public status/pong response and a
complete real-account login remain unverified. Hosting does not establish full
vanilla gameplay compatibility. Player-data persistence across a public redeploy
also remains to be verified with an actual online session.

## Current integration check

The offline-login change passed a focused local client flow with public bind
`0.0.0.0`: status/pong, unauthenticated PLAY login, the expected name-derived UUID,
no online-authentication challenge and graceful shutdown. Invalid mode
combinations were rejected. See [offline flow report](validation/offline-preview-flow.json).
Railway deployment of this change is pending.

The bucket fluid/redstone callback-ownership correction passed Rust type checking
and a direct optimized release build. The limited native client flow completed
successfully on 1 November 2026 at 02:12:45 UTC. The
[machine-readable flow report](validation/native-preview-flow.json) records the
exact binary and scope. It covered:

- Native join and movement with Minecraft Java 1.21 protocol 767.
- Water-bucket placement and pickup.
- Authoritative transfer of 17 stone through a chest menu.
- Furnace crafting in the 3×3 crafting table and shift transfer of its result.
- Completed native milk use, including entity status 9.
- SIGTERM shutdown and native block-entity persistence.
- Cold restart preserving the player, crafted item and chest contents.

This closes the previously observed bucket-to-chest transaction failure in that
bounded flow. It does not establish full gameplay compatibility or validate a
real-account login to the public deployment. Earlier cached thin-LTO link errors
were followed by the successful direct build; no Rust source compilation failure
remains recorded for this patch. Comprehensive conformance remains deferred.
Historical evidence applies only to the source and binaries recorded in those
reports.

Unsupported areas include full survival/damage/death/respawn, active effects,
vehicles, elytra, AI/mobs, full generation, all workstations/block entities,
complete redstone, TNT/explosions, commands/datapacks and full Java-world
interoperability. Unsupported owners and callbacks remain explicit errors.

The fixed 100-item planning checklist remains **55/100 code deliveries** and
**50/100 main-server integrations** at `scope_version: 1`. These are checklist
indicators, not measures of functional compatibility, engineering effort or
remaining time. Callback fixes, publication and hosting setup do not add checklist
points. Porting and integration remain the priority; broad validation belongs to
a separate final stage.

See the [README](../README.md) and [roadmap](ROADMAP.md) for the English project
scope and remaining work.
