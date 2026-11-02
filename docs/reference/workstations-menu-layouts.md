# Additional native menu layouts

Target: Minecraft Java1.21, protocol767. Primary evidence: pinned original
server/mappings identified in [the integration ledger](workstations-integration.md),
direct JDK21 javap inspection; REA was not used. Constructors `cpy` (BeaconMenu),
`cqz` (LecternMenu) and `cqv` (HorseInventoryMenu) establish these layouts.

| Menu | MenuType | Top | Player menu slots | Data |
| --- | --- | --- | --- | --- |
| Beacon | 9 | payment0 | 1..36 | levels/primary/secondary, three live values |
| Lectern | 17 | actual book0 | none | actual page, one value |
| Horse inventory | none | saddle0, separate BODY access1, storage2.. | starts2+3*columns,36 slots | none |

`MenuSync::new_horse_inventory(window, columns)` captures a synchronization
layout only, with supported builtin column counts0..5. It creates no mount or
equipment/container owner. `MenuSync::kind()` now returns
`Option<WorkstationKind>`: horse is `None`, other menus retain their actual kind.
The dedicated Horse Screen Open packet must be used for that unregistered
menu; assigning a convenient MenuType ID would be incorrect. Shared block
validity rejects that layout: the horse controller requires actual captured
entity, inventory/BODY-container validity and entity reach instead.

Allocation, snapshot, prediction bounds, listener player mapping and baseline
transfer use the captured player-slot range. Lectern therefore publishes one
slot, reads no unrelated player slot during synchronization, and transfers no
window0 inventory baseline. Crafter retains its original special order:
inputs0..8, player9..44, noninteractive preview45. No preview or player inventory
owner is duplicated by the layout metadata.

Focused native flows check the one-slot lectern content packet and absence of
player listeners, all six builtin horse column layouts and nullable type,
beacon player offsets/data, and the retained crafter preview mapping. These
are native layout checks, not proof of real block/entity Main integration.
