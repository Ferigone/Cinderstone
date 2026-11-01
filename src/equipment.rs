//! Vanilla1.21 equipment comparison, slot mapping and observer packets.
//!
//! LivingEntity checks all seven slots in enum order. A genuine hand swap uses
//! EntityEvent status55 and omits both hand entries from SetEquipment. Attribute
//! and enchantment side effects are represented by EquipmentChange for the
//! simulation owner; this module implements the comparison and wire policy.
use crate::{
    inventory::{ItemStack, PlayerInventory},
    protocol::{Packet, Writer},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EquipmentSlot {
    MainHand = 0,
    OffHand = 1,
    Feet = 2,
    Legs = 3,
    Chest = 4,
    Head = 5,
    Body = 6,
}
impl EquipmentSlot {
    pub const ALL: [Self; 7] = [
        Self::MainHand,
        Self::OffHand,
        Self::Feet,
        Self::Legs,
        Self::Chest,
        Self::Head,
        Self::Body,
    ];
    pub fn from_id(id: u8) -> Result<Self> {
        ensure!(id < 7, "unknown equipment slot");
        Ok(Self::ALL[usize::from(id)])
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerEquipment {
    slots: [ItemStack; 7],
}
impl Default for PlayerEquipment {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| ItemStack::empty()),
        }
    }
}
impl PlayerEquipment {
    pub fn from_inventory(inventory: &PlayerInventory) -> Result<Self> {
        let mut equipment = Self::default();
        equipment.slots[0] = inventory.selected_stack(0)?.clone();
        equipment.slots[1] = inventory.selected_stack(1)?.clone();
        for (equipment_slot, inventory_slot) in [(2, 8), (3, 7), (4, 6), (5, 5)] {
            equipment.slots[equipment_slot] = inventory.slots()[inventory_slot].clone();
        }
        // Player.getItemBySlot returns EMPTY for animal BODY armor.
        Ok(equipment)
    }
    pub fn validate(&self) -> Result<()> {
        for stack in &self.slots {
            ItemStack::decode_exact(&stack.encode())?;
        }
        Ok(())
    }
    pub fn get(&self, slot: EquipmentSlot) -> &ItemStack {
        &self.slots[slot as usize]
    }
    pub fn set(&mut self, slot: EquipmentSlot, stack: ItemStack) {
        self.slots[slot as usize] = stack;
    }
    pub fn initial_packet(&self, entity_id: i32) -> Result<Option<Packet>> {
        let entries: Vec<_> = EquipmentSlot::ALL
            .into_iter()
            .filter(|slot| !self.get(*slot).is_empty())
            .map(|slot| (slot, self.get(slot).clone()))
            .collect();
        if entries.is_empty() {
            Ok(None)
        } else {
            Ok(Some(equipment_packet(entity_id, &entries)?))
        }
    }
}

/// Exact ItemStack.matches semantics: empty stacks compare equal, otherwise
/// count and effective item/components participate, rather than patch bytes.
pub fn equipment_matches(left: &ItemStack, right: &ItemStack) -> Result<bool> {
    if left.is_empty() || right.is_empty() {
        return Ok(left.is_empty() && right.is_empty());
    }
    Ok(left.count == right.count && left.same_item_same_components(right)?)
}

pub fn equipment_packet(entity_id: i32, entries: &[(EquipmentSlot, ItemStack)]) -> Result<Packet> {
    ensure!(entity_id > 0, "entity ID must be positive");
    ensure!(
        !entries.is_empty(),
        "SetEquipment requires at least one entry"
    );
    let mut seen = [false; 7];
    let mut out = Writer::new();
    out.varint(entity_id);
    for (index, (slot, stack)) in entries.iter().enumerate() {
        let id = *slot as usize;
        ensure!(!seen[id], "duplicate equipment slot");
        seen[id] = true;
        out.u8(*slot as u8 | if index + 1 < entries.len() { 0x80 } else { 0 })
            .raw(&stack.encode());
    }
    Ok(Packet {
        id: 0x5b,
        payload: out.into_inner(),
    })
}

#[derive(Debug, Clone)]
pub struct EquipmentChange {
    pub slot: EquipmentSlot,
    pub previous: ItemStack,
    pub current: ItemStack,
}
#[derive(Debug, Clone, Default)]
pub struct EquipmentUpdate {
    pub packets: Vec<Packet>,
    pub changes: Vec<EquipmentChange>,
}

pub fn detect_equipment_changes(
    entity_id: i32,
    previous: &PlayerEquipment,
    current: &PlayerEquipment,
) -> Result<EquipmentUpdate> {
    ensure!(entity_id > 0, "entity ID must be positive");
    let mut update = EquipmentUpdate::default();
    let mut changed = [false; 7];
    for slot in EquipmentSlot::ALL {
        if !equipment_matches(previous.get(slot), current.get(slot))? {
            changed[slot as usize] = true;
            update.changes.push(EquipmentChange {
                slot,
                previous: previous.get(slot).clone(),
                current: current.get(slot).clone(),
            });
        }
    }
    if changed[0]
        && changed[1]
        && equipment_matches(
            current.get(EquipmentSlot::MainHand),
            previous.get(EquipmentSlot::OffHand),
        )?
        && equipment_matches(
            current.get(EquipmentSlot::OffHand),
            previous.get(EquipmentSlot::MainHand),
        )?
    {
        let mut out = Writer::new();
        out.i32(entity_id).u8(55);
        update.packets.push(Packet {
            id: 0x1f,
            payload: out.into_inner(),
        });
        changed[0] = false;
        changed[1] = false;
    }
    let entries: Vec<_> = EquipmentSlot::ALL
        .into_iter()
        .filter(|slot| changed[*slot as usize])
        .map(|slot| (slot, current.get(slot).clone()))
        .collect();
    if !entries.is_empty() {
        update.packets.push(equipment_packet(entity_id, &entries)?);
    }
    Ok(update)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    #[test]
    fn inventory_slot_mapping_and_selected_hotbar_match_player_get_item_by_slot() {
        let mut inventory = PlayerInventory::new();
        for (slot, item, count) in [
            (5, 1, 5),
            (6, 2, 6),
            (7, 3, 7),
            (8, 4, 8),
            (36, 5, 36),
            (44, 6, 44),
            (45, 7, 45),
        ] {
            inventory
                .set_creative_slot(slot, ItemStack::new(item, count).unwrap())
                .unwrap();
        }
        let equipment = PlayerEquipment::from_inventory(&inventory).unwrap();
        assert_eq!(equipment.get(EquipmentSlot::MainHand).count, 36);
        assert_eq!(equipment.get(EquipmentSlot::OffHand).count, 45);
        assert_eq!(equipment.get(EquipmentSlot::Feet).count, 8);
        assert_eq!(equipment.get(EquipmentSlot::Legs).count, 7);
        assert_eq!(equipment.get(EquipmentSlot::Chest).count, 6);
        assert_eq!(equipment.get(EquipmentSlot::Head).count, 5);
        assert!(equipment.get(EquipmentSlot::Body).is_empty());
        inventory.set_selected_hotbar(8).unwrap();
        assert_eq!(
            PlayerEquipment::from_inventory(&inventory)
                .unwrap()
                .get(EquipmentSlot::MainHand)
                .count,
            44
        );
    }
    #[test]
    fn initial_empty_equipment_is_omitted_and_packet_requires_unique_slots() {
        let equipment = PlayerEquipment::default();
        assert!(equipment.initial_packet(17).unwrap().is_none());
        assert!(equipment_packet(17, &[]).is_err());
        assert!(equipment_packet(
            17,
            &[
                (EquipmentSlot::MainHand, ItemStack::empty()),
                (EquipmentSlot::MainHand, ItemStack::empty())
            ]
        )
        .is_err());
        assert!(equipment_packet(0, &[(EquipmentSlot::MainHand, ItemStack::empty())]).is_err());
    }
    #[test]
    fn comparison_hand_swap_and_packet_bytes_match_actual_living_entity() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tools/equipment-java.json")).unwrap();
        assert_eq!(
            fixture["jar_sha256"],
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01"
        );
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 612);
        let mut previous = PlayerEquipment::default();
        for (index, case) in cases.iter().enumerate() {
            let update = if case["op"] == "new" {
                previous = PlayerEquipment::default();
                EquipmentUpdate::default()
            } else {
                let mut current = PlayerEquipment::default();
                for (slot, encoded) in EquipmentSlot::ALL
                    .into_iter()
                    .zip(case["slots"].as_array().unwrap())
                {
                    current.set(
                        slot,
                        ItemStack::decode_exact(
                            &STANDARD.decode(encoded.as_str().unwrap()).unwrap(),
                        )
                        .unwrap(),
                    );
                }
                let update = detect_equipment_changes(17, &previous, &current).unwrap();
                // Actual handleHandSwap/handleEquipmentChanges copy only slots
                // whose logical item/count/components changed.
                for change in &update.changes {
                    previous.set(change.slot, change.current.clone());
                }
                update
            };
            let wanted = &case["expected"];
            assert_eq!(
                update
                    .changes
                    .iter()
                    .map(|c| c.slot as u64)
                    .collect::<Vec<_>>(),
                wanted["changes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s.as_u64().unwrap())
                    .collect::<Vec<_>>(),
                "equipment case{index}: {case}"
            );
            let packets = wanted["packets"].as_array().unwrap();
            assert_eq!(
                update.packets.len(),
                packets.len(),
                "equipment case{index}: {case}"
            );
            for (packet, wanted) in update.packets.iter().zip(packets) {
                assert_eq!(
                    packet.id,
                    wanted["id"].as_i64().unwrap() as i32,
                    "equipment case{index}"
                );
                assert_eq!(
                    packet.payload,
                    STANDARD
                        .decode(wanted["payload"].as_str().unwrap())
                        .unwrap(),
                    "equipment case{index}: {case}"
                );
            }
        }
    }
}
