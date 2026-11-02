//! Original Java 1.21 Mob fields, separate from Entity and LivingEntity.
//!
//! Mob128–166 constructs these stores after the complete Living constructor.
//! `jvm_zeroed` belongs to the preceding object-allocation phase: references
//! remain null until their actual source assignment. There is no second health,
//! physics, entity identity, random stream, metadata byte or alive/dead flag.

use crate::{equipment::EquipmentSlot, living_entities::state::LivingObjects};

/// Canonical source objects used by a Mob. The embedding graph owns lifetimes:
/// internally stored self references must not create reference-counting cycles,
/// and every escaping object lease must retain the original actor graph. A
/// numeric-ID lookup or a weak reference alone is not such a lifetime lease.
pub trait MobObjects: LivingObjects {
    type GoalSelector;
    type LookControl;
    type MoveControl;
    type JumpControl;
    type BodyRotationControl;
    type Navigation;
    type Sensing;
    type LootTable;
    type LeashData;
}

/// A real ordinary control object's final Mob reference and native field state.
/// The surrounding object arena/lease, rather than this struct, supplies the
/// source object's stable identity. `mob` identifies the same canonical actor.
pub struct RetainedMobControl<A, S> {
    pub mob: A,
    pub state: S,
}

/// The closed original PathType enum (`epv`, source3–39), in ordinal order.
/// These are enum identities, not registry IDs or pathfinding results.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathType {
    Blocked = 0,
    Open = 1,
    Walkable = 2,
    WalkableDoor = 3,
    Trapdoor = 4,
    PowderSnow = 5,
    DangerPowderSnow = 6,
    Fence = 7,
    Lava = 8,
    Water = 9,
    WaterBorder = 10,
    Rail = 11,
    UnpassableRail = 12,
    DangerFire = 13,
    DamageFire = 14,
    DangerOther = 15,
    DamageOther = 16,
    DoorOpen = 17,
    DoorWoodClosed = 18,
    DoorIronClosed = 19,
    Breach = 20,
    Leaves = 21,
    StickyHoney = 22,
    Cocoa = 23,
    DamageCautious = 24,
    DangerTrapdoor = 25,
}
impl PathType {
    pub const ALL: [Self; 26] = [
        Self::Blocked,
        Self::Open,
        Self::Walkable,
        Self::WalkableDoor,
        Self::Trapdoor,
        Self::PowderSnow,
        Self::DangerPowderSnow,
        Self::Fence,
        Self::Lava,
        Self::Water,
        Self::WaterBorder,
        Self::Rail,
        Self::UnpassableRail,
        Self::DangerFire,
        Self::DamageFire,
        Self::DangerOther,
        Self::DamageOther,
        Self::DoorOpen,
        Self::DoorWoodClosed,
        Self::DoorIronClosed,
        Self::Breach,
        Self::Leaves,
        Self::StickyHoney,
        Self::Cocoa,
        Self::DamageCautious,
        Self::DangerTrapdoor,
    ];

    /// Actual final PathType.getMalus field value, before per-Mob overrides.
    pub fn default_malus(self) -> f32 {
        match self {
            Self::Blocked
            | Self::PowderSnow
            | Self::Fence
            | Self::Lava
            | Self::UnpassableRail
            | Self::DamageOther
            | Self::DoorWoodClosed
            | Self::DoorIronClosed
            | Self::Leaves => -1.0,
            Self::Water
            | Self::WaterBorder
            | Self::DangerFire
            | Self::DangerOther
            | Self::StickyHoney => 8.0,
            Self::DamageFire => 16.0,
            Self::Breach => 4.0,
            Self::Open
            | Self::Walkable
            | Self::WalkableDoor
            | Self::Trapdoor
            | Self::DangerPowderSnow
            | Self::Rail
            | Self::DoorOpen
            | Self::Cocoa
            | Self::DamageCautious
            | Self::DangerTrapdoor => 0.0,
        }
    }
}

/// The private Mob EnumMap's nonnull enum-key/Float-value observations.
/// Its constructor is empty, not a populated copy of all default maluses.
/// No hash, RNG, global registry or detached actor image is involved.
pub struct PathfindingMalus {
    values: [Option<f32>; 26],
}
impl PathfindingMalus {
    pub fn new() -> Self {
        Self { values: [None; 26] }
    }
    pub fn get(&self, kind: PathType) -> Option<f32> {
        self.values[kind as usize]
    }
    pub fn put(&mut self, kind: PathType, value: f32) -> Option<f32> {
        self.values[kind as usize].replace(value)
    }
    /// Only the selected actual Mob's map lookup/default portion of205.
    /// getPathfindingMalus198–201 must first select its actual vehicle/Mob.
    pub fn get_or_default(&self, kind: PathType) -> f32 {
        self.get(kind).unwrap_or_else(|| kind.default_malus())
    }
    pub fn len(&self) -> usize {
        self.values.iter().filter(|value| value.is_some()).count()
    }
    pub fn is_empty(&self) -> bool {
        self.values.iter().all(Option::is_none)
    }
}
impl Default for PathfindingMalus {
    fn default() -> Self {
        Self::new()
    }
}

/// Actual fixed NonNullList item store. Cloning an item handle aliases that
/// exact ItemStack; it must not copy stack contents. The default is retained
/// separately because NonNullList.clear resets each fixed entry to it.
pub struct MobItemList<S, const N: usize> {
    default_value: S,
    items: [S; N],
}
impl<S: Clone, const N: usize> MobItemList<S, N> {
    pub fn new(canonical_empty: S) -> Self {
        Self {
            items: std::array::from_fn(|_| canonical_empty.clone()),
            default_value: canonical_empty,
        }
    }
    pub fn items(&self) -> &[S; N] {
        &self.items
    }
    pub fn get(&self, index: usize) -> &S {
        &self.items[index]
    }
    pub fn set(&mut self, index: usize, item: S) -> S {
        std::mem::replace(&mut self.items[index], item)
    }
    pub fn clear(&mut self) {
        for item in &mut self.items {
            *item = self.default_value.clone();
        }
    }
}

/// Canonical Mob fields (`btp`), attached once to the actual retained object.
/// Current equipment is distinct from Living's last-equipment histories.
/// Metadata15 remains exclusively the inherited SynchedEntityData BYTE owner.
pub struct MobState<O: MobObjects> {
    pub ambient_sound_time: i32,
    pub xp_reward: i32,
    pub look_control: Option<O::LookControl>,
    pub move_control: Option<O::MoveControl>,
    pub jump_control: Option<O::JumpControl>,
    pub body_rotation_control: Option<O::BodyRotationControl>,
    pub navigation: Option<O::Navigation>,
    pub goal_selector: Option<O::GoalSelector>,
    pub target_selector: Option<O::GoalSelector>,
    pub target: Option<O::Actor>,
    pub sensing: Option<O::Sensing>,
    pub hand_items: Option<MobItemList<O::ItemStack, 2>>,
    pub hand_drop_chances: Option<[f32; 2]>,
    pub armor_items: Option<MobItemList<O::ItemStack, 4>>,
    pub armor_drop_chances: Option<[f32; 4]>,
    pub body_armor_item: Option<O::ItemStack>,
    pub body_armor_drop_chance: f32,
    pub can_pick_up_loot: bool,
    pub persistence_required: bool,
    pub pathfinding_malus: Option<PathfindingMalus>,
    pub loot_table: Option<O::LootTable>,
    pub loot_table_seed: i64,
    pub leash_data: Option<O::LeashData>,
    pub restrict_center: Option<O::BlockPos>,
    pub restrict_radius: f32,
}
impl<O: MobObjects> MobState<O> {
    pub fn jvm_zeroed() -> Self {
        Self {
            ambient_sound_time: 0,
            xp_reward: 0,
            look_control: None,
            move_control: None,
            jump_control: None,
            body_rotation_control: None,
            navigation: None,
            goal_selector: None,
            target_selector: None,
            target: None,
            sensing: None,
            hand_items: None,
            hand_drop_chances: None,
            armor_items: None,
            armor_drop_chances: None,
            body_armor_item: None,
            body_armor_drop_chance: 0.0,
            can_pick_up_loot: false,
            persistence_required: false,
            pathfinding_malus: None,
            loot_table: None,
            loot_table_seed: 0,
            leash_data: None,
            restrict_center: None,
            restrict_radius: 0.0,
        }
    }

    /// Callback-free getItemBySlot966–969 field fragment. None indicates an
    /// actual null constructor-prefix field, not an empty ItemStack fallback.
    pub fn item_by_slot(&self, slot: EquipmentSlot) -> Option<&O::ItemStack> {
        match slot {
            EquipmentSlot::MainHand => self.hand_items.as_ref().map(|v| v.get(0)),
            EquipmentSlot::OffHand => self.hand_items.as_ref().map(|v| v.get(1)),
            EquipmentSlot::Feet => self.armor_items.as_ref().map(|v| v.get(0)),
            EquipmentSlot::Legs => self.armor_items.as_ref().map(|v| v.get(1)),
            EquipmentSlot::Chest => self.armor_items.as_ref().map(|v| v.get(2)),
            EquipmentSlot::Head => self.armor_items.as_ref().map(|v| v.get(3)),
            EquipmentSlot::Body => self.body_armor_item.as_ref(),
        }
    }

    /// Primitive replacement ONLY. The actual setItemSlot executor must verify
    /// first, replace here, then synchronously invoke onEquipItem(old,new).
    /// Uninitialized-field failure returns the unconsumed new item.
    pub fn replace_item_by_slot(
        &mut self,
        slot: EquipmentSlot,
        item: O::ItemStack,
    ) -> Result<O::ItemStack, O::ItemStack> {
        match slot {
            EquipmentSlot::MainHand | EquipmentSlot::OffHand => {
                let Some(items) = self.hand_items.as_mut() else {
                    return Err(item);
                };
                Ok(items.set(usize::from(slot == EquipmentSlot::OffHand), item))
            }
            EquipmentSlot::Feet
            | EquipmentSlot::Legs
            | EquipmentSlot::Chest
            | EquipmentSlot::Head => {
                let Some(items) = self.armor_items.as_mut() else {
                    return Err(item);
                };
                Ok(items.set(slot as usize - EquipmentSlot::Feet as usize, item))
            }
            EquipmentSlot::Body => {
                let Some(current) = self.body_armor_item.as_mut() else {
                    return Err(item);
                };
                Ok(std::mem::replace(current, item))
            }
        }
    }

    /// Raw source chances: no normalization, clamping or guaranteed-drop flag.
    pub fn equipment_drop_chance(&self, slot: EquipmentSlot) -> Option<f32> {
        match slot {
            EquipmentSlot::MainHand | EquipmentSlot::OffHand => self
                .hand_drop_chances
                .as_ref()
                .map(|v| v[usize::from(slot == EquipmentSlot::OffHand)]),
            EquipmentSlot::Feet
            | EquipmentSlot::Legs
            | EquipmentSlot::Chest
            | EquipmentSlot::Head => self
                .armor_drop_chances
                .as_ref()
                .map(|v| v[slot as usize - EquipmentSlot::Feet as usize]),
            EquipmentSlot::Body => Some(self.body_armor_drop_chance),
        }
    }
}
