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
`master`. No source archive remains in the reachable Git history.
Repository-scoped authentication for normal Git commits and pushes is configured.

Railway follows the unpinned `master` branch for automatic deployment. Dockerfile
validation fixes are published. The confirmed rejection of the Docker `VOLUME`
instruction was corrected by removing that instruction; the actual Railway volume
mounted at `/data` remains configured. An earlier cache-format diagnosis was only
a suspect and has not been established as the initial failure cause.

The latest automatically triggered Railway build was last observed `BUILDING`.
A successful deployment, public Minecraft TCP response, real-account login and
persistent redeploy behavior have not yet been verified. A configured TCP proxy
alone does not establish a playable server.

## Current integration check

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
