//! `EndDragonFight.Data` — the `DragonFight` member of `level.dat` — and the order of the end
//! gateways a new fight draws from the world seed.

use crate::components::decode::Val;
use crate::components::java::hashmap_iteration_order;
use crate::components::text::Primitive;
use crate::nbt::{JavaString, Tag};
use crate::random::LegacyRandomSource;

/// `UUID` as 16 big-endian bytes.
pub type Uuid = [u8; 16];

/// `EndDragonFight.Data`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DragonFightData {
    pub needs_state_scanning: bool,
    pub dragon_killed: bool,
    pub previously_killed: bool,
    pub is_respawning: bool,
    pub dragon_uuid: Option<Uuid>,
    pub exit_portal_location: Option<[i32; 3]>,
    pub gateways: Option<Vec<i32>>,
}

impl DragonFightData {
    /// `EndDragonFight.Data.DEFAULT`.
    pub const DEFAULT: DragonFightData = DragonFightData {
        needs_state_scanning: true,
        dragon_killed: false,
        previously_killed: false,
        is_respawning: false,
        dragon_uuid: None,
        exit_portal_location: None,
        gateways: None,
    };
}

/// `Number.intValue()` of the number a numeric tag boxes: integers narrow, a float or double is cast
/// (truncating, saturating).
fn int_value(value: &Val) -> Option<i32> {
    match value {
        Val::Number(Primitive::Byte(n)) => Some(i32::from(*n)),
        Val::Number(Primitive::Short(n)) => Some(i32::from(*n)),
        Val::Number(Primitive::Int(n)) => Some(*n),
        Val::Number(Primitive::Long(n)) => Some(*n as i32),
        Val::Number(Primitive::Float(n)) => Some(*n as i32),
        Val::Number(Primitive::Double(n)) => Some(*n as i32),
        _ => None,
    }
}

/// `Codec.INT_STREAM` on `NbtOps`: any list or array of numbers.
fn int_stream(value: &Val) -> Option<Vec<i32>> {
    match value {
        Val::List(items) => items.iter().map(int_value).collect(),
        _ => None,
    }
}

/// `Util.fixedSize(stream, N)` after `INT_STREAM`.
fn fixed<const N: usize>(value: &Val) -> Option<[i32; N]> {
    int_stream(value)?.try_into().ok()
}

fn uuid_from_ints(ints: [i32; 4]) -> Uuid {
    let mut out = [0u8; 16];
    for (index, int) in ints.iter().enumerate() {
        out[index * 4..index * 4 + 4].copy_from_slice(&int.to_be_bytes());
    }
    out
}

fn uuid_to_ints(uuid: Uuid) -> [i32; 4] {
    std::array::from_fn(|index| {
        i32::from_be_bytes(uuid[index * 4..index * 4 + 4].try_into().expect("4 bytes"))
    })
}

impl DragonFightData {
    /// `Data.CODEC.parse(NbtOps.INSTANCE, tag)`. `Err` is a value the codec rejects: only a
    /// non-compound can be, because every member either has a default or is lenient.
    pub fn from_nbt(tag: &Tag) -> Result<Self, String> {
        let Tag::Compound(_) = tag else {
            return Err("DragonFight is not a compound".to_owned());
        };
        let member =
            |name: &str| -> Option<Val> { tag.get(name).and_then(|t| Val::from_nbt(t).ok()) };
        // `fieldOf(..).orElse(default)`: a missing or unreadable member is the default.
        let flag =
            |name: &str, default: bool| member(name).and_then(|v| v.boolean()).unwrap_or(default);
        Ok(Self {
            needs_state_scanning: flag("NeedsStateScanning", true),
            dragon_killed: flag("DragonKilled", false),
            previously_killed: flag("PreviouslyKilled", false),
            // `lenientOptionalFieldOf("IsRespawning", false)`: the same default.
            is_respawning: flag("IsRespawning", false),
            dragon_uuid: member("Dragon")
                .and_then(|v| fixed::<4>(&v))
                .map(uuid_from_ints),
            exit_portal_location: member("ExitPortalLocation").and_then(|v| fixed::<3>(&v)),
            gateways: member("Gateways").and_then(|v| match v {
                Val::List(items) => items.iter().map(int_value).collect(),
                _ => None,
            }),
        })
    }

    /// `Data.CODEC.encodeStart(NbtOps.INSTANCE, data)`: the members in field order (`IsRespawning`
    /// only when true, the optional ones only when present), in the order the compound's
    /// `HashMap` iterates them. A list of ints is an int array (`NbtOps`' list collector), an empty
    /// one an empty list.
    pub fn to_nbt(&self) -> Tag {
        let flag = |value: bool| Tag::Byte(i8::from(value));
        let mut entries: Vec<(&str, Tag)> = vec![
            ("NeedsStateScanning", flag(self.needs_state_scanning)),
            ("DragonKilled", flag(self.dragon_killed)),
            ("PreviouslyKilled", flag(self.previously_killed)),
        ];
        if self.is_respawning {
            entries.push(("IsRespawning", flag(true)));
        }
        if let Some(uuid) = self.dragon_uuid {
            entries.push(("Dragon", Tag::IntArray(uuid_to_ints(uuid).to_vec())));
        }
        if let Some(location) = self.exit_portal_location {
            entries.push(("ExitPortalLocation", Tag::IntArray(location.to_vec())));
        }
        if let Some(gateways) = &self.gateways {
            entries.push((
                "Gateways",
                if gateways.is_empty() {
                    Tag::List {
                        element_type: 0,
                        values: Vec::new(),
                    }
                } else {
                    Tag::IntArray(gateways.clone())
                },
            ));
        }
        let keys: Vec<&str> = entries.iter().map(|(key, _)| *key).collect();
        let order = hashmap_iteration_order(&keys);
        let mut slots: Vec<Option<(&str, Tag)>> = entries.into_iter().map(Some).collect();
        Tag::Compound(
            order
                .into_iter()
                .map(|index| {
                    let (key, tag) = slots[index].take().expect("each index once");
                    (JavaString::from(key), tag)
                })
                .collect(),
        )
    }
}

/// The gateway order of a new fight: `0..20` shuffled by `Util.shuffle` with
/// `RandomSource.create(worldSeed)` (a legacy random source).
pub fn default_gateways(world_seed: i64) -> Vec<i32> {
    let mut gateways: Vec<i32> = (0..20).collect();
    let mut random = LegacyRandomSource::new(world_seed);
    for size in (2..=gateways.len()).rev() {
        let other = random
            .next_bounded_i32(size as i32)
            .expect("the bound is positive") as usize;
        gateways.swap(other, size - 1);
    }
    gateways
}
