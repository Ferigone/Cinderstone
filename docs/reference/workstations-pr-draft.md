# Add native workstation transactions and production owner contracts

Proposed draft PR against `master` from
`teams/workstations/ws-20261101-7c2f/menus`. Refs
[issue #3](https://github.com/Ferigone/Cinderstone/issues/3).
This is a prepared description, not an opened pull request.

Adds callable Minecraft Java 1.21/protocol 767 workstation behavior:
stonecutter and smithing inventory adapters with all seven ordinary click
modes, authoritative result ownership, state-ID/cursor synchronization,
stale-action guards and ordered close/drop effects. Adds bounded anvil,
grindstone, loom, cartography, lectern, beacon and brewing behavior, plus
special-recipe context helpers.

Primary evidence is the pinned original server and official mappings,
inspected directly with JDK 21 `javap`. Source facts and exact production
contracts are in [the integration ledger](workstations-integration.md) and
the per-family `workstations-*.md` ledgers.

There are no new Main callsites. Production opening/dispatch, actual
world/player/registry/map/advancement owners and remaining menu families are
pending. Keep the PR draft: `inventory.4`, `crafting.2` and `crafting.5` remain
incomplete. No Main/runtime/world/player/container/furnace core, manifest,
checklist or deployment changes are included. The `src/lib.rs` changes are
only the guide's standing mechanical export exception for `workstations`,
`container_workstation_stonecutter` and `container_workstation_smithing`.

Validation at source commit
`b770aa721ab56fc56f450622bc6706553ab062dd`:

- `cargo test --locked --lib workstation` passed focused native flows,
  including inventory conservation, callback/drop ordering and mixed close
  overflow.
- `cargo build --locked --bin mc121-rust` passed.
- `cargo check --all-targets` found unchanged base
  `tools/main_persistence_smoke.rs` initializers missing existing
  Options/Server fields; that shared test was not changed by this team.
- No new protocol-client or gameplay validation is claimed.

Exact source commits, file claims and state-owner/phase requests are retained
in the [team handoff](../coordination/status/ws-20261101-7c2f.md).
