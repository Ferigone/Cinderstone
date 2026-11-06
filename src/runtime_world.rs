//! Runtime bridge for atomic block edits, shape state updates and publication.
//! Full neighbourChanged callbacks, fluid ticks and block event effects are
//! separate simulation work; this applies the verified connection-state rules.
use crate::{
    placement,
    storage::VanillaWorld,
    world::{BlockChange, FlatWorld, MAX_Y, MIN_Y},
};
use anyhow::{ensure, Result};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub enum RuntimeWorld {
    Fixture(FlatWorld),
    Anvil(VanillaWorld),
}
#[derive(Clone)]
pub enum WorldChange {
    Block(BlockChange),
    Light {
        chunk: (i32, i32),
        payload: bytes::Bytes,
    },
    /// Use-item-on corrections belong only to the requesting player, in world-event order.
    BlockResync {
        player: i32,
        blocks: [BlockChange; 2],
    },
    Acknowledge {
        player: i32,
        sequence: i32,
    },
}
const SHAPE_ORDER: [(i32, i32, i32); 6] = [
    (-1, 0, 0),
    (1, 0, 0),
    (0, 0, -1),
    (0, 0, 1),
    (0, -1, 0),
    (0, 1, 0),
];

impl RuntimeWorld {
    pub fn block(&mut self, x: i32, y: i32, z: i32) -> Result<u32> {
        match self {
            Self::Fixture(world) => Ok(world.block(x, y, z)),
            Self::Anvil(world) => world.block(x, y, z),
        }
    }
    fn set_raw(&mut self, p: (i32, i32, i32), state: u32) -> Result<bool> {
        match self {
            Self::Fixture(world) => Ok(world.set_block(p.0, p.1, p.2, state)),
            Self::Anvil(world) => world.set_block(p.0, p.1, p.2, state),
        }
    }
    pub fn chunk_packet(&mut self, x: i32, z: i32) -> Result<Vec<u8>> {
        match self {
            Self::Fixture(world) => Ok(world.chunk_packet(x, z)),
            Self::Anvil(world) => world.chunk_packet(x, z),
        }
    }
    /// The caller holds the world lock through both this mutation and enqueueing
    /// all returned updates, so concurrent sessions cannot publish in reverse.
    pub fn edit_block(&mut self, x: i32, y: i32, z: i32, state: u32) -> Result<Vec<WorldChange>> {
        if !(MIN_Y..=MAX_Y).contains(&y) {
            return Ok(Vec::new());
        }
        if self.block(x, y, z)? == state {
            return Ok(Vec::new());
        }
        placement::placement_block_name(state)?;
        // A connection-state change can reach the adjacent chunk, whose light
        // rebuild needs its own two-chunk halo. Validate availability first.
        let mut light_revisions = BTreeMap::new();
        if let Self::Anvil(world) = self {
            let (cx, cz) = (x.div_euclid(16), z.div_euclid(16));
            for dx in -3..=3 {
                for dz in -3..=3 {
                    ensure!(
                        world.has_chunk(cx + dx, cz + dz)?,
                        "edit light context is not generated at {},{}",
                        cx + dx,
                        cz + dz
                    );
                }
            }
            for dx in -2..=2 {
                for dz in -2..=2 {
                    let p = (cx + dx, cz + dz);
                    light_revisions.insert(p, world.light_revision(p.0, p.1)?);
                }
            }
        }
        // Resolve pure connection updates against an overlay first. A missing
        // world fact must not leave an unannounced primary edit behind.
        let mut staged = BTreeMap::from([((x, y, z), state)]);
        let mut changes = vec![BlockChange { x, y, z, state }];
        let mut queue = VecDeque::new();
        enqueue_shape_updates((x, y, z), &mut queue);
        let mut updates = 0usize;
        while let Some(p) = queue.pop_front() {
            if !(MIN_Y..=MAX_Y).contains(&p.1) {
                continue;
            }
            let old = match staged.get(&p) {
                Some(state) => *state,
                None => self.block(p.0, p.1, p.2)?,
            };
            let next =
                placement::update_horizontal_connections(old, p, |q| match staged.get(&q) {
                    Some(state) => Ok(*state),
                    None => self.block(q.0, q.1, q.2),
                })?;
            if let Some(next) = next {
                if next != old {
                    staged.insert(p, next);
                    changes.push(BlockChange {
                        x: p.0,
                        y: p.1,
                        z: p.2,
                        state: next,
                    });
                    enqueue_shape_updates(p, &mut queue);
                }
            }
            updates += 1;
            ensure!(
                updates < 1_000_000,
                "connection-state update budget exceeded"
            );
        }
        for change in &changes {
            self.set_raw((change.x, change.y, change.z), change.state)?;
        }
        let mut output = changes
            .iter()
            .copied()
            .map(WorldChange::Block)
            .collect::<Vec<_>>();
        if let Self::Anvil(world) = self {
            let centers = changes
                .iter()
                .map(|c| (c.x.div_euclid(16), c.z.div_euclid(16)))
                .collect::<BTreeSet<_>>();
            let mut lit = BTreeSet::new();
            for (cx, cz) in centers {
                lit.extend(world.relight_around(cx, cz)?);
            }
            for chunk in lit {
                if light_revisions.get(&chunk).copied()
                    != Some(world.light_revision(chunk.0, chunk.1)?)
                {
                    output.push(WorldChange::Light {
                        chunk,
                        payload: world.light_packet(chunk.0, chunk.1)?.into(),
                    });
                }
            }
        }
        Ok(output)
    }
}
fn enqueue_shape_updates(p: (i32, i32, i32), queue: &mut VecDeque<(i32, i32, i32)>) {
    for d in SHAPE_ORDER {
        queue.push_back((p.0 + d.0, p.1 + d.1, p.2 + d.2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_query_failure_does_not_commit_primary_write() {
        let mut fixture = FlatWorld::new(0, 1, 10, 9, 39);
        // Inject an unavailable state into a neighbour to exercise a failed
        // planning query, rather than a successful edit's mirror assertions.
        assert!(fixture.set_block(1, 64, 0, 32767));
        let mut world = RuntimeWorld::Fixture(fixture);
        assert!(world.edit_block(0, 64, 0, 1).is_err());
        assert_eq!(world.block(0, 64, 0).unwrap(), 0);
        assert_eq!(world.block(1, 64, 0).unwrap(), 32767);
    }

    #[test]
    fn unknown_target_state_is_rejected_without_a_write() {
        let mut world = RuntimeWorld::Fixture(FlatWorld::new(0, 1, 10, 9, 39));
        assert!(world.edit_block(0, 64, 0, 32767).is_err());
        assert_eq!(world.block(0, 64, 0).unwrap(), 0);
    }
}
