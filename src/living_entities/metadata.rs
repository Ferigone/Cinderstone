//! Actual synchronized Living/Mob/Bat data, composed with canonical Entity data.
//!
//! This owner stores only metadata8..16. Entity0..7 stay exclusively in the
//! actor's EntityPhysicalState; health is exclusively metadata9. It contains no
//! Player absorption/schema, actor, physical copy, RNG, scheduler or ID lookup.
//! The retained data lease belongs to the concrete actor assembly. Short field
//! projections finish before the inherited server-Bat virtual callback runs.

use crate::{entity_state::EntityPhysicalState, metadata::MetadataValue};
use std::fmt;

pub const DATA_LIVING_ENTITY_FLAGS: u8 = 8;
pub const DATA_HEALTH_ID: u8 = 9;
pub const DATA_EFFECT_PARTICLES: u8 = 10;
pub const DATA_EFFECT_AMBIENCE_ID: u8 = 11;
pub const DATA_ARROW_COUNT_ID: u8 = 12;
pub const DATA_STINGER_COUNT_ID: u8 = 13;
pub const DATA_SLEEPING_POS_ID: u8 = 14;
pub const DATA_MOB_FLAGS_ID: u8 = 15;
pub const DATA_BAT_FLAGS_ID: u8 = 16;

const SERIALIZERS: [i32; 17] = [0, 1, 6, 8, 8, 8, 21, 1, 0, 3, 18, 8, 1, 1, 11, 0, 0];
pub type MetadataValues = Vec<(u8, MetadataValue)>;

#[derive(Debug, PartialEq, Eq)]
pub enum MetadataFieldError {
    UnknownField(u8),
    WrongSerializer {
        id: u8,
        expected: i32,
        actual: i32,
    },
    InvalidPose(i32),
    /// Physical custom names currently have a wire-byte owner. Java Component
    /// equality/aliasing cannot be replaced with equality of those bytes.
    UnsupportedComponentOwner,
}

impl fmt::Display for MetadataFieldError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }
}
impl std::error::Error for MetadataFieldError {}

#[derive(Debug, PartialEq, Eq)]
pub enum MetadataError<E> {
    Host(E),
    Field(MetadataFieldError),
}
impl<E: fmt::Display> fmt::Display for MetadataError<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host(error) => error.fmt(out),
            Self::Field(error) => error.fmt(out),
        }
    }
}
impl<E: fmt::Debug + fmt::Display> std::error::Error for MetadataError<E> {}

/// The actual Living8..14/Mob15/Bat16 DataItems. Deliberately not Clone: cloning
/// an actor's synchronized fields would create an independent health owner.
#[derive(Debug)]
pub struct BatEntityData {
    values: [MetadataValue; 9],
    /// Relative indices0..8 correspond to actual accessor IDs8..16.
    dirty_items: u16,
    dirty: bool,
}

fn inherited_default(id: u8) -> MetadataValue {
    use MetadataValue as V;
    match id {
        0 | 8 | 15 | 16 => V::Byte(0),
        1 => V::Int(300),
        2 => V::OptionalComponent(None),
        3..=5 | 11 => V::Boolean(false),
        6 => V::Pose(0),
        7 | 12 | 13 => V::Int(0),
        9 => V::Float(1.0),
        10 => V::Particles(Vec::new()),
        14 => V::OptionalBlockPos(None),
        _ => unreachable!("defined Bat metadata field"),
    }
}

impl BatEntityData {
    /// Attach during Entity's virtual defineSynchedData, before Living's field
    /// initializers/constructor. These are registered defaults, not a completed
    /// Bat constructor: health is1 and Mob/Bat flags are0. Actual constructor
    /// setters subsequently change health/resting on this SAME receiver.
    pub fn defined_bat_defaults() -> Self {
        Self {
            values: std::array::from_fn(|index| inherited_default(index as u8 + 8)),
            dirty_items: 0,
            dirty: false,
        }
    }

    fn value(
        &self,
        physical: &EntityPhysicalState,
        id: u8,
    ) -> Result<MetadataValue, MetadataFieldError> {
        use MetadataValue as V;
        Ok(match id {
            0 => V::Byte(physical.shared_flags),
            1 => V::Int(physical.air_supply),
            2 => V::OptionalComponent(physical.custom_name.clone()),
            3 => V::Boolean(physical.custom_name_visible),
            4 => V::Boolean(physical.silent),
            5 => V::Boolean(physical.no_gravity),
            6 => V::Pose(i32::from(physical.pose)),
            7 => V::Int(physical.ticks_frozen),
            8..=16 => self.values[usize::from(id - 8)].clone(),
            _ => return Err(MetadataFieldError::UnknownField(id)),
        })
    }

    /// Source SynchedEntityData.set65–68 only. No payload validation or packet
    /// encoding: source serialization happens later, after mutation/packing.
    fn write_before_callback(
        &mut self,
        physical: &mut EntityPhysicalState,
        id: u8,
        value: MetadataValue,
        force: bool,
    ) -> Result<bool, MetadataFieldError> {
        let expected = *SERIALIZERS
            .get(usize::from(id))
            .ok_or(MetadataFieldError::UnknownField(id))?;
        let actual = value.serializer();
        if expected != actual {
            return Err(MetadataFieldError::WrongSerializer {
                id,
                expected,
                actual,
            });
        }
        if let MetadataValue::Pose(pose) = &value {
            if !(0..=17).contains(pose) {
                return Err(MetadataFieldError::InvalidPose(*pose));
            }
        }
        if matches!(&value, MetadataValue::OptionalComponent(Some(_))) {
            return Err(MetadataFieldError::UnsupportedComponentOwner);
        }
        if !force && self.value(physical, id)? == value {
            return Ok(false);
        }
        use MetadataValue as V;
        match (id, value) {
            (0, V::Byte(value)) => physical.shared_flags = value,
            (1, V::Int(value)) => physical.air_supply = value,
            (2, V::OptionalComponent(value)) => physical.custom_name = value,
            (3, V::Boolean(value)) => physical.custom_name_visible = value,
            (4, V::Boolean(value)) => physical.silent = value,
            (5, V::Boolean(value)) => physical.no_gravity = value,
            (6, V::Pose(value)) => physical.pose = value as u8,
            (7, V::Int(value)) => physical.ticks_frozen = value,
            (8..=16, value) => self.values[usize::from(id - 8)] = value,
            _ => unreachable!("validated source accessor serializer"),
        }
        Ok(true)
    }

    fn mark_dirty(&mut self, physical: &mut EntityPhysicalState, id: u8) {
        if id < 8 {
            physical.metadata_dirty |= 1 << id;
        } else {
            self.dirty_items |= 1 << (id - 8);
        }
        self.dirty = true;
    }

    fn is_dirty(&self, physical: &EntityPhysicalState) -> bool {
        // Existing canonical physical setters also mark their actual DataItems.
        // Preserve interoperability without copying those flags into this owner.
        self.dirty || physical.metadata_dirty & 0xff != 0
    }

    fn pack_dirty_values(&mut self, physical: &mut EntityPhysicalState) -> Option<MetadataValues> {
        if !self.is_dirty(physical) {
            return None;
        }
        self.dirty = false;
        let mut values = Vec::new();
        for id in 0..=16 {
            let dirty = if id < 8 {
                let bit = 1 << id;
                let dirty = physical.metadata_dirty & bit != 0;
                physical.metadata_dirty &= !bit;
                dirty
            } else {
                let bit = 1 << (id - 8);
                let dirty = self.dirty_items & bit != 0;
                self.dirty_items &= !bit;
                dirty
            };
            if dirty {
                // All Bat serializers are ForValueType.copy (identity). Values
                // here preserve Particle Arc identities; source lists used by
                // this bounded actor are immutable/replaced, never edited live.
                values.push((id, self.value(physical, id).expect("defined Bat field")));
            }
        }
        Some(values)
    }

    fn non_default_values(&self, physical: &EntityPhysicalState) -> Option<MetadataValues> {
        let mut values = Vec::new();
        for id in 0..=16 {
            let value = self.value(physical, id).expect("defined Bat field");
            if inherited_default(id) != value {
                values.push((id, value));
            }
        }
        (!values.is_empty()).then_some(values)
    }
}

/// Retained concrete-actor attachment. Data must root the SAME original actor,
/// including its attributes/world/dimensions owners after removal; an entity ID
/// or state-only lease cannot replace it. The projection itself is callback-free
/// and releases every guard before returning. Actor assembly supplies this real
/// binding; the metadata owner internally stores no strong actor/self-cycle.
pub trait BatMetadataHost {
    type Data: Clone;
    type Error;

    fn with_metadata<R>(
        &mut self,
        data: &Self::Data,
        field: impl FnOnce(&mut BatEntityData, &mut EntityPhysicalState) -> R,
    ) -> Result<R, Self::Error>;

    /// Actual virtual Entity.refreshDimensions3054–3067 on the retained actor.
    /// Called only for Pose6 by inherited Entity3034–3037. Server Bat inherits
    /// Living3202–3220; its additional8/14 callbacks are client-side only.
    fn refresh_dimensions(&mut self, data: &Self::Data) -> Result<(), Self::Error>;
}

pub fn get<H: BatMetadataHost>(
    data: &H::Data,
    id: u8,
    host: &mut H,
) -> Result<MetadataValue, MetadataError<H::Error>> {
    host.with_metadata(data, |values, physical| values.value(physical, id))
        .map_err(MetadataError::Host)?
        .map_err(MetadataError::Field)
}

/// SynchedEntityData.set65–73 with concrete inherited SERVER Bat callback.
/// Equality suppresses callbacks; force bypasses equality. A callback failure
/// retains its completed value mutation and does not newly mark that item dirty.
/// Reentry may pack/change fields before the outer call marks its captured item.
pub fn set<H: BatMetadataHost>(
    data: &H::Data,
    id: u8,
    value: MetadataValue,
    force: bool,
    host: &mut H,
) -> Result<bool, MetadataError<H::Error>> {
    let changed = host
        .with_metadata(data, |values, physical| {
            values.write_before_callback(physical, id, value, force)
        })
        .map_err(MetadataError::Host)?
        .map_err(MetadataError::Field)?;
    if !changed {
        return Ok(false);
    }
    if id == 6 {
        host.refresh_dimensions(data).map_err(MetadataError::Host)?;
    }
    host.with_metadata(data, |values, physical| values.mark_dirty(physical, id))
        .map_err(MetadataError::Host)?;
    Ok(true)
}

pub fn is_dirty<H: BatMetadataHost>(data: &H::Data, host: &mut H) -> Result<bool, H::Error> {
    host.with_metadata(data, |values, physical| values.is_dirty(physical))
}

/// SynchedEntityData.packDirty81–94. Clear the aggregate flag first, then each
/// ascending DataItem BEFORE its snapshot. Packet encoding is intentionally a
/// later operation: encoding failure must not restore already consumed dirty
/// flags. Unrelated physical flags (e.g. Item metadata8) are never consumed.
pub fn pack_dirty_values<H: BatMetadataHost>(
    data: &H::Data,
    host: &mut H,
) -> Result<Option<MetadataValues>, H::Error> {
    host.with_metadata(data, |values, physical| values.pack_dirty_values(physical))
}

/// SynchedEntityData.getNonDefaultValues99–111. Does not consume dirty flags.
pub fn non_default_values<H: BatMetadataHost>(
    data: &H::Data,
    host: &mut H,
) -> Result<Option<MetadataValues>, H::Error> {
    host.with_metadata(data, |values, physical| values.non_default_values(physical))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{entity::Position, geometry::Vec3};
    use std::sync::{Arc, Mutex};

    struct Frame {
        data: BatEntityData,
        physical: EntityPhysicalState,
    }
    #[derive(Default)]
    struct Host {
        dimensions: usize,
        fail_dimensions: bool,
        reenter: bool,
        callback_snapshot: Option<MetadataValues>,
    }
    impl BatMetadataHost for Host {
        type Data = Arc<Mutex<Frame>>;
        type Error = &'static str;

        fn with_metadata<R>(
            &mut self,
            data: &Self::Data,
            field: impl FnOnce(&mut BatEntityData, &mut EntityPhysicalState) -> R,
        ) -> Result<R, Self::Error> {
            let mut fields = data.lock().map_err(|_| "poisoned frame")?;
            let Frame { data, physical } = &mut *fields;
            Ok(field(data, physical))
        }
        fn refresh_dimensions(&mut self, data: &Self::Data) -> Result<(), Self::Error> {
            self.dimensions += 1;
            assert!(
                data.try_lock().is_ok(),
                "projection released before callback"
            );
            assert_eq!(get(data, 6, self).unwrap(), MetadataValue::Pose(1));
            assert!(!is_dirty(data, self)?);
            if self.fail_dimensions {
                return Err("dimensions failure");
            }
            if self.reenter {
                self.reenter = false;
                set(data, 9, MetadataValue::Float(4.0), false, self).unwrap();
                self.callback_snapshot = pack_dirty_values(data, self)?;
            }
            Ok(())
        }
    }
    fn actor() -> Arc<Mutex<Frame>> {
        Arc::new(Mutex::new(Frame {
            data: BatEntityData::defined_bat_defaults(),
            physical: EntityPhysicalState::new(
                7,
                [0; 16],
                Position {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    yaw: 0.0,
                    pitch: 0.0,
                    on_ground: false,
                },
                Vec3::ZERO,
                0.5,
                0.9,
                0.45,
            )
            .unwrap(),
        }))
    }

    #[test]
    fn actual_schema_constructor_changes_and_default_reversions() {
        let data = actor();
        let mut host = Host::default();
        assert!(non_default_values(&data, &mut host).unwrap().is_none());
        assert!(pack_dirty_values(&data, &mut host).unwrap().is_none());
        set(&data, 16, MetadataValue::Byte(1), false, &mut host).unwrap();
        set(&data, 9, MetadataValue::Float(6.0), false, &mut host).unwrap();
        assert_eq!(
            pack_dirty_values(&data, &mut host).unwrap().unwrap(),
            vec![(9, MetadataValue::Float(6.0)), (16, MetadataValue::Byte(1))]
        );
        assert_eq!(host.dimensions, 0);
        set(&data, 9, MetadataValue::Float(1.0), false, &mut host).unwrap();
        set(&data, 16, MetadataValue::Byte(0), false, &mut host).unwrap();
        assert!(non_default_values(&data, &mut host).unwrap().is_none());
        assert_eq!(
            pack_dirty_values(&data, &mut host).unwrap().unwrap(),
            vec![(9, MetadataValue::Float(1.0)), (16, MetadataValue::Byte(0))]
        );
        assert!(matches!(
            set(&data, 15, MetadataValue::Float(0.0), false, &mut host),
            Err(MetadataError::Field(
                MetadataFieldError::WrongSerializer { .. }
            ))
        ));
        assert!(matches!(
            set(
                &data,
                2,
                MetadataValue::OptionalComponent(Some(vec![8, 0])),
                false,
                &mut host
            ),
            Err(MetadataError::Field(
                MetadataFieldError::UnsupportedComponentOwner
            ))
        ));
        assert!(!is_dirty(&data, &mut host).unwrap());
    }

    #[test]
    fn synchronous_pose_reentry_and_failure_keep_source_mutation_prefix() {
        let data = actor();
        let mut host = Host {
            reenter: true,
            ..Host::default()
        };
        set(&data, 6, MetadataValue::Pose(1), false, &mut host).unwrap();
        assert_eq!(
            host.callback_snapshot,
            Some(vec![(9, MetadataValue::Float(4.0))])
        );
        assert_eq!(
            pack_dirty_values(&data, &mut host).unwrap(),
            Some(vec![(6, MetadataValue::Pose(1))])
        );
        assert!(!set(&data, 6, MetadataValue::Pose(1), false, &mut host).unwrap());
        assert_eq!(host.dimensions, 1);
        host.fail_dimensions = true;
        assert_eq!(
            set(&data, 6, MetadataValue::Pose(1), true, &mut host),
            Err(MetadataError::Host("dimensions failure"))
        );
        assert_eq!(get(&data, 6, &mut host).unwrap(), MetadataValue::Pose(1));
        assert!(!is_dirty(&data, &mut host).unwrap());
    }

    #[test]
    fn canonical_physical_projection_and_boxed_float_equality() {
        let data = actor();
        let mut host = Host::default();
        {
            let mut frame = data.lock().unwrap();
            frame.physical.set_air_supply(20);
            frame.physical.metadata_dirty |= 1 << 8;
        }
        assert_eq!(get(&data, 1, &mut host).unwrap(), MetadataValue::Int(20));
        assert_eq!(
            pack_dirty_values(&data, &mut host).unwrap(),
            Some(vec![(1, MetadataValue::Int(20))])
        );
        assert_eq!(data.lock().unwrap().physical.metadata_dirty, 1 << 8);
        set(&data, 9, MetadataValue::Float(0.0), false, &mut host).unwrap();
        pack_dirty_values(&data, &mut host).unwrap();
        assert!(set(&data, 9, MetadataValue::Float(-0.0), false, &mut host).unwrap());
        set(
            &data,
            9,
            MetadataValue::Float(f32::from_bits(0x7fa00001)),
            false,
            &mut host,
        )
        .unwrap();
        pack_dirty_values(&data, &mut host).unwrap();
        assert!(!set(
            &data,
            9,
            MetadataValue::Float(f32::from_bits(0xffc00000)),
            false,
            &mut host
        )
        .unwrap());
        assert!(!is_dirty(&data, &mut host).unwrap());
    }
}
