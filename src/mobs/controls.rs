//! Ordinary Java 1.21 Mob control owners (`bzk`, `bzn`, `bzo`, `bzp`).
//!
//! These are the actual base controls constructed by Mob, including Bat. They
//! do not implement the derived flying/swimming controls, a navigation owner,
//! actor factory, scheduler, inherited LivingEntity phases or Main admission.
//! Each driver retains its original control object and releases every native
//! state/actor borrow before invoking actual actor, world or navigation code.

#[path = "controls/body_rotation.rs"]
pub mod body_rotation;
#[path = "controls/jump_control.rs"]
pub mod jump_control;
#[path = "controls/look_control.rs"]
pub mod look_control;
#[path = "controls/move_control.rs"]
pub mod move_control;

/// An implementation is permanently bound to ONE actual Mob lifetime. After
/// synchronous removal/replacement its operations still address that Mob, not
/// a new occupant of a numeric ID. Control leases retain the corresponding
/// original control AND actor, even if an actor's current control is replaced.
///
/// Every `with_*_state` closure is callback-free and accesses the canonical
/// source fields in place. It releases ALL actor/state borrows before returning
/// and never moves state out or substitutes a snapshot. Errors happen before
/// running the closure; completed mutations remain committed. Actual virtual
/// calls may reenter these drivers with the same lease. No successful dummy
/// owner operations are provided.
pub trait ControlHost {
    type Error;
}

/// Callback-free source FINAL Entity.getX/getY/getZ accessors. These read the
/// actual canonical Entity position; they may not introduce virtual game code.
pub trait PositionHost: ControlHost {
    fn x(&mut self) -> Result<f64, Self::Error>;
    fn y(&mut self) -> Result<f64, Self::Error>;
    fn z(&mut self) -> Result<f64, Self::Error>;
}

pub trait OrientationHost: ControlHost {
    /// Source Entity.getYRot/getXRot and setYRot/setXRot virtual methods.
    fn yaw(&mut self) -> Result<f32, Self::Error>;
    fn pitch(&mut self) -> Result<f32, Self::Error>;
    fn set_yaw(&mut self, value: f32) -> Result<(), Self::Error>;
    fn set_pitch(&mut self, value: f32) -> Result<(), Self::Error>;
    /// These four operations are callback-free reads/writes of the actual
    /// LivingEntity yHeadRot/yBodyRot fields, not virtual replacement methods.
    fn head_yaw(&mut self) -> Result<f32, Self::Error>;
    fn body_yaw(&mut self) -> Result<f32, Self::Error>;
    fn set_head_yaw(&mut self, value: f32) -> Result<(), Self::Error>;
    fn set_body_yaw(&mut self, value: f32) -> Result<(), Self::Error>;
    fn max_head_yaw(&mut self) -> Result<i32, Self::Error>;
}

// Java Math.min/max preserve NaN and distinguish signed zero. Rust f32::min/max
// select the non-NaN operand, which changes the source control arithmetic.
pub(super) fn java_min(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        f32::from_bits(a.to_bits() | b.to_bits())
    } else if a <= b {
        a
    } else {
        b
    }
}
pub(super) fn java_max(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        a
    } else if b.is_nan() {
        b
    } else if a == 0.0 && b == 0.0 {
        f32::from_bits(a.to_bits() & b.to_bits())
    } else if a >= b {
        a
    } else {
        b
    }
}
pub(super) fn clamp(value: f32, minimum: f32, maximum: f32) -> f32 {
    if value < minimum {
        minimum
    } else {
        java_min(value, maximum)
    }
}
/// Source Mth.rotateIfNecessary (ayo.c(FFF)), including the target-relative
/// subtraction. This differs from LookControl.rotateTowards.
pub(super) fn rotate_if_necessary(current: f32, target: f32, maximum: f32) -> f32 {
    let difference = super::bat::wrap_degrees(target - current);
    let limited = clamp(difference, -maximum, maximum);
    target - limited
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_clamp_nan_zero_and_distinct_rotation_forms() {
        assert!(clamp(f32::NAN, -1.0, 1.0).is_nan());
        assert!(java_max(1.0, f32::NAN).is_nan());
        assert_eq!(java_min(0.0, -0.0).to_bits(), (-0.0_f32).to_bits());
        assert_eq!(java_max(-0.0, 0.0).to_bits(), 0.0_f32.to_bits());
        assert_eq!(rotate_if_necessary(0.0, 100.0, 10.0), 90.0);
        assert_eq!(look_control::rotate_towards(0.0, 100.0, 10.0), 10.0);
        assert_eq!(move_control::rotlerp(350.0, 10.0, 90.0), 10.0);
        assert_eq!(move_control::rotlerp(360.0, 360.0, 90.0), 360.0);
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use super::{jump_control::*, look_control::*, move_control::*, *};
    use std::{cell::RefCell, rc::Rc};
    pub struct Host {
        pub moves: Rc<RefCell<MoveControlState>>,
        pub looks: Rc<RefCell<LookControlState>>,
        pub jumps: Rc<RefCell<JumpControlState>>,
        pub position: [f64; 3],
        pub yaw: f32,
        pub pitch: f32,
        pub head: f32,
        pub body: f32,
        pub forward: f32,
        pub right: f32,
        pub speed: f32,
        pub attribute: f64,
        pub grounded: bool,
        pub nav_done: bool,
        pub walkable: bool,
        pub shape_empty: bool,
        pub shape_top: f64,
        pub up: f32,
        pub width: f32,
        pub events: Vec<&'static str>,
        pub fail: Option<&'static str>,
        pub hook: Option<(&'static str, fn(&mut Self) -> Result<(), &'static str>)>,
    }
    impl Default for Host {
        fn default() -> Self {
            Self {
                moves: Rc::new(RefCell::new(MoveControlState::new())),
                looks: Rc::new(RefCell::new(LookControlState::new())),
                jumps: Rc::new(RefCell::new(JumpControlState::new())),
                position: [0.; 3],
                yaw: 0.,
                pitch: 25.,
                head: 0.,
                body: 100.,
                forward: 0.5,
                right: 0.,
                speed: 0.,
                attribute: 0.7,
                grounded: false,
                nav_done: true,
                walkable: true,
                shape_empty: true,
                shape_top: 1.,
                up: 0.6,
                width: 0.5,
                events: vec![],
                fail: None,
                hook: None,
            }
        }
    }
    impl Host {
        pub fn event(&mut self, name: &'static str) -> Result<(), &'static str> {
            self.events.push(name);
            if self.hook.as_ref().is_some_and(|h| h.0 == name) {
                let (_, hook) = self.hook.take().unwrap();
                hook(self)?;
            }
            if self.fail == Some(name) {
                Err(name)
            } else {
                Ok(())
            }
        }
    }
    impl ControlHost for Host {
        type Error = &'static str;
    }
    impl PositionHost for Host {
        fn x(&mut self) -> Result<f64, Self::Error> {
            self.event("x")?;
            Ok(self.position[0])
        }
        fn y(&mut self) -> Result<f64, Self::Error> {
            self.event("y")?;
            Ok(self.position[1])
        }
        fn z(&mut self) -> Result<f64, Self::Error> {
            self.event("z")?;
            Ok(self.position[2])
        }
    }
    impl OrientationHost for Host {
        fn yaw(&mut self) -> Result<f32, Self::Error> {
            self.event("yaw")?;
            Ok(self.yaw)
        }
        fn pitch(&mut self) -> Result<f32, Self::Error> {
            self.event("pitch")?;
            Ok(self.pitch)
        }
        fn set_yaw(&mut self, v: f32) -> Result<(), Self::Error> {
            self.event("set_yaw")?;
            self.yaw = v;
            Ok(())
        }
        fn set_pitch(&mut self, v: f32) -> Result<(), Self::Error> {
            self.event("set_pitch")?;
            self.pitch = v;
            Ok(())
        }
        fn head_yaw(&mut self) -> Result<f32, Self::Error> {
            Ok(self.head)
        }
        fn body_yaw(&mut self) -> Result<f32, Self::Error> {
            Ok(self.body)
        }
        fn set_head_yaw(&mut self, v: f32) -> Result<(), Self::Error> {
            self.head = v;
            Ok(())
        }
        fn set_body_yaw(&mut self, v: f32) -> Result<(), Self::Error> {
            self.body = v;
            Ok(())
        }
        fn max_head_yaw(&mut self) -> Result<i32, Self::Error> {
            self.event("max_head_yaw")?;
            Ok(75)
        }
    }
    impl JumpControlHost for Host {
        type JumpLease = Rc<RefCell<JumpControlState>>;
        fn with_jump_state<R>(
            &mut self,
            l: &Self::JumpLease,
            f: impl FnOnce(&mut JumpControlState) -> R,
        ) -> Result<R, Self::Error> {
            Ok(f(&mut l.borrow_mut()))
        }
        fn set_jumping(&mut self, _: bool) -> Result<(), Self::Error> {
            self.event("set_jumping")
        }
    }
    impl MoveControlHost for Host {
        type MoveLease = Rc<RefCell<MoveControlState>>;
        type Level = u8;
        type BlockState = u8;
        type CollisionShape = u8;
        type Navigation = u8;
        type NodeEvaluator = u8;
        type JumpControl = Rc<RefCell<JumpControlState>>;
        fn with_move_state<R>(
            &mut self,
            l: &Self::MoveLease,
            f: impl FnOnce(&mut MoveControlState) -> R,
        ) -> Result<R, Self::Error> {
            Ok(f(&mut l.borrow_mut()))
        }
        fn movement_speed_attribute(&mut self) -> Result<f64, Self::Error> {
            self.event("attribute")?;
            Ok(self.attribute)
        }
        fn set_speed(&mut self, v: f32) -> Result<(), Self::Error> {
            self.event("set_speed")?;
            self.speed = v;
            Ok(())
        }
        fn set_forward_input(&mut self, v: f32) -> Result<(), Self::Error> {
            self.event("set_forward")?;
            self.forward = v;
            Ok(())
        }
        fn set_strafe_input(&mut self, v: f32) -> Result<(), Self::Error> {
            self.event("set_strafe")?;
            self.right = v;
            Ok(())
        }
        fn on_ground(&mut self) -> Result<bool, Self::Error> {
            self.event("on_ground")?;
            Ok(self.grounded)
        }
        fn max_up_step(&mut self) -> Result<f32, Self::Error> {
            self.event("max_up_step")?;
            Ok(self.up)
        }
        fn bounding_box_width(&mut self) -> Result<f32, Self::Error> {
            self.event("width")?;
            Ok(self.width)
        }
        fn block_position(&mut self) -> Result<(i32, i32, i32), Self::Error> {
            self.event("block_position")?;
            Ok((0, 0, 0))
        }
        fn block_y(&mut self) -> Result<i32, Self::Error> {
            self.event("block_y")?;
            Ok(0)
        }
        fn level(&mut self) -> Result<u8, Self::Error> {
            self.event("level")?;
            Ok(1)
        }
        fn block_state(&mut self, l: &u8, _: (i32, i32, i32)) -> Result<u8, Self::Error> {
            assert_eq!(*l, 1);
            self.event("block_state")?;
            Ok(2)
        }
        fn collision_shape(
            &mut self,
            s: &u8,
            l: &u8,
            _: (i32, i32, i32),
        ) -> Result<u8, Self::Error> {
            assert_eq!((*s, *l), (2, 1));
            self.event("shape")?;
            Ok(3)
        }
        fn shape_is_empty(&mut self, s: &u8) -> Result<bool, Self::Error> {
            assert_eq!(*s, 3);
            self.event("shape_empty")?;
            Ok(self.shape_empty)
        }
        fn shape_max_y(&mut self, s: &u8) -> Result<f64, Self::Error> {
            assert_eq!(*s, 3);
            self.event("shape_top")?;
            Ok(self.shape_top)
        }
        fn state_is_door(&mut self, s: &u8) -> Result<bool, Self::Error> {
            assert_eq!(*s, 2);
            self.event("door")?;
            Ok(false)
        }
        fn state_is_fence(&mut self, s: &u8) -> Result<bool, Self::Error> {
            assert_eq!(*s, 2);
            self.event("fence")?;
            Ok(false)
        }
        fn navigation(&mut self) -> Result<Option<u8>, Self::Error> {
            self.event("navigation")?;
            Ok(Some(4))
        }
        fn node_evaluator(&mut self, n: &u8) -> Result<Option<u8>, Self::Error> {
            assert_eq!(*n, 4);
            self.event("evaluator")?;
            Ok(Some(5))
        }
        fn path_type_is_walkable(
            &mut self,
            n: &u8,
            _: (i32, i32, i32),
        ) -> Result<bool, Self::Error> {
            assert_eq!(*n, 5);
            self.event("path_type")?;
            Ok(self.walkable)
        }
        fn jump_control(&mut self) -> Result<Self::JumpControl, Self::Error> {
            self.event("jump_control")?;
            Ok(self.jumps.clone())
        }
        fn request_jump(&mut self, j: &Self::JumpControl) -> Result<(), Self::Error> {
            self.event("request_jump")?;
            jump(j, self)
        }
    }
    impl LookControlHost for Host {
        type LookLease = Rc<RefCell<LookControlState>>;
        type Navigation = u8;
        type Target = bool;
        type BoundingBox = u8;
        fn with_look_state<R>(
            &mut self,
            l: &Self::LookLease,
            f: impl FnOnce(&mut LookControlState) -> R,
        ) -> Result<R, Self::Error> {
            Ok(f(&mut l.borrow_mut()))
        }
        fn eye_y(&mut self) -> Result<f64, Self::Error> {
            self.event("eye_y")?;
            Ok(self.position[1])
        }
        fn head_rotation_speed(&mut self) -> Result<i32, Self::Error> {
            self.event("head_speed")?;
            Ok(10)
        }
        fn max_head_pitch(&mut self) -> Result<i32, Self::Error> {
            self.event("max_pitch")?;
            Ok(40)
        }
        fn navigation(&mut self) -> Result<u8, Self::Error> {
            self.event("navigation")?;
            Ok(4)
        }
        fn navigation_is_done(&mut self, n: &u8) -> Result<bool, Self::Error> {
            assert_eq!(*n, 4);
            self.event("navigation_done")?;
            Ok(self.nav_done)
        }
        fn target_x(&mut self, _: &bool) -> Result<f64, Self::Error> {
            self.event("target_x")?;
            Ok(1.)
        }
        fn target_z(&mut self, _: &bool) -> Result<f64, Self::Error> {
            self.event("target_z")?;
            Ok(3.)
        }
        fn target_is_living(&mut self, t: &bool) -> Result<bool, Self::Error> {
            Ok(*t)
        }
        fn target_eye_y(&mut self, _: &bool) -> Result<f64, Self::Error> {
            self.event("target_eye")?;
            Ok(2.)
        }
        fn target_bounding_box(&mut self, _: &bool) -> Result<u8, Self::Error> {
            self.event("target_bounds")?;
            Ok(7)
        }
        fn bounding_box_min_y(&mut self, b: &u8) -> Result<f64, Self::Error> {
            assert_eq!(*b, 7);
            Ok(1.)
        }
        fn bounding_box_max_y(&mut self, b: &u8) -> Result<f64, Self::Error> {
            assert_eq!(*b, 7);
            Ok(3.)
        }
    }
}
