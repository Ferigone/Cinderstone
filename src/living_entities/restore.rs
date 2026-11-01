//! Java 1.21 ServerPlayer.restoreFrom (`aqv.a(aqv,boolean)`, 1415–1467).
//!
//! This executor transfers only the source-selected fields and operations. It
//! does not clone a lifecycle or own actors, health, food, inventories, RNG,
//! sessions or persistence. Every host operation is mandatory and fallible;
//! errors retain the executed field/callback prefix rather than rolling back.

use crate::effects::EffectInstance;

/// Real old/new actor and canonical object owners at each original callsite.
/// Handles must retain actual object identity through callbacks and ID reuse.
/// A respawn caller supplies its genuinely newly constructed actor; this body
/// does not construct one, replace the connection, change its ID or re-admit it.
///
/// Shared fields below must retain the SAME source objects (including FoodData,
/// WardenSpawnTracker, RemoteChatSession and EnderChest), not independent clones.
/// In contrast, inventory and recipe-book operations mutate their actual new
/// constructor-owned target objects through their source methods. No callback
/// has a successful fallback for an unavailable owner.
pub trait ServerPlayerRestoreHost {
    type Error;
    type Actor: ?Sized;
    type WardenSpawnTracker;
    type RemoteChatSession;
    type GameMode;
    type GameType;
    type AttributeMap;
    type Inventory;
    type FoodData;
    type EffectHolder: Clone;
    type EffectCursor;
    type EffectLease;
    type PortalProcessor;
    type EnderChest;
    type EntityData;
    type RecipeBook;
    type NetherPosition;
    type ChunkTrackingView;
    type ShoulderCompound;
    /// Actual Optional<GlobalPos> object/value, including empty state.
    type LastDeathLocation;

    fn warden_spawn_tracker(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Self::WardenSpawnTracker, Self::Error>;
    fn set_warden_spawn_tracker(
        &mut self,
        actor: &Self::Actor,
        tracker: Self::WardenSpawnTracker,
    ) -> Result<(), Self::Error>;
    fn remote_chat_session(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Option<Self::RemoteChatSession>, Self::Error>;
    fn set_remote_chat_session(
        &mut self,
        actor: &Self::Actor,
        session: Option<Self::RemoteChatSession>,
    ) -> Result<(), Self::Error>;
    fn game_mode(&mut self, actor: &Self::Actor) -> Result<Self::GameMode, Self::Error>;
    fn current_game_type(&mut self, mode: &Self::GameMode) -> Result<Self::GameType, Self::Error>;
    fn previous_game_type(
        &mut self,
        mode: &Self::GameMode,
    ) -> Result<Option<Self::GameType>, Self::Error>;
    /// Actual new.gameMode.setGameModeForPlayer(current, previous), including
    /// its real game-type/ability mutation. The target receiver was read first.
    fn set_game_mode(
        &mut self,
        mode: &Self::GameMode,
        current: Self::GameType,
        previous: Option<Self::GameType>,
    ) -> Result<(), Self::Error>;
    /// Actual virtual ServerPlayer.onUpdateAbilities, including its source
    /// packets and attribute/game-mode callbacks on the retained connection.
    fn on_update_abilities(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn attributes(&mut self, actor: &Self::Actor) -> Result<Self::AttributeMap, Self::Error>;
    /// Actual target.assignBaseValues(oldAttributes), not assignAllValues.
    /// Retain source iteration, canonical definitions and dirty-listener work;
    /// effect/equipment modifiers must not be copied wholesale from the old map.
    fn assign_attribute_base_values(
        &mut self,
        target: &Self::AttributeMap,
        old: &Self::AttributeMap,
    ) -> Result<(), Self::Error>;
    fn max_health(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn health(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Actual LivingEntity.setHealth with the current MAX_HEALTH and metadata.
    fn set_health(&mut self, actor: &Self::Actor, health: f32) -> Result<(), Self::Error>;
    fn inventory(&mut self, actor: &Self::Actor) -> Result<Self::Inventory, Self::Error>;
    /// Actual Inventory.replaceWith on the retained new inventory and old
    /// inventory, with the original stack/count/selection and mutation order.
    /// Source getItem/setItem retains actual old stack references, not independent
    /// ItemStack clones; do not duplicate ownership while the old object remains.
    fn replace_inventory(
        &mut self,
        target: &Self::Inventory,
        old: &Self::Inventory,
    ) -> Result<(), Self::Error>;
    fn food_data(&mut self, actor: &Self::Actor) -> Result<Self::FoodData, Self::Error>;
    fn set_food_data(
        &mut self,
        actor: &Self::Actor,
        food: Self::FoodData,
    ) -> Result<(), Self::Error>;

    /// The real old.getActiveEffects().iterator(), not a cloned/sorted list.
    fn active_effects_iterator(
        &mut self,
        old: &Self::Actor,
    ) -> Result<Self::EffectCursor, Self::Error>;
    /// Actual hasNext then (only when true) next, including the original live
    /// iterator's fail-fast boundary. Lease retains that canonical selected
    /// object despite replacement/removal of the map value during callbacks.
    fn next_active_effect(
        &mut self,
        cursor: &mut Self::EffectCursor,
    ) -> Result<Option<Self::EffectLease>, Self::Error>;
    /// A callback-free read of that exact canonical object. Release its borrow
    /// before any actor callback; a key lookup or detached snapshot is invalid.
    fn with_effect_instance<R>(
        &mut self,
        lease: &Self::EffectLease,
        access: impl FnOnce(&EffectInstance<Self::EffectHolder>) -> R,
    ) -> Result<R, Self::Error>;
    /// Actual new.addEffect on the newly copy-constructed instance. Adopt this
    /// exact object into its real owner when the source admits it, with all
    /// source instance/attribute/packet/criterion callbacks. Do not replace it
    /// with a second copy or omit work when its eventual boolean is false.
    fn add_effect(
        &mut self,
        actor: &Self::Actor,
        copied: EffectInstance<Self::EffectHolder>,
    ) -> Result<bool, Self::Error>;
    fn experience_level(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn set_experience_level(&mut self, actor: &Self::Actor, level: i32) -> Result<(), Self::Error>;
    fn total_experience(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn set_total_experience(&mut self, actor: &Self::Actor, total: i32) -> Result<(), Self::Error>;
    fn experience_progress(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn set_experience_progress(
        &mut self,
        actor: &Self::Actor,
        progress: f32,
    ) -> Result<(), Self::Error>;
    /// Actual virtual Player.getScore / setScore, including synchronized data.
    fn score(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn set_score(&mut self, actor: &Self::Actor, score: i32) -> Result<(), Self::Error>;
    fn portal_processor(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Option<Self::PortalProcessor>, Self::Error>;
    fn set_portal_processor(
        &mut self,
        actor: &Self::Actor,
        processor: Option<Self::PortalProcessor>,
    ) -> Result<(), Self::Error>;
    /// Read keepInventory from THIS NEW actor's current level, only on the dead
    /// branch and at this callsite. Do not use an earlier or old-level snapshot.
    fn keep_inventory(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn is_spectator(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;

    fn enchantment_seed(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn set_enchantment_seed(&mut self, actor: &Self::Actor, seed: i32) -> Result<(), Self::Error>;
    fn ender_chest(&mut self, actor: &Self::Actor) -> Result<Self::EnderChest, Self::Error>;
    fn set_ender_chest(
        &mut self,
        actor: &Self::Actor,
        chest: Self::EnderChest,
    ) -> Result<(), Self::Error>;
    fn entity_data(&mut self, actor: &Self::Actor) -> Result<Self::EntityData, Self::Error>;
    fn model_customization(&mut self, data: &Self::EntityData) -> Result<i8, Self::Error>;
    fn set_model_customization(
        &mut self,
        data: &Self::EntityData,
        value: i8,
    ) -> Result<(), Self::Error>;
    fn set_last_sent_exp(&mut self, actor: &Self::Actor, value: i32) -> Result<(), Self::Error>;
    fn set_last_sent_health(&mut self, actor: &Self::Actor, value: f32) -> Result<(), Self::Error>;
    fn set_last_sent_food(&mut self, actor: &Self::Actor, value: i32) -> Result<(), Self::Error>;
    fn recipe_book(&mut self, actor: &Self::Actor) -> Result<Self::RecipeBook, Self::Error>;
    /// Actual new.recipeBook.copyOverData(old.recipeBook), source RecipeBook
    /// 19–26, on the constructor-owned target. Do not share the entire old book.
    fn copy_recipe_book_data(
        &mut self,
        target: &Self::RecipeBook,
        old: &Self::RecipeBook,
    ) -> Result<(), Self::Error>;
    fn seen_credits(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn set_seen_credits(&mut self, actor: &Self::Actor, seen: bool) -> Result<(), Self::Error>;
    fn entered_nether_position(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Option<Self::NetherPosition>, Self::Error>;
    fn set_entered_nether_position(
        &mut self,
        actor: &Self::Actor,
        position: Option<Self::NetherPosition>,
    ) -> Result<(), Self::Error>;
    fn chunk_tracking_view(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Self::ChunkTrackingView, Self::Error>;
    fn set_chunk_tracking_view(
        &mut self,
        actor: &Self::Actor,
        view: Self::ChunkTrackingView,
    ) -> Result<(), Self::Error>;
    fn shoulder_left(&mut self, actor: &Self::Actor)
        -> Result<Self::ShoulderCompound, Self::Error>;
    /// Actual virtual shoulder setters with their actual synchronized data.
    fn set_shoulder_left(
        &mut self,
        actor: &Self::Actor,
        compound: Self::ShoulderCompound,
    ) -> Result<(), Self::Error>;
    fn shoulder_right(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Self::ShoulderCompound, Self::Error>;
    fn set_shoulder_right(
        &mut self,
        actor: &Self::Actor,
        compound: Self::ShoulderCompound,
    ) -> Result<(), Self::Error>;
    fn last_death_location(
        &mut self,
        actor: &Self::Actor,
    ) -> Result<Self::LastDeathLocation, Self::Error>;
    fn set_last_death_location(
        &mut self,
        actor: &Self::Actor,
        location: Self::LastDeathLocation,
    ) -> Result<(), Self::Error>;
}

fn replace_inventory<H: ServerPlayerRestoreHost + ?Sized>(
    new: &H::Actor,
    old: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    let target = host.inventory(new)?;
    let source = host.inventory(old)?;
    host.replace_inventory(&target, &source)
}
fn copy_experience_and_score<H: ServerPlayerRestoreHost + ?Sized>(
    new: &H::Actor,
    old: &H::Actor,
    host: &mut H,
) -> Result<(), H::Error> {
    let level = host.experience_level(old)?;
    host.set_experience_level(new, level)?;
    let total = host.total_experience(old)?;
    host.set_total_experience(new, total)?;
    let progress = host.experience_progress(old)?;
    host.set_experience_progress(new, progress)?;
    let score = host.score(old)?;
    host.set_score(new, score)
}

/// Execute the selected ServerPlayer.restoreFrom body on real actor objects.
/// Source locals retain only the selected receiver/iterator/copied object; old
/// mutable fields are read after earlier callbacks at their original callsites.
pub fn restore_from<H: ServerPlayerRestoreHost + ?Sized>(
    new: &H::Actor,
    old: &H::Actor,
    alive: bool,
    host: &mut H,
) -> Result<(), H::Error> {
    let tracker = host.warden_spawn_tracker(old)?;
    host.set_warden_spawn_tracker(new, tracker)?;
    let chat = host.remote_chat_session(old)?;
    host.set_remote_chat_session(new, chat)?;
    let target_mode = host.game_mode(new)?;
    let old_mode = host.game_mode(old)?;
    let current = host.current_game_type(&old_mode)?;
    let old_mode = host.game_mode(old)?;
    let previous = host.previous_game_type(&old_mode)?;
    host.set_game_mode(&target_mode, current, previous)?;
    host.on_update_abilities(new)?;
    let target_attributes = host.attributes(new)?;
    let old_attributes = host.attributes(old)?;
    host.assign_attribute_base_values(&target_attributes, &old_attributes)?;
    let max_health = host.max_health(new)?;
    host.set_health(new, max_health)?;
    if alive {
        replace_inventory(new, old, host)?;
        let health = host.health(old)?;
        host.set_health(new, health)?;
        let food = host.food_data(old)?;
        host.set_food_data(new, food)?;
        let mut effects = host.active_effects_iterator(old)?;
        while let Some(retained) = host.next_active_effect(&mut effects)? {
            // The actual copy constructor creates a distinct instance with the
            // same holder/details; hidden chain and blend state are NOT copied.
            let copied = host.with_effect_instance(&retained, EffectInstance::copy_instance)?;
            let _ = host.add_effect(new, copied)?;
        }
        copy_experience_and_score(new, old, host)?;
        let portal = host.portal_processor(old)?;
        host.set_portal_processor(new, portal)?;
    } else if host.keep_inventory(new)? || host.is_spectator(old)? {
        replace_inventory(new, old, host)?;
        copy_experience_and_score(new, old, host)?;
    }
    let seed = host.enchantment_seed(old)?;
    host.set_enchantment_seed(new, seed)?;
    let chest = host.ender_chest(old)?;
    host.set_ender_chest(new, chest)?;
    let target_data = host.entity_data(new)?;
    let old_data = host.entity_data(old)?;
    let customization = host.model_customization(&old_data)?;
    host.set_model_customization(&target_data, customization)?;
    host.set_last_sent_exp(new, -1)?;
    host.set_last_sent_health(new, -1.0)?;
    host.set_last_sent_food(new, -1)?;
    let target_book = host.recipe_book(new)?;
    let old_book = host.recipe_book(old)?;
    host.copy_recipe_book_data(&target_book, &old_book)?;
    let seen_credits = host.seen_credits(old)?;
    host.set_seen_credits(new, seen_credits)?;
    let nether = host.entered_nether_position(old)?;
    host.set_entered_nether_position(new, nether)?;
    let view = host.chunk_tracking_view(old)?;
    host.set_chunk_tracking_view(new, view)?;
    let left = host.shoulder_left(old)?;
    host.set_shoulder_left(new, left)?;
    let right = host.shoulder_right(old)?;
    host.set_shoulder_right(new, right)?;
    let death_location = host.last_death_location(old)?;
    host.set_last_death_location(new, death_location)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    // Controlled canonical-object/callback recorder, not a production actor,
    // inventory/map/definition implementation or restore integration result.
    type Lease = Rc<RefCell<EffectInstance<Rc<u32>>>>;
    struct Cursor {
        next: Option<usize>,
        expected_mod_count: u32,
    }
    struct Host {
        calls: Vec<String>,
        fail_at: Option<&'static str>,
        keep: bool,
        spectator: bool,
        old_objects: [Rc<u32>; 8],
        shares: Vec<(&'static str, Rc<u32>)>,
        old_food: Rc<RefCell<i32>>,
        new_food: Rc<RefCell<i32>>,
        old_death: Rc<Option<[i32; 3]>>,
        new_death: Option<Rc<Option<[i32; 3]>>>,
        effects: Vec<Lease>,
        received_copies: Vec<Lease>,
        mod_count: u32,
        mutate_during_add: bool,
        structural_during_add: bool,
        add_result: bool,
        max_health: f32,
        old_health: f32,
        new_health: f32,
        old_level: i32,
        old_total: i32,
        old_progress: f32,
        old_score: i32,
        new_level: i32,
        new_total: i32,
        new_progress: f32,
        new_score: i32,
    }
    impl Default for Host {
        fn default() -> Self {
            let holder = Rc::new(767);
            let hidden = EffectInstance::new(Rc::clone(&holder), 400, 0);
            let mut effect = EffectInstance::with_hidden(
                Rc::clone(&holder),
                100,
                1,
                true,
                false,
                true,
                Some(Box::new(hidden)),
            );
            effect.skip_blending(8);
            Self {
                calls: Vec::new(),
                fail_at: None,
                keep: false,
                spectator: false,
                old_objects: std::array::from_fn(|index| Rc::new(index as u32)),
                shares: Vec::new(),
                old_food: Rc::new(RefCell::new(5)),
                new_food: Rc::new(RefCell::new(9)),
                old_death: Rc::new(Some([1, 2, 3])),
                new_death: None,
                effects: vec![Rc::new(RefCell::new(effect))],
                received_copies: Vec::new(),
                mod_count: 0,
                mutate_during_add: false,
                structural_during_add: false,
                add_result: true,
                max_health: 20.0,
                old_health: 17.0,
                new_health: 20.0,
                old_level: 3,
                old_total: 44,
                old_progress: 0.5,
                old_score: 5,
                new_level: 0,
                new_total: 0,
                new_progress: 0.0,
                new_score: 0,
            }
        }
    }
    impl Host {
        fn call(&mut self, value: impl Into<String>) -> Result<(), &'static str> {
            let value = value.into();
            let fail = self.fail_at == Some(value.as_str());
            self.calls.push(value);
            if fail {
                Err("owner failure")
            } else {
                Ok(())
            }
        }
        fn old_object(
            &mut self,
            actor: &u8,
            index: usize,
            name: &str,
        ) -> Result<Rc<u32>, &'static str> {
            assert_eq!(*actor, 0);
            self.call(name)?;
            Ok(Rc::clone(&self.old_objects[index]))
        }
        fn share(
            &mut self,
            actor: &u8,
            index: usize,
            name: &'static str,
            object: Rc<u32>,
        ) -> Result<(), &'static str> {
            assert_eq!(*actor, 1);
            assert!(Rc::ptr_eq(&object, &self.old_objects[index]));
            self.shares.push((name, object));
            self.call(name)
        }
    }
    macro_rules! object_get_set {
        ($get:ident,$set:ident,$index:expr) => {
            fn $get(&mut self, a: &u8) -> Result<Rc<u32>, Self::Error> {
                self.old_object(a, $index, stringify!($get))
            }
            fn $set(&mut self, a: &u8, v: Rc<u32>) -> Result<(), Self::Error> {
                self.share(a, $index, stringify!($set), v)
            }
        };
    }
    macro_rules! optional_get_set {
        ($get:ident,$set:ident,$index:expr) => {
            fn $get(&mut self, a: &u8) -> Result<Option<Rc<u32>>, Self::Error> {
                Ok(Some(self.old_object(a, $index, stringify!($get))?))
            }
            fn $set(&mut self, a: &u8, v: Option<Rc<u32>>) -> Result<(), Self::Error> {
                self.share(a, $index, stringify!($set), v.expect("same source object"))
            }
        };
    }
    impl ServerPlayerRestoreHost for Host {
        type Error = &'static str;
        type Actor = u8;
        type WardenSpawnTracker = Rc<u32>;
        type RemoteChatSession = Rc<u32>;
        type GameMode = u8;
        type GameType = u8;
        type AttributeMap = u8;
        type Inventory = u8;
        type FoodData = Rc<RefCell<i32>>;
        type EffectHolder = Rc<u32>;
        type EffectCursor = Cursor;
        type EffectLease = Lease;
        type PortalProcessor = Rc<u32>;
        type EnderChest = Rc<u32>;
        type EntityData = u8;
        type RecipeBook = u8;
        type NetherPosition = Rc<u32>;
        type ChunkTrackingView = Rc<u32>;
        type ShoulderCompound = Rc<u32>;
        type LastDeathLocation = Rc<Option<[i32; 3]>>;
        object_get_set!(warden_spawn_tracker, set_warden_spawn_tracker, 0);
        optional_get_set!(remote_chat_session, set_remote_chat_session, 1);
        optional_get_set!(portal_processor, set_portal_processor, 2);
        object_get_set!(ender_chest, set_ender_chest, 3);
        optional_get_set!(entered_nether_position, set_entered_nether_position, 4);
        object_get_set!(chunk_tracking_view, set_chunk_tracking_view, 5);
        object_get_set!(shoulder_left, set_shoulder_left, 6);
        object_get_set!(shoulder_right, set_shoulder_right, 7);
        fn game_mode(&mut self, a: &u8) -> Result<u8, Self::Error> {
            self.call(format!("game_mode:{a}"))?;
            Ok(*a)
        }
        fn current_game_type(&mut self, m: &u8) -> Result<u8, Self::Error> {
            assert_eq!(*m, 0);
            self.call("current_game_type")?;
            Ok(2)
        }
        fn previous_game_type(&mut self, m: &u8) -> Result<Option<u8>, Self::Error> {
            assert_eq!(*m, 0);
            self.call("previous_game_type")?;
            Ok(Some(3))
        }
        fn set_game_mode(&mut self, m: &u8, c: u8, p: Option<u8>) -> Result<(), Self::Error> {
            assert_eq!((*m, c, p), (1, 2, Some(3)));
            self.call("set_game_mode")
        }
        fn on_update_abilities(&mut self, a: &u8) -> Result<(), Self::Error> {
            assert_eq!(*a, 1);
            self.call("abilities")
        }
        fn attributes(&mut self, a: &u8) -> Result<u8, Self::Error> {
            self.call(format!("attributes:{a}"))?;
            Ok(*a)
        }
        fn assign_attribute_base_values(&mut self, t: &u8, o: &u8) -> Result<(), Self::Error> {
            assert_eq!((*t, *o), (1, 0));
            self.max_health = 30.0;
            self.call("assign_base_values")
        }
        fn max_health(&mut self, a: &u8) -> Result<f32, Self::Error> {
            assert_eq!(*a, 1);
            self.call("max_health")?;
            Ok(self.max_health)
        }
        fn health(&mut self, a: &u8) -> Result<f32, Self::Error> {
            assert_eq!(*a, 0);
            self.call("old_health")?;
            Ok(self.old_health)
        }
        fn set_health(&mut self, a: &u8, h: f32) -> Result<(), Self::Error> {
            assert_eq!(*a, 1);
            self.new_health = h;
            self.call(format!("set_health:{h}"))
        }
        fn inventory(&mut self, a: &u8) -> Result<u8, Self::Error> {
            self.call(format!("inventory:{a}"))?;
            Ok(*a)
        }
        fn replace_inventory(&mut self, t: &u8, o: &u8) -> Result<(), Self::Error> {
            assert_eq!((*t, *o), (1, 0));
            self.call("replace_inventory")
        }
        fn food_data(&mut self, a: &u8) -> Result<Rc<RefCell<i32>>, Self::Error> {
            assert_eq!(*a, 0);
            self.call("food")?;
            Ok(Rc::clone(&self.old_food))
        }
        fn set_food_data(&mut self, a: &u8, f: Rc<RefCell<i32>>) -> Result<(), Self::Error> {
            assert_eq!(*a, 1);
            assert!(Rc::ptr_eq(&f, &self.old_food));
            self.new_food = f;
            self.call("set_food")
        }
        fn active_effects_iterator(&mut self, a: &u8) -> Result<Cursor, Self::Error> {
            assert_eq!(*a, 0);
            self.call("effect_iterator")?;
            Ok(Cursor {
                next: (!self.effects.is_empty()).then_some(0),
                expected_mod_count: self.mod_count,
            })
        }
        fn next_active_effect(&mut self, c: &mut Cursor) -> Result<Option<Lease>, Self::Error> {
            self.call("next_effect")?;
            let Some(index) = c.next else { return Ok(None) };
            if c.expected_mod_count != self.mod_count {
                return Err("concurrent modification");
            }
            c.next = (index + 1 < self.effects.len()).then_some(index + 1);
            Ok(Some(Rc::clone(&self.effects[index])))
        }
        fn with_effect_instance<R>(
            &mut self,
            l: &Lease,
            access: impl FnOnce(&EffectInstance<Rc<u32>>) -> R,
        ) -> Result<R, Self::Error> {
            self.call("copy_effect")?;
            Ok(access(&l.borrow()))
        }
        fn add_effect(&mut self, a: &u8, e: EffectInstance<Rc<u32>>) -> Result<bool, Self::Error> {
            assert_eq!(*a, 1);
            for old in &self.effects {
                drop(old.try_borrow_mut().expect("borrow held across addEffect"));
            }
            self.received_copies.push(Rc::new(RefCell::new(e)));
            if self.mutate_during_add {
                self.old_level = 7;
                self.old_total = 70;
                self.old_progress = 0.75;
                self.old_score = 9;
                self.old_health = 2.0;
            }
            if self.structural_during_add {
                self.effects.push(Rc::new(RefCell::new(EffectInstance::new(
                    Rc::new(888),
                    10,
                    0,
                ))));
                self.mod_count += 1;
            }
            self.call("add_effect")?;
            Ok(self.add_result)
        }
        fn experience_level(&mut self, a: &u8) -> Result<i32, Self::Error> {
            assert_eq!(*a, 0);
            self.call("xp_level")?;
            Ok(self.old_level)
        }
        fn set_experience_level(&mut self, a: &u8, n: i32) -> Result<(), Self::Error> {
            assert_eq!(*a, 1);
            self.new_level = n;
            self.call(format!("set_xp_level:{n}"))
        }
        fn total_experience(&mut self, a: &u8) -> Result<i32, Self::Error> {
            assert_eq!(*a, 0);
            self.call("xp_total")?;
            Ok(self.old_total)
        }
        fn set_total_experience(&mut self, a: &u8, n: i32) -> Result<(), Self::Error> {
            assert_eq!(*a, 1);
            self.new_total = n;
            self.call(format!("set_xp_total:{n}"))
        }
        fn experience_progress(&mut self, a: &u8) -> Result<f32, Self::Error> {
            assert_eq!(*a, 0);
            self.call("xp_progress")?;
            Ok(self.old_progress)
        }
        fn set_experience_progress(&mut self, a: &u8, n: f32) -> Result<(), Self::Error> {
            assert_eq!(*a, 1);
            self.new_progress = n;
            self.call(format!("set_xp_progress:{n}"))
        }
        fn score(&mut self, a: &u8) -> Result<i32, Self::Error> {
            assert_eq!(*a, 0);
            self.call("score")?;
            Ok(self.old_score)
        }
        fn set_score(&mut self, a: &u8, n: i32) -> Result<(), Self::Error> {
            assert_eq!(*a, 1);
            self.new_score = n;
            self.call(format!("set_score:{n}"))
        }
        fn keep_inventory(&mut self, a: &u8) -> Result<bool, Self::Error> {
            assert_eq!(*a, 1);
            self.call("keep_new_level")?;
            Ok(self.keep)
        }
        fn is_spectator(&mut self, a: &u8) -> Result<bool, Self::Error> {
            assert_eq!(*a, 0);
            self.call("old_spectator")?;
            Ok(self.spectator)
        }
        fn enchantment_seed(&mut self, a: &u8) -> Result<i32, Self::Error> {
            assert_eq!(*a, 0);
            self.call("seed")?;
            Ok(75)
        }
        fn set_enchantment_seed(&mut self, a: &u8, n: i32) -> Result<(), Self::Error> {
            assert_eq!((*a, n), (1, 75));
            self.call("set_seed")
        }
        fn entity_data(&mut self, a: &u8) -> Result<u8, Self::Error> {
            self.call(format!("data:{a}"))?;
            Ok(*a)
        }
        fn model_customization(&mut self, d: &u8) -> Result<i8, Self::Error> {
            assert_eq!(*d, 0);
            self.call("model")?;
            Ok(-7)
        }
        fn set_model_customization(&mut self, d: &u8, n: i8) -> Result<(), Self::Error> {
            assert_eq!((*d, n), (1, -7));
            self.call("set_model")
        }
        fn set_last_sent_exp(&mut self, a: &u8, n: i32) -> Result<(), Self::Error> {
            assert_eq!((*a, n), (1, -1));
            self.call("sent_exp:-1")
        }
        fn set_last_sent_health(&mut self, a: &u8, n: f32) -> Result<(), Self::Error> {
            assert_eq!((*a, n), (1, -1.0));
            self.call("sent_health:-1")
        }
        fn set_last_sent_food(&mut self, a: &u8, n: i32) -> Result<(), Self::Error> {
            assert_eq!((*a, n), (1, -1));
            self.call("sent_food:-1")
        }
        fn recipe_book(&mut self, a: &u8) -> Result<u8, Self::Error> {
            self.call(format!("book:{a}"))?;
            Ok(*a)
        }
        fn copy_recipe_book_data(&mut self, t: &u8, o: &u8) -> Result<(), Self::Error> {
            assert_eq!((*t, *o), (1, 0));
            self.call("copy_book_data")
        }
        fn seen_credits(&mut self, a: &u8) -> Result<bool, Self::Error> {
            assert_eq!(*a, 0);
            self.call("credits")?;
            Ok(true)
        }
        fn set_seen_credits(&mut self, a: &u8, v: bool) -> Result<(), Self::Error> {
            assert_eq!((*a, v), (1, true));
            self.call("set_credits")
        }
        fn last_death_location(&mut self, a: &u8) -> Result<Self::LastDeathLocation, Self::Error> {
            assert_eq!(*a, 0);
            self.call("death_location")?;
            Ok(Rc::clone(&self.old_death))
        }
        fn set_last_death_location(
            &mut self,
            a: &u8,
            d: Self::LastDeathLocation,
        ) -> Result<(), Self::Error> {
            assert_eq!(*a, 1);
            assert!(Rc::ptr_eq(&d, &self.old_death));
            self.new_death = Some(d);
            self.call("set_death_location")
        }
    }
    #[test]
    fn alive_shares_real_objects_copies_effect_details_and_rereads_xp_after_callbacks() {
        let mut host = Host {
            mutate_during_add: true,
            add_result: false,
            ..Host::default()
        };
        restore_from(&1, &0, true, &mut host).unwrap();
        assert_eq!(
            &host.calls[..16],
            [
                "warden_spawn_tracker",
                "set_warden_spawn_tracker",
                "remote_chat_session",
                "set_remote_chat_session",
                "game_mode:1",
                "game_mode:0",
                "current_game_type",
                "game_mode:0",
                "previous_game_type",
                "set_game_mode",
                "abilities",
                "attributes:1",
                "attributes:0",
                "assign_base_values",
                "max_health",
                "set_health:30"
            ]
        );
        assert!(!host
            .calls
            .iter()
            .any(|call| call == "keep_new_level" || call == "old_spectator"));
        assert_eq!(host.new_health, 17.0);
        assert_eq!(host.old_health, 2.0);
        assert_eq!(
            (
                host.new_level,
                host.new_total,
                host.new_progress,
                host.new_score
            ),
            (7, 70, 0.75, 9)
        );
        assert!(Rc::ptr_eq(&host.new_food, &host.old_food));
        *host.old_food.borrow_mut() = 33;
        assert_eq!(*host.new_food.borrow(), 33);
        let copy = &host.received_copies[0];
        assert!(!Rc::ptr_eq(copy, &host.effects[0]));
        let original = host.effects[0].borrow();
        let mut copied = copy.borrow_mut();
        assert!(Rc::ptr_eq(original.effect(), copied.effect()));
        assert!(original.hidden_effect().is_some());
        assert!(copied.hidden_effect().is_none());
        assert_eq!(
            (
                copied.duration(),
                copied.amplifier(),
                copied.is_ambient(),
                copied.is_visible(),
                copied.show_icon()
            ),
            (100, 1, true, false, true)
        );
        assert_eq!(copied.blend_factor(false, 1.0), 0.0);
        drop(original);
        drop(copied);
        let effect = host.calls.iter().position(|c| c == "add_effect").unwrap();
        let xp = host.calls.iter().position(|c| c == "xp_level").unwrap();
        assert!(effect < xp);
        assert_eq!(host.shares.len(), 8);
        assert!(host.new_death.is_some());
        let sentinel = host.calls.iter().position(|c| c == "sent_exp:-1").unwrap();
        assert_eq!(
            &host.calls[sentinel..sentinel + 6],
            [
                "sent_exp:-1",
                "sent_health:-1",
                "sent_food:-1",
                "book:1",
                "book:0",
                "copy_book_data"
            ]
        );
    }
    #[test]
    fn dead_branches_keep_only_the_source_subset_and_use_new_level_rule() {
        let mut discarded = Host::default();
        let food = Rc::clone(&discarded.new_food);
        restore_from(&1, &0, false, &mut discarded).unwrap();
        assert_eq!(discarded.new_health, 30.0);
        assert!(Rc::ptr_eq(&discarded.new_food, &food));
        assert_eq!(discarded.new_level, 0);
        assert!(discarded.received_copies.is_empty());
        assert!(!discarded
            .calls
            .iter()
            .any(|call| call == "inventory:1" || call == "food" || call == "portal_processor"));
        assert!(discarded
            .calls
            .windows(2)
            .any(|calls| calls == ["keep_new_level", "old_spectator"]));
        for (keep, spectator) in [(true, false), (false, true)] {
            let mut host = Host {
                keep,
                spectator,
                ..Host::default()
            };
            let food = Rc::clone(&host.new_food);
            restore_from(&1, &0, false, &mut host).unwrap();
            assert_eq!(host.new_health, 30.0);
            assert!(Rc::ptr_eq(&host.new_food, &food));
            assert_eq!(
                (
                    host.new_level,
                    host.new_total,
                    host.new_progress,
                    host.new_score
                ),
                (3, 44, 0.5, 5)
            );
            assert!(!host.calls.iter().any(|call| call == "old_health"
                || call == "food"
                || call == "effect_iterator"
                || call == "portal_processor"));
            assert_eq!(host.calls.iter().any(|call| call == "old_spectator"), !keep);
            assert_eq!(host.shares.len(), 7);
        }
    }
    #[test]
    fn effect_failure_and_live_iterator_failure_preserve_the_completed_prefix() {
        let mut failed = Host {
            fail_at: Some("add_effect"),
            ..Host::default()
        };
        assert_eq!(
            restore_from(&1, &0, true, &mut failed),
            Err("owner failure")
        );
        assert_eq!(failed.new_health, 17.0);
        assert!(Rc::ptr_eq(&failed.new_food, &failed.old_food));
        assert_eq!(failed.received_copies.len(), 1);
        assert_eq!(failed.new_level, 0);
        assert_eq!(failed.calls.last().unwrap(), "add_effect");
        assert!(!failed.calls.iter().any(|c| c == "seed"));
        let mut live = Host {
            structural_during_add: true,
            ..Host::default()
        };
        live.effects.push(Rc::new(RefCell::new(EffectInstance::new(
            Rc::new(999),
            50,
            0,
        ))));
        assert_eq!(
            restore_from(&1, &0, true, &mut live),
            Err("concurrent modification")
        );
        assert_eq!(live.received_copies.len(), 1);
        assert_eq!(live.calls.last().unwrap(), "next_effect");
        assert!(!live
            .calls
            .iter()
            .any(|call| call == "xp_level" || call == "seed"));
        let mut sentinel = Host {
            fail_at: Some("sent_health:-1"),
            ..Host::default()
        };
        assert_eq!(
            restore_from(&1, &0, false, &mut sentinel),
            Err("owner failure")
        );
        assert!(sentinel.calls.iter().any(|c| c == "sent_exp:-1"));
        assert_eq!(sentinel.calls.last().unwrap(), "sent_health:-1");
        assert!(!sentinel
            .calls
            .iter()
            .any(|c| c == "sent_food:-1" || c == "book:1"));
    }
}
