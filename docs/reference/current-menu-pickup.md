# Current Generic and Crafting menu pickup

One focused actual Main protocol flow passed for a barrel and a crafting table.
It verifies that collecting a genuine ItemEntity updates the retained current
menu after Collect, preserves canonical item counts and does not turn a crafting
preview into an owned stack. It does not complete a checklist row or establish
full container or Minecraft compatibility.

## Exact executable and record

| Identity | Value |
| --- | --- |
| Minecraft / protocol | Java 1.21 / 767 |
| Source commit | `c136c78b7f55098f7e93eb1cb57caab1046ca152` |
| Source tree | `c7dd687526fdf2f6c81ead45e91fcce16ba12720` |
| Actual executable SHA-256 | `af50dfb4c7f378dc4101770efcbf352f915a815a6ccf29e2d1912c97d616d06e` |
| Focused harness SHA-256 | `18bab156f5c6475d4579cb664cda2e65c652080b929a300709c9e2e6cf5b07cf` |
| Reused protocol/menu helper SHA-256 | `f9b5656cfef8907b9f054f0abe96d942b1711839363d44319155949e370e31d7` |
| Reused transport helper SHA-256 | `14f81108916913a825a7001d1a07ac9afcb07c3b92e28b20bc9de90f9ba17d94` |
| Started / completed UTC | `2026-11-02T00:46:22.136807+00:00` / `2026-11-02T00:46:52.481095+00:00` |
| Sanitized report SHA-256 | `134bbf7b76427abe073e4c4749282de2c460b272c8c65715b86b0d24e47b1f73` |

The [actual report](../validation/current-menu-pickup-flow.json) retains the
observed packet ordinals and fixture digests. The full Main executable build
passed before this run. The isolated server completed its genuine SIGINT
shutdown with exit 0; the harness checks that exit before accepting saved counts.
No proprietary binaries, private world files, identities or local paths appear
in the published record.

## Production source path

`main_player_body.rs`, `PlayerBodyPhase::Pickup`, captures the sole current menu
owner before touching an item. Generic and Crafting owners install the actual
`with_external_menu_sync_and_pickup_preflight` callback on `RuntimeInventoryHost`.
This is the live native body path exercised here, not a startup sample or a
separate demonstration menu.

`RuntimeInventoryHost::add_to_inventory` first runs pure current-owner admission
against the candidate, actual inventory and retained baselines, then calls the
canonical inventory add without an inventory packet. The actual item runtime
publishes Collect once. Its `collect_item` phase calls `sync_menu` through the
retained current owner. Typed unsupported FOOD identity can refuse before that
mutation; other errors propagate. This flow uses plain supported stacks and
does not exercise refusal recovery.

`main_block_entities.rs`, `NativeContainerRuntime::broadcast_for_inventory`,
borrows the real block entity's top slots. `main_crafting_table.rs`,
`CraftingTableRuntime::broadcast_for_inventory`, borrows the same canonical
player inventory and retains the actual crafting grid, result and cursor.
Neither creates a second player inventory or substitutes inactive window 0.

The kernels' `broadcast_changes_with_listener` operations perform fallible value
work first. Each changed slot then updates its local observation, invokes its
listener and only afterwards updates that slot's remote baseline and emits its
packet. Actual player-backed slots map to canonical inventory coordinates;
Generic top slots and Crafting grid/result slots fail that backing-identity
check. Main forwards genuine player-slot observations to the existing gameplay
owner under the native empty-criteria/nonreentrant profile. This callback and
baseline order is a source finding; the protocol run does not instrument
internal callbacks or `Inventory.timesChanged`.

## Actual runtime observations

All blocks and stacks came from real protocol operations in a fresh native
world. The synthetic offline creative client placed barrel item 1205 and
crafting table item 300, opened the actual blocks and used legal plain stacks.
It created each stone ItemEntity through a genuine negative CreativeSlot drop,
then moved to the server-observed item position for normal pickup. It did not
inject a saved world, edit inventory files or call an internal pickup function.

| Actual operation | Observation |
| --- | --- |
| Barrel open | Menu type 2, actual window 1, 63 slots; its 27 real storage slots remained empty |
| First pickup | Item entity 2, count 7: Collect `0x6f` at ordinal 120, then actual current-window SetSlot `0x15` at ordinal 121; canonical stone 0 to 7 |
| Crafting setup | Menu type 12, actual window 2, 46 slots; one owned oak log 132 in grid slot 1 produced the actual four oak planks 36 preview in result slot 0 |
| Second pickup | Item entity 3, count 7: Collect at ordinal 151, then current-window SetSlot at ordinal 152; canonical stone 7 to 14 |
| Preview identity | The same grid log 1 and derived preview planks 4 remained; owned player planks stayed 0 |
| Packet ownership | Neither pickup interval contained an inactive window 0 or direct-inventory window -2 update; actual current menu and empty cursor remained retained |
| Connected close | Player owned stone 14, oak log 1, oak planks 0; cursor empty |
| Actual native save | Player retained those same counts; watched world-item and block-container counts were all 0; crafted statistics stayed empty |

The saved inventory and actual block-entity slots independently confirm the
wire count observations. Closing the crafting table returned its owned log and
did not return, drop, persist or award a second stack from its preview. Each of
the two seven-stone source drops has exactly one final player owner.

## Limits

The run covers one ordinary barrel and one crafting table under the supported
creative native flat-world, default-feature, unmounted and unenchanted-equipment
profile. It does not exercise survival, partial or refused pickup, inventory
overflow, custom components, equipped enchantments, arbitrary advancement or
reentrant callbacks, multiple observers, other menu families, full click/drag
behavior, ordinary autosave timing, Java saves, public or graphical clients, or
performance. It includes a real disconnect and native shutdown save, not a cold
restart. No transient inventory-counter or callback instrumentation is claimed.
The source proof and this narrow actual runtime witness are separate evidence.
