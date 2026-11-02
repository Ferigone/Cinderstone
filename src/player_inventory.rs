//! A player's authoritative menu and its ordered, bounded packet outbox.
//!
//! World ticks and the connection handler share this handle. Mutating the menu
//! and publishing its packet batch happen under the same lock, so two writers
//! cannot publish their changes in reverse order. No guard escapes a closure.
//! After a lagged receiver, send [`SharedContainer::snapshot`] and discard queued
//! updates whose revision is at or below the returned snapshot revision.

use crate::{container::PlayerContainer, protocol::Packet};
use anyhow::{anyhow, Context, Result};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::broadcast;

pub const CONTAINER_OUTBOX_CAPACITY: usize = 256;

#[derive(Clone, Debug)]
pub struct ContainerUpdate {
    pub revision: u64,
    pub packets: Vec<Packet>,
}

#[derive(Clone, Debug)]
pub struct ContainerSnapshot {
    pub revision: u64,
    pub packet: Packet,
}

struct ContainerState {
    menu: PlayerContainer,
    revision: u64,
}

#[derive(Clone)]
pub struct SharedContainer {
    state: Arc<Mutex<ContainerState>>,
    updates: broadcast::Sender<ContainerUpdate>,
    publisher: Option<Arc<dyn Fn(ContainerUpdate) + Send + Sync>>,
}

impl SharedContainer {
    pub fn new() -> (Self, broadcast::Receiver<ContainerUpdate>) {
        Self::from_container(PlayerContainer::new())
    }

    pub fn from_container(menu: PlayerContainer) -> (Self, broadcast::Receiver<ContainerUpdate>) {
        Self::with_capacity(menu, CONTAINER_OUTBOX_CAPACITY)
    }

    /// Put menu packets in the same ordered stream as entity effects. The
    /// callback runs under the menu lock and must not re-enter this container.
    pub fn with_publisher(publisher: Arc<dyn Fn(ContainerUpdate) + Send + Sync>) -> Self {
        let (mut container, _) = Self::new();
        container.publisher = Some(publisher);
        container
    }

    fn with_capacity(
        menu: PlayerContainer,
        capacity: usize,
    ) -> (Self, broadcast::Receiver<ContainerUpdate>) {
        let (updates, receiver) = broadcast::channel(capacity);
        (
            Self {
                state: Arc::new(Mutex::new(ContainerState { menu, revision: 0 })),
                updates,
                publisher: None,
            },
            receiver,
        )
    }

    fn lock(&self) -> Result<MutexGuard<'_, ContainerState>> {
        self.state
            .lock()
            .map_err(|_| anyhow!("player container lock was poisoned"))
    }

    pub fn read<R>(&self, read: impl FnOnce(&PlayerContainer) -> R) -> Result<R> {
        Ok(read(&self.lock()?.menu))
    }

    /// Commit one menu operation and publish its complete packet batch.
    /// On error the menu, its remote prediction baseline and revision revert.
    /// The closure must not mutate external state before returning an error.
    /// Successful operations advance the revision even when they emit no packet.
    pub fn transact<R>(
        &self,
        transaction: impl FnOnce(&mut PlayerContainer) -> Result<(R, Vec<Packet>)>,
    ) -> Result<R> {
        self.transact_with_commit(transaction, |result| result)
    }
    /// Commit a validated world action after menu state changes and before its
    /// packets enter the shared outbox. The commit must be infallible, using a
    /// prepared token which exclusively borrows its authoritative world store.
    /// Neither callback may re-enter this container while its lock is held.
    pub fn transact_with_commit<R, S>(
        &self,
        transaction: impl FnOnce(&mut PlayerContainer) -> Result<(R, Vec<Packet>)>,
        commit: impl FnOnce(R) -> S,
    ) -> Result<S> {
        self.transact_with_menu_commit(transaction, |result, _menu| commit(result))
    }
    /// Commit with access to the same actual menu while its existing lock is
    /// held, after preparation/revision advance and before outbox publication.
    /// Source-ordered callbacks may update that canonical menu's transient
    /// inventory state; no callback may re-enter this container or retain its
    /// borrowed menu. Like transact_with_commit, commit must be infallible and
    /// use only previously validated owners and prepared actions.
    pub fn transact_with_menu_commit<R, S>(
        &self,
        transaction: impl FnOnce(&mut PlayerContainer) -> Result<(R, Vec<Packet>)>,
        commit: impl FnOnce(R, &mut PlayerContainer) -> S,
    ) -> Result<S> {
        let mut state = self.lock()?;
        let revision = state
            .revision
            .checked_add(1)
            .context("player container revision exhausted")?;
        let before = state.menu.clone();
        let (result, packets) = match transaction(&mut state.menu) {
            Ok(value) => value,
            Err(error) => {
                state.menu = before;
                return Err(error);
            }
        };
        state.revision = revision;
        let result = commit(result, &mut state.menu);
        // The state remains valid if its connection has gone away. A later
        // subscriber obtains a full snapshot rather than replaying old batches.
        let update = ContainerUpdate { revision, packets };
        if let Some(publish) = &self.publisher {
            publish(update);
        } else {
            let _ = self.updates.send(update);
        }
        Ok(result)
    }

    /// Return a full authoritative inventory and an outbox revision barrier.
    /// This changes the menu's wire state ID just like PlayerContainer.full_sync.
    /// The returned packet belongs to the caller; it is not also broadcast.
    pub fn snapshot(&self) -> Result<ContainerSnapshot> {
        let mut state = self.lock()?;
        let revision = state
            .revision
            .checked_add(1)
            .context("player container revision exhausted")?;
        let packet = state.menu.full_sync();
        state.revision = revision;
        Ok(ContainerSnapshot { revision, packet })
    }

    /// Attach another receiver before taking its initial snapshot. Subscribing
    /// first ensures any concurrent later commit is queued or in that snapshot.
    pub fn subscribe(&self) -> broadcast::Receiver<ContainerUpdate> {
        self.updates.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::ItemStack;
    use std::{sync::Barrier, thread};

    #[test]
    fn concurrent_commits_publish_in_authoritative_mutation_order() {
        let (container, mut receiver) = SharedContainer::with_capacity(PlayerContainer::new(), 512);
        let gate = Arc::new(Barrier::new(5));
        let threads = (0..4)
            .map(|_| {
                let container = container.clone();
                let gate = gate.clone();
                thread::spawn(move || {
                    gate.wait();
                    for _ in 0..100 {
                        container
                            .transact(|menu| {
                                let count = menu.inventory.slot(36).unwrap().count % 99 + 1;
                                menu.inventory
                                    .set_slot_authoritative(36, ItemStack::new(1, count)?);
                                Ok(((), vec![menu.full_sync()]))
                            })
                            .unwrap();
                    }
                })
            })
            .collect::<Vec<_>>();
        gate.wait();
        for thread in threads {
            thread.join().unwrap();
        }
        for revision in 1..=400 {
            let event = receiver.try_recv().unwrap();
            assert_eq!(event.revision, revision);
            let mut reader = crate::protocol::Reader::new(&event.packets[0].payload);
            assert_eq!(reader.u8().unwrap(), 0);
            assert_eq!(reader.varint().unwrap(), revision as i32);
            assert_eq!(reader.varint().unwrap(), 46);
            for slot in 0..46 {
                let (stack, used) = ItemStack::decode(reader.remaining()).unwrap();
                reader.bytes(used).unwrap();
                if slot == 36 {
                    assert_eq!(stack.count, (revision as i32 - 1) % 99 + 1);
                }
            }
        }
        assert!(receiver.try_recv().is_err());
        assert_eq!(
            container
                .read(|m| m.inventory.slot(36).unwrap().count)
                .unwrap(),
            4
        );
    }

    #[test]
    fn failed_transaction_rolls_back_menu_baseline_and_does_not_publish() {
        let (container, mut receiver) = SharedContainer::new();
        let before = container.snapshot().unwrap();
        let result: Result<()> = container.transact(|menu| {
            menu.inventory
                .set_slot_authoritative(36, ItemStack::new(1, 10)?);
            menu.full_sync();
            anyhow::bail!("operation rejected")
        });
        assert!(result.is_err());
        assert!(container
            .read(|m| m.inventory.slot(36).unwrap().is_empty())
            .unwrap());
        assert!(receiver.try_recv().is_err());
        let after = container.snapshot().unwrap();
        assert_eq!(after.revision, before.revision + 1);
        // The failed full_sync did not consume a protocol state ID.
        assert_eq!(container.read(|m| m.inventory.state_id()).unwrap(), 2);
    }

    #[test]
    fn lagged_receiver_uses_snapshot_barrier_before_later_deltas() {
        let (container, mut receiver) = SharedContainer::with_capacity(PlayerContainer::new(), 2);
        for count in 1..=5 {
            container
                .transact(|menu| {
                    menu.inventory
                        .set_slot_authoritative(36, ItemStack::new(1, count)?);
                    Ok(((), vec![menu.full_sync()]))
                })
                .unwrap();
        }
        assert!(matches!(
            receiver.try_recv(),
            Err(broadcast::error::TryRecvError::Lagged(3))
        ));
        let snapshot = container.snapshot().unwrap();
        assert_eq!(snapshot.revision, 6);
        container
            .transact(|menu| {
                menu.inventory
                    .set_slot_authoritative(36, ItemStack::new(1, 6)?);
                Ok(((), vec![menu.full_sync()]))
            })
            .unwrap();
        // Another lag report is possible while retaining the old receiver. Its
        // remaining old batches are skipped using the snapshot's barrier.
        let mut delivered = Vec::new();
        loop {
            match receiver.try_recv() {
                Ok(update) if update.revision > snapshot.revision => {
                    delivered.push(update.revision)
                }
                Ok(_) | Err(broadcast::error::TryRecvError::Lagged(_)) => {}
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(error) => panic!("{error:?}"),
            }
        }
        assert_eq!(delivered, [7]);
    }

    #[test]
    fn partial_false_pickup_keeps_pending_menu_changes_until_the_actual_broadcast() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/player_inventory_pickup.json")).unwrap();
        let case = data["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case["name"] == "shulker_box/False")
            .unwrap();
        let mut stack = ItemStack::decode_exact(
            &STANDARD
                .decode(
                    data["stacks"][case["pickup"].as_u64().unwrap() as usize]
                        .as_str()
                        .unwrap(),
                )
                .unwrap(),
        )
        .unwrap();
        let mut menu = PlayerContainer::new();
        menu.record_external_sync();
        assert!(!menu.add_pickup_to_inventory(&mut stack, false).unwrap());
        assert!(!stack.is_empty());
        assert!(!menu.broadcast_changes().unwrap().is_empty());
        assert!(menu.broadcast_changes().unwrap().is_empty());
    }

    #[test]
    fn actual_java_inventory_add_pickups_all_items_and_partial_or_creative_overflow() {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/player_inventory_pickup.json")).unwrap();
        let stacks = data["stacks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| {
                ItemStack::decode_exact(&STANDARD.decode(value.as_str().unwrap()).unwrap()).unwrap()
            })
            .collect::<Vec<_>>();
        for case in data["cases"].as_array().unwrap() {
            let mut menu = PlayerContainer::new();
            for (index, value) in case["slots"].as_array().unwrap().iter().enumerate() {
                menu.inventory.set_slot_authoritative(
                    index,
                    stacks[value.as_u64().unwrap() as usize].clone(),
                );
            }
            menu.inventory
                .set_selected_hotbar(case["selected"].as_i64().unwrap() as i16)
                .unwrap();
            menu.record_external_sync();
            let mut pickup = stacks[case["pickup"].as_u64().unwrap() as usize].clone();
            let (accepted, packets) = menu
                .accept_pickup(&mut pickup, case["creative"].as_bool().unwrap())
                .unwrap();
            assert_eq!(
                accepted,
                case["accepted"].as_bool().unwrap(),
                "{}:accepted",
                case["name"]
            );
            let expected = &stacks[case["remaining"].as_u64().unwrap() as usize];
            assert!(
                pickup.same_item_same_components(expected).unwrap(),
                "{}:remaining kind",
                case["name"]
            );
            assert_eq!(
                pickup.count, expected.count,
                "{}:remaining count",
                case["name"]
            );
            if pickup.is_empty() && accepted {
                let original = &stacks[case["pickup"].as_u64().unwrap() as usize];
                let mut restored = pickup.clone();
                restored.count = original.count;
                assert_eq!(
                    restored.encode(),
                    original.encode(),
                    "{}: discarded source retains its item and patch",
                    case["name"]
                );
            }
            for (index, value) in case["state"].as_array().unwrap().iter().enumerate() {
                let actual = menu.inventory.slot(index).unwrap();
                let expected = &stacks[value.as_u64().unwrap() as usize];
                assert!(
                    actual.same_item_same_components(expected).unwrap(),
                    "{}:slot{index} kind",
                    case["name"]
                );
                assert_eq!(
                    actual.count, expected.count,
                    "{}:slot{index} count",
                    case["name"]
                );
            }
            assert!(packets.iter().all(|packet| packet.id == 0x15));
        }
        assert_eq!(data["cases"].as_array().unwrap().len(), 3084);
    }
}
