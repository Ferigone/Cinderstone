//! Java 1.21 Mob lifecycle wrappers (`btp`293–320,372–395,572–592,832).
//!
//! Actual LivingEntity state, RNG, world queries, returned query lists, selectors,
//! body controls and sound/pickup callbacks remain mandatory retained owners.
//! These functions create no actor, alive/dead flag, clock, inventory or scheduler.

use super::ai_step::ProfileEvent;

/// Bound permanently to the same actual Mob lifetime, including after reentry
/// or removal. Reference-like results retain the exact source object and are
/// never re-resolved against a replacement numeric ID. No state borrow may span
/// another host operation; errors retain completed mutations/draws/callbacks.
pub trait MobLifecycleHost {
    type Error;
    type Level;
    type Profiler;
    fn level(&mut self) -> Result<Self::Level, Self::Error>;
    fn level_is_client_side(&mut self, level: &Self::Level) -> Result<bool, Self::Error>;
    fn level_profiler(&mut self, level: &Self::Level) -> Result<Self::Profiler, Self::Error>;
    fn profile(
        &mut self,
        profiler: &mut Self::Profiler,
        event: ProfileEvent,
    ) -> Result<(), Self::Error>;
}

pub trait MobTickHost: MobLifecycleHost {
    fn inherited_living_tick(&mut self) -> Result<(), Self::Error>;
    /// Callback-free actual Entity.tickCount field read.
    fn actor_tick_count(&mut self) -> Result<i32, Self::Error>;
    /// Actual virtual updateControlFlags; the base body is supplied below.
    fn update_control_flags(&mut self) -> Result<(), Self::Error>;
}

/// Source372–379. Complete Living.tick precedes the fresh side/tick reads.
pub fn tick<H: MobTickHost>(host: &mut H) -> Result<(), H::Error> {
    host.inherited_living_tick()?;
    let level = host.level()?;
    if !host.level_is_client_side(&level)? && host.actor_tick_count()? % 5 == 0 {
        host.update_control_flags()?;
    }
    Ok(())
}

pub trait MobBaseTickHost: MobLifecycleHost {
    type EntityRandom;
    fn inherited_living_base_tick(&mut self) -> Result<(), Self::Error>;
    fn is_alive(&mut self) -> Result<bool, Self::Error>;
    /// Retain the original Entity.random object; never seed/fork/substitute it.
    fn entity_random(&mut self) -> Result<Self::EntityRandom, Self::Error>;
    fn random_next_bounded_i32(
        &mut self,
        random: &Self::EntityRandom,
        bound: i32,
    ) -> Result<i32, Self::Error>;
    /// Callback-free reads/writes of the actual Mob.ambientSoundTime field.
    fn ambient_sound_time(&mut self) -> Result<i32, Self::Error>;
    fn set_ambient_sound_time(&mut self, value: i32) -> Result<(), Self::Error>;
    fn ambient_sound_interval(&mut self) -> Result<i32, Self::Error>;
    /// Actual virtual playAmbientSound, including the subtype's sound choice
    /// and all actual sound, pitch, level RNG/listener and packet callbacks.
    fn play_ambient_sound(&mut self) -> Result<(), Self::Error>;
}

/// Complete base getAmbientSoundInterval293. Actual subtype dispatch still
/// belongs to the mandatory host operation; this is not its fallback default.
pub fn base_ambient_sound_interval() -> i32 {
    80
}

/// Source private resetAmbientSoundTime319–320, also used by Mob hurt sound.
pub fn reset_ambient_sound_time<H: MobBaseTickHost>(host: &mut H) -> Result<(), H::Error> {
    let interval = host.ambient_sound_interval()?;
    host.set_ambient_sound_time(interval.wrapping_neg())
}

/// Source302–310. Alive gates the RNG draw and postincrement, not the profiler.
/// The draw precedes reading ambientSoundTime, and comparison uses its OLD value.
/// Pop obtains a fresh Level/profiler and is not synthesized after an error.
pub fn base_tick<H: MobBaseTickHost>(host: &mut H) -> Result<(), H::Error> {
    host.inherited_living_base_tick()?;
    let level = host.level()?;
    let mut profiler = host.level_profiler(&level)?;
    host.profile(&mut profiler, ProfileEvent::Push("mobBaseTick"))?;
    if host.is_alive()? {
        let random = host.entity_random()?;
        let draw = host.random_next_bounded_i32(&random, 1000)?;
        let previous = host.ambient_sound_time()?;
        host.set_ambient_sound_time(previous.wrapping_add(1))?;
        if draw < previous {
            reset_ambient_sound_time(host)?;
            host.play_ambient_sound()?;
        }
    }
    let level = host.level()?;
    let mut profiler = host.level_profiler(&level)?;
    host.profile(&mut profiler, ProfileEvent::Pop)
}

pub trait MobAmbientSoundHost {
    type Error;
    type Sound;
    fn ambient_sound(&mut self) -> Result<Option<Self::Sound>, Self::Error>;
    /// Actual LivingEntity.playSound(nullable SoundEvent), source1338–1341.
    /// A nonnull sound obtains actual volume then pitch then Entity.playSound;
    /// a null sound performs none of those later operations.
    fn living_play_sound(&mut self, sound: Option<&Self::Sound>) -> Result<(), Self::Error>;
}

/// Source297–298. Nullable virtual sound selection is preserved as-is.
pub fn play_ambient_sound<H: MobAmbientSoundHost>(host: &mut H) -> Result<(), H::Error> {
    let sound = host.ambient_sound()?;
    host.living_play_sound(sound.as_ref())
}

pub trait MobControlFlagsHost {
    type Error;
    type Passenger;
    type Vehicle;
    type Selector;
    fn controlling_passenger(&mut self) -> Result<Option<Self::Passenger>, Self::Error>;
    fn passenger_is_mob(&mut self, passenger: &Self::Passenger) -> Result<bool, Self::Error>;
    fn vehicle(&mut self) -> Result<Option<Self::Vehicle>, Self::Error>;
    fn vehicle_is_boat(&mut self, vehicle: &Self::Vehicle) -> Result<bool, Self::Error>;
    /// Callback-free read of this Mob's actual final goalSelector field.
    fn goal_selector(&mut self) -> Result<Self::Selector, Self::Error>;
    /// Each operation is the actual GoalSelector.setControlFlag for its named
    /// source Goal.Flag, not a locally buffered control-mask write.
    fn set_goal_move_enabled(
        &mut self,
        selector: &Self::Selector,
        enabled: bool,
    ) -> Result<(), Self::Error>;
    fn set_goal_jump_enabled(
        &mut self,
        selector: &Self::Selector,
        enabled: bool,
    ) -> Result<(), Self::Error>;
    fn set_goal_look_enabled(
        &mut self,
        selector: &Self::Selector,
        enabled: bool,
    ) -> Result<(), Self::Error>;
}

/// Source385–390. Capture passenger/vehicle facts before MOVE→JUMP→LOOK calls.
pub fn update_control_flags<H: MobControlFlagsHost>(host: &mut H) -> Result<(), H::Error> {
    let move_enabled = match host.controlling_passenger()? {
        Some(passenger) => !host.passenger_is_mob(&passenger)?,
        None => true,
    };
    let not_boat = match host.vehicle()? {
        Some(vehicle) => !host.vehicle_is_boat(&vehicle)?,
        None => true,
    };
    let selector = host.goal_selector()?;
    host.set_goal_move_enabled(&selector, move_enabled)?;
    let selector = host.goal_selector()?;
    host.set_goal_jump_enabled(&selector, move_enabled && not_boat)?;
    let selector = host.goal_selector()?;
    host.set_goal_look_enabled(&selector, move_enabled)
}

pub trait MobHeadTurnHost {
    type Error;
    type BodyControl;
    fn body_rotation_control(&mut self) -> Result<Self::BodyControl, Self::Error>;
    /// Actual virtual BodyRotationControl.clientTick on the retained object.
    /// Base controls use controls::body_rotation::client_tick; derived controls
    /// retain their true dispatch. No new head/body state may substitute.
    fn body_rotation_client_tick(&mut self, control: &Self::BodyControl)
        -> Result<(), Self::Error>;
}

/// Source394–395. The first float is unused; return the second unchanged after
/// the actual body-control call, including its signed-zero/NaN bit pattern.
pub fn tick_head_turn<H: MobHeadTurnHost>(
    host: &mut H,
    _first: f32,
    second: f32,
) -> Result<f32, H::Error> {
    let control = host.body_rotation_control()?;
    host.body_rotation_client_tick(&control)?;
    Ok(second)
}

pub trait MobLootingHost: MobLifecycleHost {
    type Reach;
    type Bounds;
    type ItemList;
    type ItemIterator;
    type ItemEntity;
    type Item;

    fn inherited_living_ai_step(&mut self) -> Result<(), Self::Error>;
    fn can_pick_up_loot(&mut self) -> Result<bool, Self::Error>;
    fn is_alive(&mut self) -> Result<bool, Self::Error>;
    /// Callback-free actual LivingEntity.dead field read, not another flag.
    fn dead(&mut self) -> Result<bool, Self::Error>;
    fn mob_griefing(&mut self, level: &Self::Level) -> Result<bool, Self::Error>;
    fn pickup_reach(&mut self) -> Result<Self::Reach, Self::Error>;
    fn bounding_box(&mut self) -> Result<Self::Bounds, Self::Error>;
    fn reach_x(&mut self, reach: &Self::Reach) -> Result<i32, Self::Error>;
    fn reach_y(&mut self, reach: &Self::Reach) -> Result<i32, Self::Error>;
    fn reach_z(&mut self, reach: &Self::Reach) -> Result<i32, Self::Error>;
    /// Actual AABB.inflate immutable value operation; do not query the world.
    fn inflate_bounds(
        &mut self,
        bounds: &Self::Bounds,
        x: f64,
        y: f64,
        z: f64,
    ) -> Result<Self::Bounds, Self::Error>;
    /// Actual Level.getEntitiesOfClass(ItemEntity.class, inflated bounds), with
    /// source query/class/order semantics and canonical retained ItemEntities.
    fn nearby_items(
        &mut self,
        level: &Self::Level,
        bounds: &Self::Bounds,
    ) -> Result<Self::ItemList, Self::Error>;
    /// The actual returned list/iterator owner, not a fresh live-world scan.
    fn items_iterator(&mut self, list: Self::ItemList) -> Result<Self::ItemIterator, Self::Error>;
    fn iterator_has_next(&mut self, iterator: &Self::ItemIterator) -> Result<bool, Self::Error>;
    fn iterator_next(
        &mut self,
        iterator: &mut Self::ItemIterator,
    ) -> Result<Self::ItemEntity, Self::Error>;
    fn item_entity_is_removed(&mut self, entity: &Self::ItemEntity) -> Result<bool, Self::Error>;
    fn item_entity_item(&mut self, entity: &Self::ItemEntity) -> Result<Self::Item, Self::Error>;
    fn item_is_empty(&mut self, item: &Self::Item) -> Result<bool, Self::Error>;
    fn item_has_pickup_delay(&mut self, entity: &Self::ItemEntity) -> Result<bool, Self::Error>;
    fn wants_to_pick_up(&mut self, item: &Self::Item) -> Result<bool, Self::Error>;
    /// Actual virtual Mob.pickUpItem, with real item/equipment/removal owners.
    fn pick_up_item(&mut self, entity: &Self::ItemEntity) -> Result<(), Self::Error>;
}

/// Source572–589. This is the Mob wrapper AFTER complete Living.aiStep, not its
/// effective-AI/serverAiStep replacement. Even a rejected gate profiles looting.
pub fn ai_step<H: MobLootingHost>(host: &mut H) -> Result<(), H::Error> {
    host.inherited_living_ai_step()?;
    let level = host.level()?;
    let mut profiler = host.level_profiler(&level)?;
    host.profile(&mut profiler, ProfileEvent::Push("looting"))?;
    let level = host.level()?;
    if !host.level_is_client_side(&level)?
        && host.can_pick_up_loot()?
        && host.is_alive()?
        && !host.dead()?
    {
        let level = host.level()?;
        if host.mob_griefing(&level)? {
            let reach = host.pickup_reach()?;
            // JVM obtains query Level before the bounding-box/reach operands.
            let level = host.level()?;
            let bounds = host.bounding_box()?;
            let x = f64::from(host.reach_x(&reach)?);
            let y = f64::from(host.reach_y(&reach)?);
            let z = f64::from(host.reach_z(&reach)?);
            let bounds = host.inflate_bounds(&bounds, x, y, z)?;
            let items = host.nearby_items(&level, &bounds)?;
            let mut iterator = host.items_iterator(items)?;
            while host.iterator_has_next(&iterator)? {
                let item_entity = host.iterator_next(&mut iterator)?;
                if host.item_entity_is_removed(&item_entity)? {
                    continue;
                }
                let item = host.item_entity_item(&item_entity)?;
                if host.item_is_empty(&item)? || host.item_has_pickup_delay(&item_entity)? {
                    continue;
                }
                // The source asks getItem AGAIN after the empty/delay checks.
                let item = host.item_entity_item(&item_entity)?;
                if host.wants_to_pick_up(&item)? {
                    host.pick_up_item(&item_entity)?;
                }
            }
        }
    }
    let level = host.level()?;
    let mut profiler = host.level_profiler(&level)?;
    host.profile(&mut profiler, ProfileEvent::Pop)
}

/// The complete protected Mob.customServerAiStep832 body is empty. This exact
/// inherited body is usable by Bat; it is not a default for missing Mob AI.
pub fn base_custom_server_ai_step() {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::LegacyRandomSource;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    // Controlled real-field/owner recorder. This does not instantiate a Mob,
    // replace Living.tick or certify a production sound/query/pickup owner.
    struct Host {
        random: Rc<RefCell<LegacyRandomSource>>,
        alive: bool,
        ambient: i32,
        interval: i32,
        level: i32,
        client: bool,
        tick: i32,
        advance_tick: bool,
        passenger_mob: bool,
        boat: bool,
        reenter_flags: bool,
        flags: Vec<(&'static str, bool)>,
        fail: Option<&'static str>,
        sound: Option<&'static str>,
        calls: Vec<String>,
    }
    impl Host {
        fn new() -> Self {
            Self {
                random: Rc::new(RefCell::new(LegacyRandomSource::new(38))),
                alive: true,
                ambient: 1000,
                interval: 80,
                level: 1,
                client: false,
                tick: 4,
                advance_tick: true,
                passenger_mob: false,
                boat: false,
                reenter_flags: false,
                flags: Vec::new(),
                fail: None,
                sound: Some("actual sound handle"),
                calls: Vec::new(),
            }
        }
        fn event(&mut self, event: impl Into<String>) -> Result<(), &'static str> {
            let event = event.into();
            let fail = self.fail == Some(event.as_str());
            self.calls.push(event);
            if fail {
                Err("controlled callback failure")
            } else {
                Ok(())
            }
        }
    }
    impl MobLifecycleHost for Host {
        type Error = &'static str;
        type Level = i32;
        type Profiler = i32;
        fn level(&mut self) -> Result<i32, Self::Error> {
            self.event(format!("level:{}", self.level))?;
            Ok(self.level)
        }
        fn level_is_client_side(&mut self, _: &i32) -> Result<bool, Self::Error> {
            self.event("client")?;
            Ok(self.client)
        }
        fn level_profiler(&mut self, level: &i32) -> Result<i32, Self::Error> {
            self.event(format!("profiler:{level}"))?;
            Ok(*level)
        }
        fn profile(&mut self, profiler: &mut i32, event: ProfileEvent) -> Result<(), Self::Error> {
            self.event(format!("{event:?}:{profiler}"))
        }
    }
    impl MobBaseTickHost for Host {
        type EntityRandom = Rc<RefCell<LegacyRandomSource>>;
        fn inherited_living_base_tick(&mut self) -> Result<(), Self::Error> {
            self.event("living.baseTick")
        }
        fn is_alive(&mut self) -> Result<bool, Self::Error> {
            self.event("alive")?;
            Ok(self.alive)
        }
        fn entity_random(&mut self) -> Result<Self::EntityRandom, Self::Error> {
            self.event("entity.random")?;
            Ok(self.random.clone())
        }
        fn random_next_bounded_i32(
            &mut self,
            random: &Self::EntityRandom,
            bound: i32,
        ) -> Result<i32, Self::Error> {
            assert!(Rc::ptr_eq(random, &self.random));
            self.event(format!("nextInt:{bound}"))?;
            Ok(random.borrow_mut().next_bounded_i32(bound).unwrap())
        }
        fn ambient_sound_time(&mut self) -> Result<i32, Self::Error> {
            self.event("ambient.read")?;
            Ok(self.ambient)
        }
        fn set_ambient_sound_time(&mut self, value: i32) -> Result<(), Self::Error> {
            self.event(format!("ambient.write:{value}"))?;
            self.ambient = value;
            Ok(())
        }
        fn ambient_sound_interval(&mut self) -> Result<i32, Self::Error> {
            self.event("interval")?;
            Ok(self.interval)
        }
        fn play_ambient_sound(&mut self) -> Result<(), Self::Error> {
            self.event("virtual.playAmbient")?;
            // Exercise this actual callable source body on the same host.
            play_ambient_sound(self)?;
            self.level = 2;
            Ok(())
        }
    }
    impl MobAmbientSoundHost for Host {
        type Error = &'static str;
        type Sound = &'static str;
        fn ambient_sound(&mut self) -> Result<Option<Self::Sound>, Self::Error> {
            self.event("virtual.getAmbient")?;
            Ok(self.sound)
        }
        fn living_play_sound(&mut self, sound: Option<&&'static str>) -> Result<(), Self::Error> {
            self.event(if sound.is_some() {
                "Living.playSound:some"
            } else {
                "Living.playSound:null"
            })
        }
    }
    impl MobTickHost for Host {
        fn inherited_living_tick(&mut self) -> Result<(), Self::Error> {
            self.event("living.tick")?;
            if self.advance_tick {
                self.tick += 1;
            }
            Ok(())
        }
        fn actor_tick_count(&mut self) -> Result<i32, Self::Error> {
            self.event("tickCount")?;
            Ok(self.tick)
        }
        fn update_control_flags(&mut self) -> Result<(), Self::Error> {
            self.event("virtual.updateFlags")?;
            update_control_flags(self)
        }
    }
    impl MobControlFlagsHost for Host {
        type Error = &'static str;
        type Passenger = bool;
        type Vehicle = bool;
        type Selector = i32;
        fn controlling_passenger(&mut self) -> Result<Option<bool>, Self::Error> {
            self.event("passenger")?;
            Ok(Some(self.passenger_mob))
        }
        fn passenger_is_mob(&mut self, value: &bool) -> Result<bool, Self::Error> {
            self.event("passenger.instanceof.Mob")?;
            Ok(*value)
        }
        fn vehicle(&mut self) -> Result<Option<bool>, Self::Error> {
            self.event("vehicle")?;
            Ok(Some(self.boat))
        }
        fn vehicle_is_boat(&mut self, value: &bool) -> Result<bool, Self::Error> {
            self.event("vehicle.instanceof.Boat")?;
            Ok(*value)
        }
        fn goal_selector(&mut self) -> Result<i32, Self::Error> {
            self.event("goalSelector")?;
            Ok(7)
        }
        fn set_goal_move_enabled(
            &mut self,
            selector: &i32,
            value: bool,
        ) -> Result<(), Self::Error> {
            assert_eq!(*selector, 7);
            self.event(format!("MOVE:{value}"))?;
            self.flags.push(("MOVE", value));
            if self.reenter_flags {
                self.reenter_flags = false;
                self.passenger_mob = true;
                self.boat = true;
                update_control_flags(self)?;
            }
            Ok(())
        }
        fn set_goal_jump_enabled(
            &mut self,
            selector: &i32,
            value: bool,
        ) -> Result<(), Self::Error> {
            assert_eq!(*selector, 7);
            self.event(format!("JUMP:{value}"))?;
            self.flags.push(("JUMP", value));
            Ok(())
        }
        fn set_goal_look_enabled(
            &mut self,
            selector: &i32,
            value: bool,
        ) -> Result<(), Self::Error> {
            assert_eq!(*selector, 7);
            self.event(format!("LOOK:{value}"))?;
            self.flags.push(("LOOK", value));
            Ok(())
        }
    }
    impl MobHeadTurnHost for Host {
        type Error = &'static str;
        type BodyControl = i32;
        fn body_rotation_control(&mut self) -> Result<i32, Self::Error> {
            self.event("bodyControl")?;
            Ok(8)
        }
        fn body_rotation_client_tick(&mut self, control: &i32) -> Result<(), Self::Error> {
            assert_eq!(*control, 8);
            self.event("body.clientTick")
        }
    }

    #[test]
    fn ambient_draw_precedes_postincrement_and_fresh_pop_preserves_error_prefix() {
        let mut host = Host::new();
        let mut expected = LegacyRandomSource::new(38);
        expected.next_bounded_i32(1000).unwrap();
        base_tick(&mut host).unwrap();
        assert_eq!(host.ambient, -80);
        assert_eq!(host.random.borrow().state48(), expected.state48());
        assert_eq!(
            host.calls,
            [
                "living.baseTick",
                "level:1",
                "profiler:1",
                "Push(\"mobBaseTick\"):1",
                "alive",
                "entity.random",
                "nextInt:1000",
                "ambient.read",
                "ambient.write:1001",
                "interval",
                "ambient.write:-80",
                "virtual.playAmbient",
                "virtual.getAmbient",
                "Living.playSound:some",
                "level:2",
                "profiler:2",
                "Pop:2"
            ]
        );

        let mut failed = Host::new();
        failed.ambient = i32::MAX;
        failed.interval = i32::MIN;
        failed.fail = Some("virtual.playAmbient");
        assert_eq!(base_tick(&mut failed), Err("controlled callback failure"));
        assert_eq!(failed.ambient, i32::MIN); // postincrement and reset both wrap
        assert_eq!(failed.random.borrow().state48(), expected.state48());
        assert_eq!(failed.calls.last().unwrap(), "virtual.playAmbient");
        assert!(!failed.calls.iter().any(|call| call.starts_with("Pop")));

        let mut alive_zero = Host::new();
        alive_zero.ambient = 0;
        base_tick(&mut alive_zero).unwrap();
        assert_eq!(alive_zero.ambient, 1);
        assert!(!alive_zero.calls.iter().any(|call| call == "interval"));
        let mut dead = Host::new();
        dead.alive = false;
        let old_random = dead.random.borrow().state48();
        base_tick(&mut dead).unwrap();
        assert_eq!(dead.random.borrow().state48(), old_random);
        assert_eq!(dead.ambient, 1000);
        assert!(!dead.calls.iter().any(|call| call == "ambient.read"));
        let mut client = Host::new();
        client.client = true;
        client.sound = None;
        base_tick(&mut client).unwrap(); // this body has NO client-side gate
        assert_eq!(client.random.borrow().state48(), expected.state48());
        assert!(client
            .calls
            .iter()
            .any(|call| call == "Living.playSound:null"));
        assert!(!client.calls.iter().any(|call| call == "client"));
    }

    #[test]
    fn tick_uses_post_parent_tick_and_flag_callbacks_keep_pre_reentry_facts() {
        let mut host = Host::new();
        host.reenter_flags = true;
        tick(&mut host).unwrap();
        assert_eq!(
            &host.calls[..5],
            [
                "living.tick",
                "level:1",
                "client",
                "tickCount",
                "virtual.updateFlags"
            ]
        );
        assert_eq!(
            host.flags,
            [
                ("MOVE", true),
                ("MOVE", false),
                ("JUMP", false),
                ("LOOK", false),
                ("JUMP", true),
                ("LOOK", true)
            ]
        );
        let mut client = Host::new();
        client.client = true;
        tick(&mut client).unwrap();
        assert_eq!(client.calls, ["living.tick", "level:1", "client"]);
        let mut negative = Host::new();
        negative.tick = -5;
        negative.advance_tick = false;
        tick(&mut negative).unwrap();
        assert_eq!(
            negative.flags,
            [("MOVE", true), ("JUMP", true), ("LOOK", true)]
        );
        let mut failed = Host::new();
        failed.fail = Some("JUMP:true");
        assert!(update_control_flags(&mut failed).is_err());
        assert_eq!(failed.flags, [("MOVE", true)]);
        assert_eq!(failed.calls.last().unwrap(), "JUMP:true");
        let payload = f32::from_bits(0x7fa1_2345);
        assert_eq!(
            tick_head_turn(&mut host, 900.0, payload).unwrap().to_bits(),
            payload.to_bits()
        );
        assert_eq!(
            &host.calls[host.calls.len() - 2..],
            ["bodyControl", "body.clientTick"]
        );
    }

    struct Item {
        id: i32,
        removed: bool,
        empty: bool,
        delay: bool,
        stack: i32,
    }
    type Entity = Rc<RefCell<Item>>;
    struct LootHost {
        base: Host,
        can_pick_up: bool,
        alive: bool,
        dead: bool,
        griefing: bool,
        items: Vec<Entity>,
        picked: Vec<i32>,
        change_after_empty: bool,
    }
    impl LootHost {
        fn new() -> Self {
            let item = |id, removed, empty, delay| {
                Rc::new(RefCell::new(Item {
                    id,
                    removed,
                    empty,
                    delay,
                    stack: id * 10,
                }))
            };
            Self {
                base: Host::new(),
                can_pick_up: true,
                alive: true,
                dead: false,
                griefing: true,
                items: vec![
                    item(1, true, false, false),
                    item(2, false, true, false),
                    item(3, false, false, true),
                    item(4, false, false, false),
                ],
                picked: Vec::new(),
                change_after_empty: true,
            }
        }
        fn event(&mut self, call: impl Into<String>) -> Result<(), &'static str> {
            self.base.event(call)
        }
    }
    impl MobLifecycleHost for LootHost {
        type Error = &'static str;
        type Level = i32;
        type Profiler = i32;
        fn level(&mut self) -> Result<i32, Self::Error> {
            self.base.level()
        }
        fn level_is_client_side(&mut self, level: &i32) -> Result<bool, Self::Error> {
            self.base.level_is_client_side(level)
        }
        fn level_profiler(&mut self, level: &i32) -> Result<i32, Self::Error> {
            self.base.level_profiler(level)
        }
        fn profile(&mut self, profiler: &mut i32, event: ProfileEvent) -> Result<(), Self::Error> {
            self.base.profile(profiler, event)
        }
    }
    impl MobLootingHost for LootHost {
        type Reach = [i32; 3];
        type Bounds = [f64; 6];
        type ItemList = Vec<Entity>;
        type ItemIterator = VecDeque<Entity>;
        type ItemEntity = Entity;
        type Item = (i32, bool);
        fn inherited_living_ai_step(&mut self) -> Result<(), Self::Error> {
            self.event("living.aiStep")
        }
        fn can_pick_up_loot(&mut self) -> Result<bool, Self::Error> {
            self.event("canPickUpLoot")?;
            Ok(self.can_pick_up)
        }
        fn is_alive(&mut self) -> Result<bool, Self::Error> {
            self.event("alive")?;
            Ok(self.alive)
        }
        fn dead(&mut self) -> Result<bool, Self::Error> {
            self.event("dead.field")?;
            Ok(self.dead)
        }
        fn mob_griefing(&mut self, _: &i32) -> Result<bool, Self::Error> {
            self.event("mobGriefing")?;
            Ok(self.griefing)
        }
        fn pickup_reach(&mut self) -> Result<[i32; 3], Self::Error> {
            self.event("reach")?;
            Ok([1, 0, 1])
        }
        fn bounding_box(&mut self) -> Result<[f64; 6], Self::Error> {
            self.event("bounds")?;
            Ok([0., 1., 2., 3., 4., 5.])
        }
        fn reach_x(&mut self, reach: &[i32; 3]) -> Result<i32, Self::Error> {
            self.event("reach.x")?;
            Ok(reach[0])
        }
        fn reach_y(&mut self, reach: &[i32; 3]) -> Result<i32, Self::Error> {
            self.event("reach.y")?;
            Ok(reach[1])
        }
        fn reach_z(&mut self, reach: &[i32; 3]) -> Result<i32, Self::Error> {
            self.event("reach.z")?;
            Ok(reach[2])
        }
        fn inflate_bounds(
            &mut self,
            b: &[f64; 6],
            x: f64,
            y: f64,
            z: f64,
        ) -> Result<[f64; 6], Self::Error> {
            self.event("inflate")?;
            Ok([b[0] - x, b[1] - y, b[2] - z, b[3] + x, b[4] + y, b[5] + z])
        }
        fn nearby_items(
            &mut self,
            level: &i32,
            bounds: &[f64; 6],
        ) -> Result<Vec<Entity>, Self::Error> {
            assert_eq!(*level, 1);
            assert_eq!(*bounds, [-1., 1., 1., 4., 4., 6.]);
            self.event("query.items")?;
            Ok(self.items.clone())
        }
        fn items_iterator(&mut self, list: Vec<Entity>) -> Result<VecDeque<Entity>, Self::Error> {
            self.event("items.iterator")?;
            Ok(list.into())
        }
        fn iterator_has_next(&mut self, iterator: &VecDeque<Entity>) -> Result<bool, Self::Error> {
            self.event("iterator.hasNext")?;
            Ok(!iterator.is_empty())
        }
        fn iterator_next(
            &mut self,
            iterator: &mut VecDeque<Entity>,
        ) -> Result<Entity, Self::Error> {
            self.event("iterator.next")?;
            Ok(iterator.pop_front().unwrap())
        }
        fn item_entity_is_removed(&mut self, entity: &Entity) -> Result<bool, Self::Error> {
            self.event(format!("removed:{}", entity.borrow().id))?;
            Ok(entity.borrow().removed)
        }
        fn item_entity_item(&mut self, entity: &Entity) -> Result<(i32, bool), Self::Error> {
            self.event(format!("item:{}", entity.borrow().id))?;
            let item = entity.borrow();
            Ok((item.stack, item.empty))
        }
        fn item_is_empty(&mut self, item: &(i32, bool)) -> Result<bool, Self::Error> {
            self.event(format!("empty:{}", item.0))?;
            if item.0 == 40 && self.change_after_empty {
                self.items[3].borrow_mut().stack = 41;
            }
            Ok(item.1)
        }
        fn item_has_pickup_delay(&mut self, entity: &Entity) -> Result<bool, Self::Error> {
            self.event(format!("delay:{}", entity.borrow().id))?;
            Ok(entity.borrow().delay)
        }
        fn wants_to_pick_up(&mut self, item: &(i32, bool)) -> Result<bool, Self::Error> {
            self.event(format!("wants:{}", item.0))?;
            Ok(true)
        }
        fn pick_up_item(&mut self, entity: &Entity) -> Result<(), Self::Error> {
            let id = entity.borrow().id;
            self.picked.push(id);
            entity.borrow_mut().removed = true;
            self.base.level = 2;
            self.event(format!("pickup:{id}"))
        }
    }

    #[test]
    fn looting_gate_query_order_reread_and_pickup_error_keep_real_prefix() {
        let mut host = LootHost::new();
        ai_step(&mut host).unwrap();
        assert_eq!(
            &host.base.calls[..18],
            [
                "living.aiStep",
                "level:1",
                "profiler:1",
                "Push(\"looting\"):1",
                "level:1",
                "client",
                "canPickUpLoot",
                "alive",
                "dead.field",
                "level:1",
                "mobGriefing",
                "reach",
                "level:1",
                "bounds",
                "reach.x",
                "reach.y",
                "reach.z",
                "inflate"
            ]
        );
        assert_eq!(host.picked, [4]);
        let calls = &host.base.calls;
        let at = calls.iter().position(|call| call == "removed:4").unwrap();
        assert_eq!(
            &calls[at..at + 7],
            [
                "removed:4",
                "item:4",
                "empty:40",
                "delay:4",
                "item:4",
                "wants:41",
                "pickup:4"
            ]
        );
        assert!(!calls
            .iter()
            .any(|call| call == "item:1" || call == "delay:2" || call == "wants:30"));
        assert_eq!(
            &calls[calls.len() - 3..],
            ["level:2", "profiler:2", "Pop:2"]
        );
        let mut failed = LootHost::new();
        failed.base.fail = Some("pickup:4");
        assert!(ai_step(&mut failed).is_err());
        assert!(failed.items[3].borrow().removed);
        assert_eq!(failed.picked, [4]);
        assert_eq!(failed.base.calls.last().unwrap(), "pickup:4");
        for gate in 0..5 {
            let mut gated = LootHost::new();
            match gate {
                0 => gated.base.client = true,
                1 => gated.can_pick_up = false,
                2 => gated.alive = false,
                3 => gated.dead = true,
                _ => gated.griefing = false,
            }
            ai_step(&mut gated).unwrap();
            assert!(gated.picked.is_empty());
            assert!(!gated.base.calls.iter().any(|call| call == "query.items"));
            assert_eq!(gated.base.calls.last().unwrap(), "Pop:1");
            if gate == 0 {
                assert!(!gated.base.calls.iter().any(|call| call == "canPickUpLoot"));
            }
            if gate == 1 {
                assert!(!gated.base.calls.iter().any(|call| call == "alive"));
            }
            if gate == 2 {
                assert!(!gated.base.calls.iter().any(|call| call == "dead.field"));
            }
            if gate == 3 {
                assert!(!gated.base.calls.iter().any(|call| call == "mobGriefing"));
            }
        }
    }
}
