# Current native port status

Cinderstone targets Minecraft Java 1.21, protocol767 and DataVersion3953. It is an experimental native Rust implementation, not a complete replacement for vanilla.

The current integrated batch includes native player body phases for the supported creative profile; water/lava travel; food, milk and honey use; Chest/Barrel inventories and persistence; CraftingMenu3x3; selected redstone components; a shared BLOCK scheduler; and selected current-version Java player.dat/item NBT codecs.

Whole-server compilation passed for this reconstructed batch. Optimized build and a limited client flow are separate checks; comprehensive conformance remains deferred. Historical evidence applies only to the source and binaries recorded in those reports.

Unsupported areas include full survival/damage/death/respawn, active effects, vehicles, elytra, AI/mobs, full generation, all workstations/block entities, complete redstone, TNT/explosions, commands/datapacks and full Java-world interoperability. Unsupported owners/callbacks remain explicit errors rather than fabricated defaults.

See README.md and docs/ROADMAP.md for the English roadmap. The fixed100-item delivery checklist is a planning indicator, not a compatibility or effort percentage.
