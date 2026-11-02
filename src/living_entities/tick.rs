//! Original Java 1.21 LivingEntity.tick (`btn.l`, 2468–2595).
//!
//! The source parent Entity.tick487–488 is the literal virtual baseTick call
//! below. This executor creates no actor, field state, health, RNG or scheduler.
//! Actual item, equipment, combat, AI, attribute and world operations remain
//! mandatory owners, with their normal virtual dispatch and mutation prefixes.

use crate::mobs::bat::{mth_atan2, wrap_degrees};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickIntField {
    RemoveArrowTime,
    RemoveStingerTime,
    TickCount,
    FallFlyTicks,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickFloatField {
    BodyYaw,
    Run,
    OldRun,
    AttackAnim,
    OldYaw,
    OldBodyYaw,
    OldPitch,
    HeadYaw,
    OldHeadYaw,
    AnimationStep,
    AppliedScale,
    SwimAmount,
    OldSwimAmount,
}

/// A short projection of EXISTING canonical actor fields, never a copied state
/// object. Its methods perform only the named raw read/write, with no actor,
/// world, RNG, virtual callback or hidden synchronization. A runtime may project
/// fields from its existing physical/body owners without allocating duplicates.
pub trait LivingTickFields {
    fn int(&self, field: TickIntField) -> i32;
    fn set_int(&mut self, field: TickIntField, value: i32);
    fn float(&self, field: TickFloatField) -> f32;
    fn set_float(&mut self, field: TickFloatField, value: f32);
    /// Entity.xo/zo (`bsr.L/N`), distinct from Entity.xOld/zOld (`ad/af`).
    fn xo(&self) -> f64;
    fn zo(&self) -> f64;
}

/// Access to one retained actual LivingEntity lifetime and its real owners.
///
/// Actor retains the SAME object through removal, reentry and numeric-ID reuse.
/// with_tick_fields must access that canonical object in place and release ALL
/// field/actor borrows before returning. It must not move state out, use a
/// snapshot, or resolve an old lease through the current ID registry. Lookup
/// errors precede the closure; executed field fragments remain committed.
/// Other source handles retain the exact returned receiver during callbacks.
/// Every operation is mandatory; an unavailable owner must return an error.
/// Errors stop the method without rollback, retries or inserted profiler pops.
pub trait LivingTickHost {
    type Error;
    type Actor;
    type Level;
    type Profiler;
    type CombatTracker;

    fn with_tick_fields<R>(
        &mut self,
        actor: &Self::Actor,
        access: impl FnOnce(&mut dyn LivingTickFields) -> R,
    ) -> Result<R, Self::Error>;

    /// Actual virtual baseTick from the inspected Entity.tick parent body.
    /// For an ordinary LivingEntity this selects living_entities::base_tick;
    /// inherited Entity.baseTick is then called at its OWN source position.
    /// Do not substitute Entity.baseTick here or advance the level tick counter.
    fn base_tick(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    /// Actual private updatingUsingItem3137–3146: current hand/useItem, item-only
    /// ItemStack.isSameItem, refresh the SAME current stack reference, then real
    /// updateUsingItem or stopUsingItem with effects/consumption/finish callbacks.
    /// An empty/equal presentation stack is not permission to skip that method.
    fn updating_using_item(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn is_visually_swimming(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn level(&mut self, actor: &Self::Actor) -> Result<Self::Level, Self::Error>;
    fn level_is_client_side(&mut self, level: &Self::Level) -> Result<bool, Self::Error>;
    fn arrow_count(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn set_arrow_count(&mut self, actor: &Self::Actor, count: i32) -> Result<(), Self::Error>;
    fn stinger_count(&mut self, actor: &Self::Actor) -> Result<i32, Self::Error>;
    fn set_stinger_count(&mut self, actor: &Self::Actor, count: i32) -> Result<(), Self::Error>;
    /// Actual private detectEquipmentUpdates2598–2607, with original stacks,
    /// collect/remove/apply attribute and enchantment operations, hand-swap
    /// event55, last-stack copies and equipment packets in their source order.
    fn detect_equipment_updates(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn combat_tracker(&mut self, actor: &Self::Actor) -> Result<Self::CombatTracker, Self::Error>;
    fn combat_tracker_recheck_status(&mut self, tracker: &Self::CombatTracker) -> Result<(), Self::Error>;
    fn is_sleeping(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual private checkBedExists3484, including the current sleeping
    /// Optional and its real current-Level BedBlock query, not a cached flag.
    fn check_bed_exists(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn stop_sleeping(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    fn is_removed(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    /// Actual virtual aiStep. Mob/Player overrides and their parent calls remain
    /// authoritative; do not unconditionally select the generic body or replace
    /// it with controls/travel scheduled outside the original callgraph.
    fn ai_step(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    /// Actual final coordinate accessors, with no virtual game callbacks.
    fn x(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    fn z(&mut self, actor: &Self::Actor) -> Result<f64, Self::Error>;
    /// Actual Entity.getYRot/getXRot virtual methods, reread at source sites.
    fn yaw(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn pitch(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    fn on_ground(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn profiler(&mut self, level: &Self::Level) -> Result<Self::Profiler, Self::Error>;
    fn profiler_push(&mut self, profiler: &Self::Profiler, section: &'static str) -> Result<(), Self::Error>;
    fn profiler_pop(&mut self, profiler: &Self::Profiler) -> Result<(), Self::Error>;
    /// Actual virtual tickHeadTurn. Mob's override calls its real retained
    /// BodyRotationControl.clientTick and returns the input step amount; it must
    /// not silently execute the different generic LivingEntity head-turn math.
    fn tick_head_turn(&mut self, actor: &Self::Actor, desired_body_yaw: f32, step: f32) -> Result<f32, Self::Error>;
    fn is_fall_flying(&mut self, actor: &Self::Actor) -> Result<bool, Self::Error>;
    fn set_pitch(&mut self, actor: &Self::Actor, pitch: f32) -> Result<(), Self::Error>;
    /// Actual private refreshDirtyAttributes1091–1096: retain the real update
    /// set, iterate original instances/holders, run onAttributeUpdated, then
    /// clear that SAME set only after successful iteration. Health/absorption
    /// clamps and their setters are real callbacks, not dirty-flag-only work.
    fn refresh_dirty_attributes(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
    /// Actual virtual getScale (current attribute map and sanitizeScale).
    fn scale(&mut self, actor: &Self::Actor) -> Result<f32, Self::Error>;
    /// Actual virtual Entity.refreshDimensions and its physical/position/eye/
    /// collision callbacks. appliedScale was already stored when this starts.
    fn refresh_dimensions(&mut self, actor: &Self::Actor) -> Result<(), Self::Error>;
}

fn java_min(a: f32, b: f32) -> f32 {
    if a.is_nan() { a } else if b.is_nan() { b } else if a == 0.0 && b == 0.0 {
        f32::from_bits(a.to_bits() | b.to_bits())
    } else if a <= b { a } else { b }
}
fn java_max(a: f32, b: f32) -> f32 {
    if a.is_nan() { a } else if b.is_nan() { b } else if a == 0.0 && b == 0.0 {
        f32::from_bits(a.to_bits() & b.to_bits())
    } else if a >= b { a } else { b }
}

/// Original private updateSwimAmount3166–3172 over the actual swim fields.
pub fn update_swim_amount<H: LivingTickHost>(actor: &H::Actor, host: &mut H) -> Result<(), H::Error> {
    host.with_tick_fields(actor, |fields| {
        let amount = fields.float(TickFloatField::SwimAmount);
        fields.set_float(TickFloatField::OldSwimAmount, amount);
    })?;
    let swimming = host.is_visually_swimming(actor)?;
    host.with_tick_fields(actor, |fields| {
        let amount = fields.float(TickFloatField::SwimAmount);
        fields.set_float(TickFloatField::SwimAmount, if swimming {
            java_min(1.0, amount + 0.09_f32)
        } else {
            java_max(0.0, amount - 0.09_f32)
        });
    })
}

fn projectile_count_expired<H: LivingTickHost>(actor: &H::Actor, field: TickIntField, count: i32, host: &mut H) -> Result<bool, H::Error> {
    host.with_tick_fields(actor, |fields| {
        if fields.int(field) <= 0 {
            fields.set_int(field, 20_i32.wrapping_mul(30_i32.wrapping_sub(count)));
        }
        let value = fields.int(field).wrapping_sub(1);
        fields.set_int(field, value);
        fields.int(field) <= 0
    })
}

#[derive(Clone, Copy)]
enum Rotation { Yaw, BodyYaw, Pitch, HeadYaw }
fn rotation_difference<H: LivingTickHost>(actor: &H::Actor, rotation: Rotation, previous: TickFloatField, host: &mut H) -> Result<f32, H::Error> {
    let current = match rotation {
        Rotation::Yaw => host.yaw(actor)?,
        Rotation::Pitch => host.pitch(actor)?,
        Rotation::BodyYaw => host.with_tick_fields(actor, |f| f.float(TickFloatField::BodyYaw))?,
        Rotation::HeadYaw => host.with_tick_fields(actor, |f| f.float(TickFloatField::HeadYaw))?,
    };
    let old = host.with_tick_fields(actor, |f| f.float(previous))?;
    Ok(current - old)
}

/// Execute the complete inspected LivingEntity.tick once. Source removed/alive
/// admission belongs to the actual caller; only the original aiStep call is
/// gated by isRemoved here. Work after AI/removal continues on the retained actor.
pub fn tick<H: LivingTickHost>(actor: &H::Actor, host: &mut H) -> Result<(), H::Error> {
    // Literal inspected Entity.tick487–488: this.baseTick(), with virtual dispatch.
    host.base_tick(actor)?;
    host.updating_using_item(actor)?;
    update_swim_amount(actor, host)?;

    let level = host.level(actor)?;
    if !host.level_is_client_side(&level)? {
        let arrows = host.arrow_count(actor)?;
        if arrows > 0 && projectile_count_expired(actor, TickIntField::RemoveArrowTime, arrows, host)? {
            host.set_arrow_count(actor, arrows.wrapping_sub(1))?;
        }
        let stingers = host.stinger_count(actor)?;
        if stingers > 0 && projectile_count_expired(actor, TickIntField::RemoveStingerTime, stingers, host)? {
            host.set_stinger_count(actor, stingers.wrapping_sub(1))?;
        }
        host.detect_equipment_updates(actor)?;
        if host.with_tick_fields(actor, |f| f.int(TickIntField::TickCount))? % 20 == 0 {
            let tracker = host.combat_tracker(actor)?;
            host.combat_tracker_recheck_status(&tracker)?;
        }
        if host.is_sleeping(actor)? && !host.check_bed_exists(actor)? {
            host.stop_sleeping(actor)?;
        }
    }
    if !host.is_removed(actor)? {
        host.ai_step(actor)?;
    }

    let x = host.x(actor)?;
    let dx = x - host.with_tick_fields(actor, |f| f.xo())?;
    let z = host.z(actor)?;
    let dz = z - host.with_tick_fields(actor, |f| f.zo())?;
    let distance = (dx * dx + dz * dz) as f32;
    let mut desired_body_yaw = host.with_tick_fields(actor, |f| f.float(TickFloatField::BodyYaw))?;
    let mut step = 0.0_f32;
    host.with_tick_fields(actor, |f| {
        let run = f.float(TickFloatField::Run);
        f.set_float(TickFloatField::OldRun, run);
    })?;
    let mut run_target = 0.0_f32;
    if distance > 0.0025000002_f32 {
        run_target = 1.0;
        step = (f64::from(distance).sqrt() as f32) * 3.0_f32;
        // This callsite casts BEFORE the multiply; other source callers differ.
        let movement_yaw = (mth_atan2(dz, dx) as f32) * 57.295776_f32 - 90.0_f32;
        let absolute_difference = (wrap_degrees(host.yaw(actor)?) - movement_yaw).abs();
        desired_body_yaw = if 95.0 < absolute_difference && absolute_difference < 265.0 {
            movement_yaw - 180.0_f32
        } else {
            movement_yaw
        };
    }
    if host.with_tick_fields(actor, |f| f.float(TickFloatField::AttackAnim))? > 0.0 {
        desired_body_yaw = host.yaw(actor)?;
    }
    if !host.on_ground(actor)? {
        run_target = 0.0;
    }
    host.with_tick_fields(actor, |f| {
        let run = f.float(TickFloatField::Run);
        let difference = run_target - f.float(TickFloatField::Run);
        f.set_float(TickFloatField::Run, run + difference * 0.3_f32);
    })?;

    let level = host.level(actor)?;
    let profiler = host.profiler(&level)?;
    host.profiler_push(&profiler, "headTurn")?;
    step = host.tick_head_turn(actor, desired_body_yaw, step)?;
    let level = host.level(actor)?;
    let profiler = host.profiler(&level)?;
    host.profiler_pop(&profiler)?;
    let level = host.level(actor)?;
    let profiler = host.profiler(&level)?;
    host.profiler_push(&profiler, "rangeChecks")?;

    for (current, previous) in [
        (Rotation::Yaw, TickFloatField::OldYaw),
        (Rotation::BodyYaw, TickFloatField::OldBodyYaw),
        (Rotation::Pitch, TickFloatField::OldPitch),
        (Rotation::HeadYaw, TickFloatField::OldHeadYaw),
    ] {
        // Preserve the TWO source loops per rotation, including fresh getters
        // per condition. No modulo normalization, finite guard or iteration cap.
        while rotation_difference(actor, current, previous, host)? < -180.0 {
            host.with_tick_fields(actor, |f| {
                let old = f.float(previous);
                f.set_float(previous, old - 360.0_f32);
            })?;
        }
        while rotation_difference(actor, current, previous, host)? >= 180.0 {
            host.with_tick_fields(actor, |f| {
                let old = f.float(previous);
                f.set_float(previous, old + 360.0_f32);
            })?;
        }
    }
    let level = host.level(actor)?;
    let profiler = host.profiler(&level)?;
    host.profiler_pop(&profiler)?;
    host.with_tick_fields(actor, |f| {
        let value = f.float(TickFloatField::AnimationStep);
        f.set_float(TickFloatField::AnimationStep, value + step);
    })?;

    let flying = host.is_fall_flying(actor)?;
    host.with_tick_fields(actor, |f| {
        let ticks = if flying { f.int(TickIntField::FallFlyTicks).wrapping_add(1) } else { 0 };
        f.set_int(TickIntField::FallFlyTicks, ticks);
    })?;
    if host.is_sleeping(actor)? {
        host.set_pitch(actor, 0.0)?;
    }
    host.refresh_dirty_attributes(actor)?;
    let scale = host.scale(actor)?;
    if scale != host.with_tick_fields(actor, |f| f.float(TickFloatField::AppliedScale))? {
        host.with_tick_fields(actor, |f| f.set_float(TickFloatField::AppliedScale, scale))?;
        host.refresh_dimensions(actor)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    type Actor = Rc<RefCell<State>>;
    struct State {
        ints: [i32; 4],
        floats: [f32; 13],
        position: [f64; 2],
        old: [f64; 2],
        level: u32,
        arrows: i32,
        stingers: i32,
        removed: bool,
        yaw: f32,
        pitch: f32,
    }
    impl State {
        fn actor() -> Actor {
            let mut state = Self {
                ints: [0, 0, 20, 0], floats: [0.0; 13],
                position: [0.0; 2], old: [0.0; 2], level: 1,
                arrows: 0, stingers: 0, removed: false, yaw: 0.0, pitch: 0.0,
            };
            state.floats[TickFloatField::AppliedScale as usize] = 1.0;
            Rc::new(RefCell::new(state))
        }
    }
    impl LivingTickFields for State {
        fn int(&self, field: TickIntField) -> i32 { self.ints[field as usize] }
        fn set_int(&mut self, field: TickIntField, value: i32) { self.ints[field as usize] = value; }
        fn float(&self, field: TickFloatField) -> f32 { self.floats[field as usize] }
        fn set_float(&mut self, field: TickFloatField, value: f32) { self.floats[field as usize] = value; }
        fn xo(&self) -> f64 { self.old[0] }
        fn zo(&self) -> f64 { self.old[1] }
    }
    type Hook = (&'static str, fn(&Actor, &mut Host) -> Result<(), &'static str>);
    struct Host {
        retained: Actor,
        current_registry_actor: Actor,
        calls: Vec<String>,
        head_arguments: Vec<(f32, f32)>,
        hook: Option<Hook>,
        fail: Option<&'static str>,
        swimming: bool,
        grounded: bool,
        flying: bool,
        sleeping: VecDeque<bool>,
        bed_exists: bool,
        returned_step: Option<f32>,
        scale: f32,
        yaw_values: VecDeque<f32>,
        yaw_calls: usize,
        fail_yaw_call: Option<usize>,
    }
    impl Host {
        fn new(actor: &Actor) -> Self {
            Self {
                retained: actor.clone(), current_registry_actor: actor.clone(), calls: Vec::new(),
                head_arguments: Vec::new(), hook: None, fail: None, swimming: false,
                grounded: true, flying: false, sleeping: VecDeque::new(), bed_exists: true,
                returned_step: None, scale: 1.0, yaw_values: VecDeque::new(),
                yaw_calls: 0, fail_yaw_call: None,
            }
        }
        fn check(&self, actor: &Actor) { assert!(Rc::ptr_eq(actor, &self.retained)); }
        fn call(&mut self, actor: &Actor, name: &'static str) -> Result<(), &'static str> {
            self.check(actor);
            self.calls.push(name.into());
            if self.hook.as_ref().is_some_and(|hook| hook.0 == name) {
                let (_, hook) = self.hook.take().unwrap();
                hook(actor, self)?;
            }
            if self.fail == Some(name) { Err(name) } else { Ok(()) }
        }
        fn index(&self, name: &str) -> usize { self.calls.iter().position(|call| call == name).unwrap() }
    }
    impl LivingTickHost for Host {
        type Error = &'static str;
        type Actor = Actor;
        type Level = u32;
        type Profiler = u32;
        type CombatTracker = Actor;

        fn with_tick_fields<R>(&mut self, actor: &Actor, access: impl FnOnce(&mut dyn LivingTickFields) -> R) -> Result<R, Self::Error> {
            self.check(actor); Ok(access(&mut *actor.borrow_mut()))
        }
        fn base_tick(&mut self, a: &Actor) -> Result<(), Self::Error> { self.call(a, "virtual-base") }
        fn updating_using_item(&mut self, a: &Actor) -> Result<(), Self::Error> { self.call(a, "item-use") }
        fn is_visually_swimming(&mut self, a: &Actor) -> Result<bool, Self::Error> { self.call(a, "swimming")?; Ok(self.swimming) }
        fn level(&mut self, a: &Actor) -> Result<u32, Self::Error> { self.check(a); let level = a.borrow().level; self.calls.push(format!("level:{level}")); Ok(level) }
        fn level_is_client_side(&mut self, l: &u32) -> Result<bool, Self::Error> { self.calls.push(format!("client:{l}")); Ok(*l == 0) }
        fn arrow_count(&mut self, a: &Actor) -> Result<i32, Self::Error> { self.call(a, "arrows")?; Ok(a.borrow().arrows) }
        fn set_arrow_count(&mut self, a: &Actor, n: i32) -> Result<(), Self::Error> { a.borrow_mut().arrows = n; self.call(a, "set-arrows") }
        fn stinger_count(&mut self, a: &Actor) -> Result<i32, Self::Error> { self.call(a, "stingers")?; Ok(a.borrow().stingers) }
        fn set_stinger_count(&mut self, a: &Actor, n: i32) -> Result<(), Self::Error> { a.borrow_mut().stingers = n; self.call(a, "set-stingers") }
        fn detect_equipment_updates(&mut self, a: &Actor) -> Result<(), Self::Error> { self.call(a, "equipment") }
        fn combat_tracker(&mut self, a: &Actor) -> Result<Actor, Self::Error> { self.call(a, "tracker")?; Ok(a.clone()) }
        fn combat_tracker_recheck_status(&mut self, tracker: &Actor) -> Result<(), Self::Error> { self.call(tracker, "recheck") }
        fn is_sleeping(&mut self, a: &Actor) -> Result<bool, Self::Error> { self.call(a, "sleeping")?; Ok(self.sleeping.pop_front().unwrap_or(false)) }
        fn check_bed_exists(&mut self, a: &Actor) -> Result<bool, Self::Error> { self.call(a, "bed")?; Ok(self.bed_exists) }
        fn stop_sleeping(&mut self, a: &Actor) -> Result<(), Self::Error> { self.call(a, "stop-sleeping") }
        fn is_removed(&mut self, a: &Actor) -> Result<bool, Self::Error> { self.call(a, "removed")?; Ok(a.borrow().removed) }
        fn ai_step(&mut self, a: &Actor) -> Result<(), Self::Error> { self.call(a, "ai") }
        fn x(&mut self, a: &Actor) -> Result<f64, Self::Error> { self.check(a); self.calls.push("x".into()); Ok(a.borrow().position[0]) }
        fn z(&mut self, a: &Actor) -> Result<f64, Self::Error> { self.check(a); self.calls.push("z".into()); Ok(a.borrow().position[1]) }
        fn yaw(&mut self, a: &Actor) -> Result<f32, Self::Error> {
            self.call(a, "yaw")?; self.yaw_calls += 1;
            if self.fail_yaw_call == Some(self.yaw_calls) { return Err("yaw-read"); }
            Ok(self.yaw_values.pop_front().unwrap_or_else(|| a.borrow().yaw))
        }
        fn pitch(&mut self, a: &Actor) -> Result<f32, Self::Error> { self.call(a, "pitch")?; Ok(a.borrow().pitch) }
        fn on_ground(&mut self, a: &Actor) -> Result<bool, Self::Error> { self.call(a, "ground")?; Ok(self.grounded) }
        fn profiler(&mut self, l: &u32) -> Result<u32, Self::Error> { self.calls.push(format!("profiler:{l}")); Ok(*l) }
        fn profiler_push(&mut self, p: &u32, section: &'static str) -> Result<(), Self::Error> { self.calls.push(format!("push:{p}:{section}")); Ok(()) }
        fn profiler_pop(&mut self, p: &u32) -> Result<(), Self::Error> { self.calls.push(format!("pop:{p}")); Ok(()) }
        fn tick_head_turn(&mut self, a: &Actor, yaw: f32, step: f32) -> Result<f32, Self::Error> { self.head_arguments.push((yaw, step)); self.call(a, "head")?; Ok(self.returned_step.unwrap_or(step)) }
        fn is_fall_flying(&mut self, a: &Actor) -> Result<bool, Self::Error> { self.call(a, "flying")?; Ok(self.flying) }
        fn set_pitch(&mut self, a: &Actor, value: f32) -> Result<(), Self::Error> { a.borrow_mut().pitch = value; self.call(a, "set-pitch") }
        fn refresh_dirty_attributes(&mut self, a: &Actor) -> Result<(), Self::Error> { self.call(a, "attributes") }
        fn scale(&mut self, a: &Actor) -> Result<f32, Self::Error> { self.call(a, "scale")?; Ok(self.scale) }
        fn refresh_dimensions(&mut self, a: &Actor) -> Result<(), Self::Error> { self.call(a, "dimensions") }
    }

    #[test]
    fn original_prefix_and_callback_free_swim_fragment_preserve_fresh_read() {
        let actor = State::actor(); actor.borrow_mut().set_float(TickFloatField::SwimAmount, 0.2);
        let mut host = Host::new(&actor);
        host.hook = Some(("swimming", |actor, _| { actor.borrow_mut().set_float(TickFloatField::SwimAmount, 0.8); Ok(()) }));
        tick(&actor, &mut host).unwrap();
        assert_eq!(&host.calls[..3], ["virtual-base", "item-use", "swimming"]);
        let state = actor.borrow();
        assert_eq!(state.float(TickFloatField::OldSwimAmount), 0.2);
        assert_eq!(state.float(TickFloatField::SwimAmount).to_bits(), (0.8_f32 - 0.09_f32).to_bits());
    }

    #[test]
    fn server_gate_is_retained_counts_wrap_and_setter_changes_next_count() {
        let actor = State::actor();
        { let mut state = actor.borrow_mut(); state.arrows = 31; state.set_int(TickIntField::TickCount, -20); }
        let mut host = Host::new(&actor);
        host.hook = Some(("set-arrows", |actor, _| {
            let mut state = actor.borrow_mut(); state.stingers = 31; state.level = 0; Ok(())
        }));
        tick(&actor, &mut host).unwrap();
        let state = actor.borrow();
        assert_eq!(state.int(TickIntField::RemoveArrowTime), -21);
        assert_eq!(state.arrows, 30);
        assert_eq!((state.stingers, state.int(TickIntField::RemoveStingerTime)), (30, -21));
        assert!(host.index("set-arrows") < host.index("stingers"));
        assert!(host.index("set-stingers") < host.index("equipment"));
        assert!(host.index("equipment") < host.index("recheck"));
        assert_eq!(host.calls.iter().filter(|call| call.starts_with("client:")).count(), 1);
        drop(state);
        { let mut state = actor.borrow_mut(); state.level = 1; state.arrows = i32::MAX; state.stingers = 0; state.set_int(TickIntField::RemoveArrowTime, 0); }
        tick(&actor, &mut host).unwrap();
        assert_eq!(actor.borrow().int(TickIntField::RemoveArrowTime), 20_i32.wrapping_mul(30_i32.wrapping_sub(i32::MAX)).wrapping_sub(1));
        assert_eq!(actor.borrow().arrows, i32::MAX);
    }

    #[test]
    fn removed_admission_only_skips_ai_and_ai_replacement_keeps_same_actor() {
        let actor = State::actor(); actor.borrow_mut().removed = true;
        let mut host = Host::new(&actor);
        tick(&actor, &mut host).unwrap();
        assert!(!host.calls.iter().any(|call| call == "ai"));
        assert!(host.index("item-use") < host.index("equipment"));
        assert!(host.index("removed") < host.index("head"));
        assert!(host.index("head") < host.index("attributes"));
        actor.borrow_mut().removed = false; host.calls.clear();
        host.hook = Some(("ai", |actor, host| {
            actor.borrow_mut().removed = true;
            actor.borrow_mut().position = [1.0, 0.0];
            host.current_registry_actor = State::actor();
            Ok(())
        }));
        tick(&actor, &mut host).unwrap();
        assert!(!Rc::ptr_eq(&actor, &host.current_registry_actor));
        assert_eq!(host.head_arguments.last().unwrap().1, 3.0);
        assert_eq!(actor.borrow().float(TickFloatField::AnimationStep), 3.0);
        assert_eq!(host.current_registry_actor.borrow().float(TickFloatField::AnimationStep), 0.0);
    }

    #[test]
    fn nested_same_actor_tick_releases_fields_and_outer_locals_survive() {
        let actor = State::actor(); actor.borrow_mut().position = [1.0, 0.0];
        let mut host = Host::new(&actor);
        host.hook = Some(("head", |actor, host| {
            { let mut state = actor.borrow_mut(); state.position = [2.0, 0.0]; state.level = 3; }
            tick(actor, host)?;
            Ok(())
        }));
        tick(&actor, &mut host).unwrap();
        assert_eq!(host.head_arguments, [(-90.0, 3.0), (-90.0, 6.0)]);
        assert_eq!(actor.borrow().float(TickFloatField::AnimationStep), 9.0);
        assert!(host.index("push:1:headTurn") < host.index("push:3:headTurn"));
        assert!(!host.calls.iter().any(|call| call == "pop:1"));
        assert_eq!(host.calls.iter().filter(|call| *call == "pop:3").count(), 4);
    }

    #[test]
    fn separate_range_loops_reread_virtual_yaw_and_keep_failed_prefix() {
        let actor = State::actor();
        let mut host = Host::new(&actor);
        host.yaw_values = [-540.0, -100.0, 540.0, 0.0].into();
        tick(&actor, &mut host).unwrap();
        assert_eq!(host.yaw_calls, 4);
        assert_eq!(actor.borrow().float(TickFloatField::OldYaw), 0.0);
        host.calls.clear(); host.yaw_calls = 0;
        host.yaw_values = [-540.0, -100.0, 540.0].into();
        host.fail_yaw_call = Some(3);
        assert_eq!(tick(&actor, &mut host), Err("yaw-read"));
        assert_eq!(actor.borrow().float(TickFloatField::OldYaw), -360.0);
        assert_eq!(host.calls.last().unwrap(), "yaw");
        assert!(!host.calls.iter().any(|call| call == "attributes"));
    }

    #[test]
    fn late_fields_nan_wrapping_sleep_and_dimension_failure_are_committed() {
        let actor = State::actor();
        {
            let mut state = actor.borrow_mut(); state.yaw = f32::NAN; state.pitch = f32::NAN;
            for field in [TickFloatField::BodyYaw, TickFloatField::HeadYaw, TickFloatField::Run, TickFloatField::SwimAmount] { state.set_float(field, f32::NAN); }
            state.set_int(TickIntField::FallFlyTicks, i32::MAX);
        }
        let mut host = Host::new(&actor); host.flying = true; host.swimming = true;
        host.sleeping = [false, true].into(); host.scale = f32::NAN;
        host.fail = Some("dimensions");
        assert_eq!(tick(&actor, &mut host), Err("dimensions"));
        let state = actor.borrow();
        assert_eq!(state.int(TickIntField::FallFlyTicks), i32::MIN);
        assert_eq!(state.pitch, 0.0);
        assert!(state.float(TickFloatField::Run).is_nan());
        assert!(state.float(TickFloatField::SwimAmount).is_nan());
        assert!(state.float(TickFloatField::AppliedScale).is_nan());
        assert!(host.index("set-pitch") < host.index("attributes"));
        assert!(host.index("attributes") < host.index("scale"));
        assert_eq!(host.calls.last().unwrap(), "dimensions");
    }

    #[test]
    fn threshold_attack_override_grounded_target_and_head_failure_prefix() {
        let actor = State::actor();
        { let mut state = actor.borrow_mut(); state.position = [1.0, 0.0]; state.yaw = 37.0; state.set_float(TickFloatField::AttackAnim, 1.0); }
        let mut host = Host::new(&actor); host.grounded = false; host.fail = Some("head");
        assert_eq!(tick(&actor, &mut host), Err("head"));
        assert_eq!(host.head_arguments, [(37.0, 3.0)]);
        assert_eq!(actor.borrow().float(TickFloatField::Run), 0.0);
        assert_eq!(host.calls.last().unwrap(), "head");
        assert!(!host.calls.iter().any(|call| call.starts_with("pop:")));
        assert_eq!(java_max(-0.0, 0.0).to_bits(), 0.0_f32.to_bits());
        assert_eq!(java_min(0.0, -0.0).to_bits(), (-0.0_f32).to_bits());
        assert!(java_min(1.0, f32::NAN).is_nan());
    }
}
