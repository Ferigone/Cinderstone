//! Reentrant Java 1.21 Bat tick/custom-AI dispatch over retained actual state.
//!
//! Bat.tick's inherited LivingEntity tick normally calls the virtual Bat custom
//! AI method on the same object. Holding `&mut BatState` across that inherited
//! call prevents a safe native owner from implementing this ordinary callgraph.
//! These drivers use short, callback-free state accesses instead. They create
//! no actor, state arena, identity, RNG, scheduler, navigation or Main callsite.

use super::bat::{self, BatHost, BatState, BlockPosition};

/// Source null-field failures after synchronous reentry. Each variant names
/// the Java dereference, rather than fabricating another target or continuing
/// with a pre-callback target snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetRead {
    ValidationY,
    CloserToCenter,
    SteeringX,
    SteeringY,
    SteeringZ,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RetainedBatError<E> {
    Host(E),
    NullTarget(TargetRead),
}
impl<E> From<E> for RetainedBatError<E> {
    fn from(error: E) -> Self {
        Self::Host(error)
    }
}

/// The inherited BatHost is permanently bound to one actual actor lifetime.
/// Every implicit actor operation on it, including after removal/reentry, must
/// refer to that actor. An ID lookup or mutable "current actor" slot must not
/// redirect operations to a replacement sharing its numeric ID.
///
/// Lease retains that SAME actor and its canonical BatState for the whole
/// invocation. It is supplied by the actual owner before entering this driver;
/// it must survive removal/replacement and must not resolve through a current
/// live-entity registry on each access. Passing another actor's lease is outside
/// this contract. Actual constructor/admission ownership remains with the host.
///
/// with_state executes only the supplied callback-free field fragment on the
/// exact canonical object. It must release ALL actor/state borrows before
/// returning, without moving state out of its owner or substituting a snapshot.
/// Inherited/world/RNG/metadata callbacks consequently have their full context
/// available for normal virtual custom-AI dispatch or synchronous reentry.
/// Lookup errors occur before executing the closure; executed mutations remain
/// committed. Bat's two animation fields retain their original final-object
/// identity; reentry may mutate them but may not replace them with new objects.
pub trait RetainedBatHost: BatHost {
    type Lease;
    fn with_state<R>(
        &mut self,
        retained: &Self::Lease,
        access: impl FnOnce(&mut BatState) -> R,
    ) -> Result<R, Self::Error>;
}

fn target<H: RetainedBatHost>(
    retained: &H::Lease,
    read: TargetRead,
    host: &mut H,
) -> Result<BlockPosition, RetainedBatError<H::Error>> {
    host.with_state(retained, |state| state.target_position())?
        .ok_or(RetainedBatError::NullTarget(read))
}

/// Source Bat.tick131–139 and setupAnimationStates269–276. The FIRST operation
/// is the actual inherited tick, with no BatState borrow held. It can call
/// custom_server_ai_step_retained on this exact lease through the ordinary
/// LivingEntity.aiStep -> Mob.serverAiStep -> virtual custom-AI callgraph.
pub fn tick_retained<H: RetainedBatHost>(
    retained: &H::Lease,
    host: &mut H,
) -> Result<(), RetainedBatError<H::Error>> {
    host.inherited_tick()?;
    if bat::is_resting(host)? {
        host.set_delta_movement([0.0; 3])?;
        let x = host.x()?;
        let y =
            f64::from(bat::java_floor(host.y()?)) + 1.0 - f64::from(host.bounding_box_height()?);
        let z = host.z()?;
        host.set_position([x, y, z])?;
    } else {
        let velocity = host.delta_movement()?;
        host.set_delta_movement([velocity[0] * 1.0, velocity[1] * 0.6, velocity[2] * 1.0])?;
    }
    // This is a fresh metadata read after the physical callbacks, as in gg().
    if bat::is_resting(host)? {
        host.with_state(retained, |state| state.stop_fly_animation())?;
        let tick = host.actor_tick_count()?;
        host.with_state(retained, |state| {
            state.start_rest_animation_if_stopped(tick)
        })?;
    } else {
        host.with_state(retained, |state| state.stop_rest_animation())?;
        let tick = host.actor_tick_count()?;
        host.with_state(retained, |state| state.start_fly_animation_if_stopped(tick))?;
    }
    Ok(())
}

/// Source Bat.customServerAiStep143–197. Only source locals survive callbacks:
/// current/above block positions, resting-branch silent, coordinate/velocity
/// temporaries and the closerToCenterThan receiver captured before position().
/// targetPosition is read again at EACH original field-read/dereference point.
/// A nested call may therefore change which target supplies later coordinates.
pub fn custom_server_ai_step_retained<H: RetainedBatHost>(
    retained: &H::Lease,
    host: &mut H,
) -> Result<(), RetainedBatError<H::Error>> {
    host.inherited_custom_server_ai_step()?;
    let at = host.block_position()?;
    let above = (at.0, at.1.wrapping_add(1), at.2);
    if bat::is_resting(host)? {
        let silent = host.is_silent()?;
        let ceiling = host.block_state(above)?;
        if host.is_redstone_conductor(&ceiling, at)? {
            if host.next_entity_int(200)? == 0 {
                let yaw = host.next_entity_int(360)? as f32;
                host.set_head_yaw(yaw)?;
            }
            if host.has_nearest_resting_target_player()? {
                bat::set_resting(host, false)?;
                if !silent {
                    host.level_event(1025, at, 0)?;
                }
            }
        } else {
            bat::set_resting(host, false)?;
            if !silent {
                host.level_event(1025, at, 0)?;
            }
        }
    } else {
        if host.with_state(retained, |state| state.target_position().is_some())? {
            let empty_argument = target(retained, TargetRead::ValidationY, host)?;
            let invalidate = if !host.is_empty_block(empty_argument)? {
                true
            } else {
                // Source rereads target AFTER isEmptyBlock, then gets its Y
                // BEFORE querying the level's minimum build height.
                let target_y = target(retained, TargetRead::ValidationY, host)?.1;
                target_y <= host.minimum_build_height()?
            };
            if invalidate {
                host.with_state(retained, |state| state.set_target_position(None))?;
            }
        }
        let retarget = if host.with_state(retained, |state| state.target_position().is_none())? {
            true
        } else if host.next_entity_int(30)? == 0 {
            true
        } else {
            // JVM loads the nullable target RECEIVER before evaluating the
            // position() argument. Even a null receiver still evaluates that
            // argument before invokevirtual raises the null-field failure.
            let receiver = host.with_state(retained, |state| state.target_position())?;
            let position = host.position()?;
            let receiver =
                receiver.ok_or(RetainedBatError::NullTarget(TargetRead::CloserToCenter))?;
            bat::closer_to_center_than(receiver, position, 2.0)
        };
        if retarget {
            let x = host.x()? + f64::from(host.next_entity_int(7)?)
                - f64::from(host.next_entity_int(7)?);
            let y = host.y()? + f64::from(host.next_entity_int(6)?) - 2.0;
            let z = host.z()? + f64::from(host.next_entity_int(7)?)
                - f64::from(host.next_entity_int(7)?);
            let position = (bat::java_floor(x), bat::java_floor(y), bat::java_floor(z));
            host.with_state(retained, |state| state.set_target_position(Some(position)))?;
        }
        let target_x = target(retained, TargetRead::SteeringX, host)?.0;
        let dx = f64::from(target_x) + 0.5 - host.x()?;
        let target_y = target(retained, TargetRead::SteeringY, host)?.1;
        let dy = f64::from(target_y) + 0.1 - host.y()?;
        let target_z = target(retained, TargetRead::SteeringZ, host)?.2;
        let dz = f64::from(target_z) + 0.5 - host.z()?;
        let velocity = host.delta_movement()?;
        let next = bat::steering_velocity(velocity, [dx, dy, dz]);
        host.set_delta_movement(next)?;
        // newVelocity is a source local. Reentry from setDeltaMovement may
        // change the current actor velocity, but yaw still uses THIS value.
        let wanted_yaw = (bat::mth_atan2(next[2], next[0]) * 57.2957763671875) as f32 - 90.0_f32;
        let turn = bat::wrap_degrees(wanted_yaw - host.yaw()?);
        host.set_forward_input(0.5)?;
        let yaw = host.yaw()? + turn;
        host.set_yaw(yaw)?;
        if host.next_entity_int(100)? == 0 {
            let ceiling = host.block_state(above)?;
            if host.is_redstone_conductor(&ceiling, above)? {
                bat::set_resting(host, true)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::bat::Vector;
    use super::*;
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    // A retained-object/registry callback recorder. It is not a production mob
    // host or a replacement LivingEntity implementation. The nested driver call
    // below exercises the real Rust borrowing shape of the NORMAL callgraph.
    #[derive(Clone)]
    struct Lease(Rc<RefCell<BatState>>);
    struct Host {
        bound: Option<Lease>,
        active: Option<Lease>,
        replacement: Option<Lease>,
        flags: u8,
        position: Vector,
        velocity: Vector,
        yaw: f32,
        tick: i32,
        trace: Vec<String>,
        draws: VecDeque<(i32, i32)>,
        empty: bool,
        normal_nested_ai: bool,
        replace_active_after_nested: bool,
        target_on_empty: Option<Option<BlockPosition>>,
        target_on_min: Option<Option<BlockPosition>>,
        target_on_random30: Option<Option<BlockPosition>>,
        target_on_position: Option<Option<BlockPosition>>,
        target_on_x: Option<Option<BlockPosition>>,
        target_on_y: Option<Option<BlockPosition>>,
        velocity_after_set: Option<Vector>,
        fail: Option<&'static str>,
    }
    impl Host {
        fn unbound() -> Self {
            Self {
                bound: None,
                active: None,
                replacement: None,
                flags: 0,
                position: [0.0, 64.0, 0.0],
                velocity: [0.0; 3],
                yaw: 0.0,
                tick: 10,
                trace: vec![],
                draws: VecDeque::new(),
                empty: true,
                normal_nested_ai: false,
                replace_active_after_nested: false,
                target_on_empty: None,
                target_on_min: None,
                target_on_random30: None,
                target_on_position: None,
                target_on_x: None,
                target_on_y: None,
                velocity_after_set: None,
                fail: None,
            }
        }
        fn new() -> (Self, Lease) {
            let mut host = Self::unbound();
            let state = BatState::construct(&mut host).unwrap();
            let lease = Lease(Rc::new(RefCell::new(state)));
            host.bound = Some(lease.clone());
            host.active = Some(lease.clone());
            host.flags = 0;
            host.trace.clear();
            (host, lease)
        }
        fn callback_available(&self) {
            if let Some(lease) = &self.bound {
                drop(
                    lease
                        .0
                        .try_borrow_mut()
                        .expect("actor/state borrow leaked into host callback"),
                );
            }
            if let Some(lease) = &self.active {
                drop(
                    lease
                        .0
                        .try_borrow_mut()
                        .expect("live registry actor borrowed across callback"),
                );
            }
        }
        fn record(&mut self, event: impl Into<String>) -> Result<(), &'static str> {
            self.callback_available();
            let event = event.into();
            let fail = self.fail.is_some_and(|wanted| wanted == event);
            self.trace.push(event);
            if fail {
                Err("controlled callback failure")
            } else {
                Ok(())
            }
        }
        fn set_target(&mut self, target: Option<BlockPosition>) {
            let lease = self.bound.as_ref().unwrap().clone();
            self.with_state(&lease, |state| state.set_target_position(target))
                .unwrap();
        }
        fn draws(&mut self, values: &[(i32, i32)]) {
            self.draws = values.iter().copied().collect();
        }
    }
    impl BatHost for Host {
        type Error = &'static str;
        type Source = ();
        type BlockState = BlockPosition;
        fn inherited_construct(&mut self) -> Result<(), Self::Error> {
            self.record("construct")
        }
        fn inherited_define_synched_data(&mut self) -> Result<(), Self::Error> {
            self.record("define-super")
        }
        fn define_bat_flags(&mut self, n: u8) -> Result<(), Self::Error> {
            self.record("define")?;
            self.flags = n;
            Ok(())
        }
        fn is_client_side(&mut self) -> Result<bool, Self::Error> {
            self.record("client")?;
            Ok(false)
        }
        fn bat_flags(&mut self) -> Result<u8, Self::Error> {
            self.record("flags")?;
            Ok(self.flags)
        }
        fn set_bat_flags(&mut self, n: u8) -> Result<(), Self::Error> {
            self.record("set-flags")?;
            self.flags = n;
            Ok(())
        }
        fn inherited_tick(&mut self) -> Result<(), Self::Error> {
            self.record("inherited-tick")?;
            if self.normal_nested_ai {
                self.normal_nested_ai = false;
                let retained = self.bound.as_ref().unwrap().clone();
                custom_server_ai_step_retained(&retained, self).map_err(|e| match e {
                    RetainedBatError::Host(error) => error,
                    RetainedBatError::NullTarget(_) => "source null target in nested AI",
                })?;
                self.record("inherited-after-virtual-ai")?;
            }
            if self.replace_active_after_nested {
                self.active = self.replacement.take();
                self.record("old-actor-removed-and-live-entry-replaced")?;
            }
            Ok(())
        }
        fn inherited_custom_server_ai_step(&mut self) -> Result<(), Self::Error> {
            self.record("inherited-ai")
        }
        fn actor_tick_count(&mut self) -> Result<i32, Self::Error> {
            self.record("tick")?;
            Ok(self.tick)
        }
        fn block_position(&mut self) -> Result<BlockPosition, Self::Error> {
            self.record("block-position")?;
            Ok((0, 64, 0))
        }
        fn position(&mut self) -> Result<Vector, Self::Error> {
            self.record("position")?;
            if let Some(n) = self.target_on_position.take() {
                self.set_target(n);
            }
            Ok(self.position)
        }
        fn x(&mut self) -> Result<f64, Self::Error> {
            self.record("x")?;
            if let Some(n) = self.target_on_x.take() {
                self.set_target(n);
            }
            Ok(self.position[0])
        }
        fn y(&mut self) -> Result<f64, Self::Error> {
            self.record("y")?;
            if let Some(n) = self.target_on_y.take() {
                self.set_target(n);
            }
            Ok(self.position[1])
        }
        fn z(&mut self) -> Result<f64, Self::Error> {
            self.record("z")?;
            Ok(self.position[2])
        }
        fn bounding_box_height(&mut self) -> Result<f32, Self::Error> {
            self.record("height")?;
            Ok(0.9)
        }
        fn delta_movement(&mut self) -> Result<Vector, Self::Error> {
            self.record("velocity")?;
            Ok(self.velocity)
        }
        fn set_delta_movement(&mut self, v: Vector) -> Result<(), Self::Error> {
            self.record("set-velocity")?;
            self.velocity = v;
            if let Some(other) = self.velocity_after_set.take() {
                self.velocity = other;
            }
            Ok(())
        }
        fn set_position(&mut self, v: Vector) -> Result<(), Self::Error> {
            self.record("set-position")?;
            self.position = v;
            Ok(())
        }
        fn is_silent(&mut self) -> Result<bool, Self::Error> {
            self.record("silent")?;
            Ok(false)
        }
        fn block_state(&mut self, p: BlockPosition) -> Result<BlockPosition, Self::Error> {
            self.record(format!("block:{p:?}"))?;
            Ok(p)
        }
        fn is_redstone_conductor(
            &mut self,
            s: &BlockPosition,
            p: BlockPosition,
        ) -> Result<bool, Self::Error> {
            self.record(format!("conductor:{s:?}:{p:?}"))?;
            Ok(true)
        }
        fn has_nearest_resting_target_player(&mut self) -> Result<bool, Self::Error> {
            self.record("nearest")?;
            Ok(false)
        }
        fn is_empty_block(&mut self, p: BlockPosition) -> Result<bool, Self::Error> {
            self.record(format!("empty:{p:?}"))?;
            if let Some(n) = self.target_on_empty.take() {
                self.set_target(n);
            }
            Ok(self.empty)
        }
        fn minimum_build_height(&mut self) -> Result<i32, Self::Error> {
            self.record("min")?;
            if let Some(n) = self.target_on_min.take() {
                self.set_target(n);
            }
            Ok(-64)
        }
        fn next_entity_int(&mut self, bound: i32) -> Result<i32, Self::Error> {
            self.record(format!("random:{bound}"))?;
            let (wanted, n) = self.draws.pop_front().expect("unexpected RNG call");
            assert_eq!(bound, wanted);
            if bound == 30 {
                if let Some(target) = self.target_on_random30.take() {
                    self.set_target(target);
                }
            }
            Ok(n)
        }
        fn set_head_yaw(&mut self, n: f32) -> Result<(), Self::Error> {
            self.record(format!("head:{n}"))
        }
        fn yaw(&mut self) -> Result<f32, Self::Error> {
            self.record("yaw")?;
            Ok(self.yaw)
        }
        fn set_yaw(&mut self, n: f32) -> Result<(), Self::Error> {
            self.record("set-yaw")?;
            self.yaw = n;
            Ok(())
        }
        fn set_forward_input(&mut self, _: f32) -> Result<(), Self::Error> {
            self.record("forward")
        }
        fn level_event(&mut self, e: i32, p: BlockPosition, d: i32) -> Result<(), Self::Error> {
            self.record(format!("event:{e}:{p:?}:{d}"))
        }
        fn is_invulnerable_to(&mut self, _: &()) -> Result<bool, Self::Error> {
            self.record("invulnerable")?;
            Ok(false)
        }
        fn inherited_hurt(&mut self, _: &(), _: f32) -> Result<bool, Self::Error> {
            self.record("hurt")?;
            Ok(false)
        }
    }
    impl RetainedBatHost for Host {
        type Lease = Lease;
        fn with_state<R>(
            &mut self,
            lease: &Lease,
            access: impl FnOnce(&mut BatState) -> R,
        ) -> Result<R, Self::Error> {
            assert!(
                Rc::ptr_eq(&self.bound.as_ref().unwrap().0, &lease.0),
                "lease changed actor lifetime"
            );
            let mut state = lease.0.borrow_mut();
            Ok(access(&mut state))
        }
    }
    fn new_target_draws() -> [(i32, i32); 6] {
        [(7, 6), (7, 2), (6, 5), (7, 1), (7, 4), (100, 1)]
    }

    #[test]
    fn normal_inherited_tick_reenters_same_state_and_retains_removed_actor() {
        let (mut h, lease) = Host::new();
        let (_, replacement) = Host::new();
        h.replacement = Some(replacement.clone());
        h.replace_active_after_nested = true;
        h.normal_nested_ai = true;
        h.draws(&new_target_draws());
        tick_retained(&lease, &mut h).unwrap();
        assert!(Rc::ptr_eq(&h.active.as_ref().unwrap().0, &replacement.0));
        assert_eq!(lease.0.borrow().target_position(), Some((4, 67, -3)));
        assert_eq!(replacement.0.borrow().target_position(), None);
        assert!(lease.0.borrow().fly_animation_state().is_started());
        assert!(!replacement.0.borrow().fly_animation_state().is_started());
        assert_eq!(h.velocity[1], 0.699999988079071 * 0.10000000149011612 * 0.6);
        assert_eq!(h.trace.first().unwrap(), "inherited-tick");
        assert!(
            h.trace.iter().position(|e| e == "inherited-ai").unwrap()
                < h.trace
                    .iter()
                    .position(|e| e == "inherited-after-virtual-ai")
                    .unwrap()
        );
        assert_eq!(h.trace.iter().filter(|e| *e == "set-velocity").count(), 2);
        assert!(h.draws.is_empty());
    }
    #[test]
    fn target_validation_rereads_after_world_call_then_captures_y_before_minimum() {
        let (mut h, lease) = Host::new();
        h.set_target(Some((10, 20, 30)));
        h.target_on_empty = Some(Some((99, -64, 99)));
        h.target_on_min = Some(Some((100, 100, 100)));
        h.draws(&new_target_draws());
        custom_server_ai_step_retained(&lease, &mut h).unwrap();
        assert_eq!(lease.0.borrow().target_position(), Some((4, 67, -3)));
        assert!(h.trace.iter().any(|e| e == "empty:(10, 20, 30)"));
        assert!(!h.trace.iter().any(|e| e == "random:30"));
        // The captured -64 Y makes the outer call clear the reentrant target.
        assert!(h.draws.is_empty());

        let (mut h, lease) = Host::new();
        h.set_target(Some((10, 20, 30)));
        h.target_on_empty = Some(None);
        assert_eq!(
            custom_server_ai_step_retained(&lease, &mut h),
            Err(RetainedBatError::NullTarget(TargetRead::ValidationY))
        );
        assert!(h.trace.last().unwrap().starts_with("empty:"));
        assert!(!h.trace.iter().any(|e| e == "min"));
    }
    #[test]
    fn closer_receiver_is_read_after_rng_and_retained_across_position_argument() {
        let (mut h, lease) = Host::new();
        h.set_target(Some((10, 20, 30)));
        h.target_on_random30 = Some(Some((0, 64, 0)));
        h.target_on_position = Some(Some((100, 100, 100)));
        let mut draws = vec![(30, 1)];
        draws.extend(new_target_draws());
        h.draws(&draws);
        custom_server_ai_step_retained(&lease, &mut h).unwrap();
        assert_eq!(lease.0.borrow().target_position(), Some((4, 67, -3)));
        assert!(h.draws.is_empty());

        let (mut h, lease) = Host::new();
        h.set_target(Some((10, 20, 30)));
        h.target_on_random30 = Some(None);
        h.target_on_position = Some(Some((100, 100, 100)));
        h.draws(&[(30, 1)]);
        assert_eq!(
            custom_server_ai_step_retained(&lease, &mut h),
            Err(RetainedBatError::NullTarget(TargetRead::CloserToCenter))
        );
        assert_eq!(h.trace.last().unwrap(), "position"); // null receiver still evaluates argument
        assert_eq!(lease.0.borrow().target_position(), Some((100, 100, 100)));
    }
    #[test]
    fn steering_rereads_three_targets_but_preserves_new_velocity_local() {
        let (mut h, lease) = Host::new();
        h.set_target(Some((10, 70, 30)));
        h.target_on_x = Some(Some((99, -10, 88)));
        h.target_on_y = Some(Some((77, 99, -2)));
        h.velocity_after_set = Some([9.0; 3]);
        h.draws(&[(30, 1), (100, 1)]);
        custom_server_ai_step_retained(&lease, &mut h).unwrap();
        let original_next = bat::steering_velocity([0.0; 3], [10.5, -73.9, -1.5]);
        let expected_yaw = bat::wrap_degrees(
            (bat::mth_atan2(original_next[2], original_next[0]) * 57.2957763671875) as f32 - 90.0,
        );
        assert_eq!(h.velocity, [9.0; 3]);
        assert_eq!(h.yaw, expected_yaw);
        assert_eq!(lease.0.borrow().target_position(), Some((77, 99, -2)));
        assert!(h.yaw < -90.0); // snapshotting the current +9/+9 velocity would turn the other way

        let (mut h, lease) = Host::new();
        h.set_target(Some((10, 70, 30)));
        h.target_on_x = Some(None);
        h.draws(&[(30, 1)]);
        assert_eq!(
            custom_server_ai_step_retained(&lease, &mut h),
            Err(RetainedBatError::NullTarget(TargetRead::SteeringY))
        );
        assert_eq!(h.trace.last().unwrap(), "x");
    }
    #[test]
    fn nested_inherited_failure_keeps_target_and_rng_prefix_without_tick_tail() {
        let (mut h, lease) = Host::new();
        h.normal_nested_ai = true;
        h.fail = Some("velocity");
        h.draws(&new_target_draws()[..5]);
        assert_eq!(
            tick_retained(&lease, &mut h),
            Err(RetainedBatError::Host("controlled callback failure"))
        );
        assert_eq!(lease.0.borrow().target_position(), Some((4, 67, -3)));
        assert!(h.draws.is_empty());
        assert_eq!(h.velocity, [0.0; 3]);
        assert_eq!(h.trace.last().unwrap(), "velocity");
        assert!(!lease.0.borrow().fly_animation_state().is_started());
        assert!(!h
            .trace
            .iter()
            .any(|e| e == "inherited-after-virtual-ai" || e == "set-position" || e == "tick"));
    }
}
