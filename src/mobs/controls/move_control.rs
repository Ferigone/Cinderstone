//! Original ordinary MoveControl (`bzp`, 28–163). Bat inherits this Ground
//! navigation control, not FlyingMoveControl. WAIT still clears forward input.
use super::super::bat::{java_floor, mth_atan2, wrap_degrees};
use super::{java_max, OrientationHost, PositionHost};
use crate::geometry::{vanilla_cos, vanilla_sin};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Wait,
    MoveTo,
    Strafe,
    Jumping,
}
#[derive(Debug)]
pub struct MoveControlState {
    wanted_x: f64,
    wanted_y: f64,
    wanted_z: f64,
    speed_modifier: f64,
    strafe_forwards: f32,
    strafe_right: f32,
    operation: Operation,
}
impl Default for MoveControlState {
    fn default() -> Self {
        Self::new()
    }
}
impl MoveControlState {
    pub fn new() -> Self {
        Self {
            wanted_x: 0.0,
            wanted_y: 0.0,
            wanted_z: 0.0,
            speed_modifier: 0.0,
            strafe_forwards: 0.0,
            strafe_right: 0.0,
            operation: Operation::Wait,
        }
    }
    pub fn has_wanted(&self) -> bool {
        self.operation == Operation::MoveTo
    }
    pub fn speed_modifier(&self) -> f64 {
        self.speed_modifier
    }
    pub fn wanted_x(&self) -> f64 {
        self.wanted_x
    }
    pub fn wanted_y(&self) -> f64 {
        self.wanted_y
    }
    pub fn wanted_z(&self) -> f64 {
        self.wanted_z
    }
    pub fn operation(&self) -> Operation {
        self.operation
    }
    pub fn set_wanted_position(&mut self, x: f64, y: f64, z: f64, speed: f64) {
        self.wanted_x = x;
        self.wanted_y = y;
        self.wanted_z = z;
        self.speed_modifier = speed;
        if self.operation != Operation::Jumping {
            self.operation = Operation::MoveTo;
        }
    }
    pub fn strafe(&mut self, forward: f32, right: f32) {
        self.operation = Operation::Strafe;
        self.strafe_forwards = forward;
        self.strafe_right = right;
        self.speed_modifier = 0.25;
    }
}
pub trait MoveControlHost: PositionHost + OrientationHost {
    type MoveLease;
    type Level;
    type BlockState;
    type CollisionShape;
    type Navigation;
    type NodeEvaluator;
    type JumpControl;
    fn with_move_state<R>(
        &mut self,
        retained: &Self::MoveLease,
        access: impl FnOnce(&mut MoveControlState) -> R,
    ) -> Result<R, Self::Error>;
    /// Actual Attributes.MOVEMENT_SPEED value, with real attribute callbacks.
    fn movement_speed_attribute(&mut self) -> Result<f64, Self::Error>;
    fn set_speed(&mut self, value: f32) -> Result<(), Self::Error>;
    fn set_forward_input(&mut self, value: f32) -> Result<(), Self::Error>;
    fn set_strafe_input(&mut self, value: f32) -> Result<(), Self::Error>;
    fn on_ground(&mut self) -> Result<bool, Self::Error>;
    fn max_up_step(&mut self) -> Result<f32, Self::Error>;
    /// Callback-free FINAL Entity.getBbWidth accessor.
    fn bounding_box_width(&mut self) -> Result<f32, Self::Error>;
    fn block_position(&mut self) -> Result<(i32, i32, i32), Self::Error>;
    /// Callback-free FINAL Entity.getBlockY accessor.
    fn block_y(&mut self) -> Result<i32, Self::Error>;
    /// Every returned reference-like value retains the ORIGINAL object. Later
    /// actor/world reentry must not retarget it to a replacement owner.
    fn level(&mut self) -> Result<Self::Level, Self::Error>;
    fn block_state(
        &mut self,
        level: &Self::Level,
        at: (i32, i32, i32),
    ) -> Result<Self::BlockState, Self::Error>;
    fn collision_shape(
        &mut self,
        state: &Self::BlockState,
        level: &Self::Level,
        at: (i32, i32, i32),
    ) -> Result<Self::CollisionShape, Self::Error>;
    fn shape_is_empty(&mut self, shape: &Self::CollisionShape) -> Result<bool, Self::Error>;
    fn shape_max_y(&mut self, shape: &Self::CollisionShape) -> Result<f64, Self::Error>;
    fn state_is_door(&mut self, state: &Self::BlockState) -> Result<bool, Self::Error>;
    fn state_is_fence(&mut self, state: &Self::BlockState) -> Result<bool, Self::Error>;
    /// Source isWalkable admits null navigation/evaluator; ordinary Mob's
    /// constructor supplies real GroundPathNavigation and WalkNodeEvaluator.
    /// None is only legitimate when the actual source field/getter is null.
    fn navigation(&mut self) -> Result<Option<Self::Navigation>, Self::Error>;
    fn node_evaluator(
        &mut self,
        navigation: &Self::Navigation,
    ) -> Result<Option<Self::NodeEvaluator>, Self::Error>;
    fn path_type_is_walkable(
        &mut self,
        evaluator: &Self::NodeEvaluator,
        at: (i32, i32, i32),
    ) -> Result<bool, Self::Error>;
    fn jump_control(&mut self) -> Result<Self::JumpControl, Self::Error>;
    fn request_jump(&mut self, jump: &Self::JumpControl) -> Result<(), Self::Error>;
}

pub fn set_wanted_position<H: MoveControlHost>(
    retained: &H::MoveLease,
    host: &mut H,
    position: [f64; 3],
    speed: f64,
) -> Result<(), H::Error> {
    host.with_move_state(retained, |s| {
        s.set_wanted_position(position[0], position[1], position[2], speed)
    })
}
pub fn strafe<H: MoveControlHost>(
    retained: &H::MoveLease,
    host: &mut H,
    forward: f32,
    right: f32,
) -> Result<(), H::Error> {
    host.with_move_state(retained, |s| s.strafe(forward, right))
}
fn is_walkable<H: MoveControlHost>(host: &mut H, x: f32, z: f32) -> Result<bool, H::Error> {
    if let Some(navigation) = host.navigation()? {
        if let Some(evaluator) = host.node_evaluator(&navigation)? {
            let x = host.x()? + f64::from(x);
            let y = f64::from(host.block_y()?);
            let z = host.z()? + f64::from(z);
            let at = (java_floor(x), java_floor(y), java_floor(z));
            return host.path_type_is_walkable(&evaluator, at);
        }
    }
    Ok(true)
}
/// Source MoveControl.rotlerp normalizes at most once and leaves exactly360.
pub fn rotlerp(current: f32, target: f32, maximum: f32) -> f32 {
    let mut difference = wrap_degrees(target - current);
    if difference > maximum {
        difference = maximum;
    }
    if difference < -maximum {
        difference = -maximum;
    }
    let mut result = current + difference;
    if result < 0.0 {
        result += 360.0;
    } else if result > 360.0 {
        result -= 360.0;
    }
    result
}

pub fn tick<H: MoveControlHost>(retained: &H::MoveLease, host: &mut H) -> Result<(), H::Error> {
    // Branch decisions are source field reads. A callback can change the
    // operation before the NEXT else-if, but never reselects an entered branch.
    if host.with_move_state(retained, |s| s.operation == Operation::Strafe)? {
        let attribute = host.movement_speed_attribute()? as f32;
        let speed = host.with_move_state(retained, |s| s.speed_modifier as f32)? * attribute;
        let forward = host.with_move_state(retained, |s| s.strafe_forwards)?;
        let right = host.with_move_state(retained, |s| s.strafe_right)?;
        let mut length = f64::from(forward * forward + right * right).sqrt() as f32;
        if length < 1.0 {
            length = 1.0;
        }
        let scale = speed / length;
        let forward = forward * scale;
        let right = right * scale;
        let sin = vanilla_sin(host.yaw()? * 0.017453292_f32);
        let cos = vanilla_cos(host.yaw()? * 0.017453292_f32);
        let x = forward * cos - right * sin;
        let z = right * cos + forward * sin;
        if !is_walkable(host, x, z)? {
            host.with_move_state(retained, |s| {
                s.strafe_forwards = 1.0;
                s.strafe_right = 0.0;
            })?;
        }
        host.set_speed(speed)?;
        let forward = host.with_move_state(retained, |s| s.strafe_forwards)?;
        host.set_forward_input(forward)?;
        let right = host.with_move_state(retained, |s| s.strafe_right)?;
        host.set_strafe_input(right)?;
        host.with_move_state(retained, |s| s.operation = Operation::Wait)?;
    } else if host.with_move_state(retained, |s| s.operation == Operation::MoveTo)? {
        host.with_move_state(retained, |s| s.operation = Operation::Wait)?;
        let wanted = host.with_move_state(retained, |s| s.wanted_x)?;
        let dx = wanted - host.x()?;
        let wanted = host.with_move_state(retained, |s| s.wanted_z)?;
        let dz = wanted - host.z()?;
        let wanted = host.with_move_state(retained, |s| s.wanted_y)?;
        let dy = wanted - host.y()?;
        let distance = dx * dx + dy * dy + dz * dz;
        if distance < 2.500000277905201E-7 {
            host.set_forward_input(0.0)?;
            return Ok(());
        }
        let wanted_yaw = (mth_atan2(dz, dx) * 57.2957763671875) as f32 - 90.0;
        let yaw = host.yaw()?;
        host.set_yaw(rotlerp(yaw, wanted_yaw, 90.0))?;
        // speedModifier is loaded BEFORE the attribute virtual call.
        let modifier = host.with_move_state(retained, |s| s.speed_modifier)?;
        let attribute = host.movement_speed_attribute()?;
        host.set_speed((modifier * attribute) as f32)?;
        let at = host.block_position()?;
        let level = host.level()?;
        let state = host.block_state(&level, at)?;
        let shape_level = host.level()?;
        let shape = host.collision_shape(&state, &shape_level, at)?;
        let height = host.max_up_step()?;
        let first_jump = if dy > f64::from(height) {
            let width = host.bounding_box_width()?;
            dx * dx + dz * dz < f64::from(java_max(1.0, width))
        } else {
            false
        };
        let jump = if first_jump {
            true
        } else if host.shape_is_empty(&shape)? {
            false
        } else {
            let y = host.y()?;
            let top = host.shape_max_y(&shape)? + f64::from(at.1);
            y < top && !host.state_is_door(&state)? && !host.state_is_fence(&state)?
        };
        if jump {
            let jump = host.jump_control()?;
            host.request_jump(&jump)?;
            host.with_move_state(retained, |s| s.operation = Operation::Jumping)?;
        }
    } else if host.with_move_state(retained, |s| s.operation == Operation::Jumping)? {
        let modifier = host.with_move_state(retained, |s| s.speed_modifier)?;
        let attribute = host.movement_speed_attribute()?;
        host.set_speed((modifier * attribute) as f32)?;
        if host.on_ground()? {
            host.with_move_state(retained, |s| s.operation = Operation::Wait)?;
        }
    } else {
        host.set_forward_input(0.0)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::test_support::Host;
    use super::*;
    #[test]
    fn actual_wait_clears_bat_forward_and_close_move_returns_after_wait() {
        let mut h = Host::default();
        let lease = h.moves.clone();
        tick(&lease, &mut h).unwrap();
        assert_eq!(h.forward, 0.0);
        assert_eq!(h.events, vec!["set_forward"]);
        h.events.clear();
        set_wanted_position(&lease, &mut h, [0.; 3], 1.).unwrap();
        tick(&lease, &mut h).unwrap();
        assert_eq!(lease.borrow().operation(), Operation::Wait);
        assert_eq!(h.events, vec!["x", "z", "y", "set_forward"]);
    }
    #[test]
    fn strafe_real_path_query_then_current_fields_after_reentry() {
        let mut h = Host::default();
        let lease = h.moves.clone();
        strafe(&lease, &mut h, 2., 3.).unwrap();
        h.walkable = false;
        tick(&lease, &mut h).unwrap();
        assert_eq!((h.forward, h.right), (1., 0.));
        assert_eq!(h.speed.to_bits(), (0.25_f32 * 0.7_f32).to_bits());
        assert_eq!(
            h.events,
            vec![
                "attribute",
                "yaw",
                "yaw",
                "navigation",
                "evaluator",
                "x",
                "block_y",
                "z",
                "path_type",
                "set_speed",
                "set_forward",
                "set_strafe"
            ]
        );
        strafe(&lease, &mut h, 2., 3.).unwrap();
        h.walkable = true;
        h.hook = Some(("set_speed", |h| {
            let lease = h.moves.clone();
            strafe(&lease, h, 4., 5.)
        }));
        tick(&lease, &mut h).unwrap();
        assert_eq!((h.forward, h.right), (4., 5.));
        assert_eq!(lease.borrow().operation(), Operation::Wait);
    }
    #[test]
    fn move_prefix_reference_order_jump_reentry_and_jumping_setter() {
        let mut h = Host::default();
        let lease = h.moves.clone();
        set_wanted_position(&lease, &mut h, [0., 1., 0.], 2.).unwrap();
        h.hook = Some(("request_jump", |h| {
            let lease = h.moves.clone();
            set_wanted_position(&lease, h, [8.; 3], 9.)
        }));
        tick(&lease, &mut h).unwrap();
        assert!(h.jumps.borrow().requested());
        assert_eq!(lease.borrow().operation(), Operation::Jumping);
        assert_eq!(lease.borrow().wanted_x(), 8.);
        assert_eq!(
            h.events,
            vec![
                "x",
                "z",
                "y",
                "yaw",
                "set_yaw",
                "attribute",
                "set_speed",
                "block_position",
                "level",
                "block_state",
                "level",
                "shape",
                "max_up_step",
                "width",
                "jump_control",
                "request_jump"
            ]
        );
        set_wanted_position(&lease, &mut h, [3.; 3], 4.).unwrap();
        assert_eq!(lease.borrow().operation(), Operation::Jumping);
        h.grounded = true;
        h.events.clear();
        tick(&lease, &mut h).unwrap();
        assert_eq!(h.speed, (4. * 0.7) as f32);
        assert_eq!(lease.borrow().operation(), Operation::Wait);
        assert_eq!(h.events, vec!["attribute", "set_speed", "on_ground"]);
        set_wanted_position(&lease, &mut h, [0., 1., 0.], 2.).unwrap();
        h.fail = Some("request_jump");
        assert_eq!(tick(&lease, &mut h), Err("request_jump"));
        assert_eq!(lease.borrow().operation(), Operation::Wait);
    }
    #[test]
    fn modifier_load_before_attribute_and_state_borrow_released() {
        let mut h = Host::default();
        let lease = h.moves.clone();
        set_wanted_position(&lease, &mut h, [1., 0., 0.], 2.).unwrap();
        h.hook = Some(("attribute", |h| {
            h.moves.borrow_mut().set_wanted_position(9., 9., 9., 10.);
            Ok(())
        }));
        tick(&lease, &mut h).unwrap();
        assert_eq!(h.speed, (2. * 0.7) as f32);
        assert_eq!(lease.borrow().operation(), Operation::MoveTo);
        assert_eq!(lease.borrow().speed_modifier(), 10.);
        // NaN width does not satisfy the first jump clause (Java Math.max).
        set_wanted_position(&lease, &mut h, [0., 1., 0.], 1.).unwrap();
        h.width = f32::NAN;
        tick(&lease, &mut h).unwrap();
        assert_eq!(lease.borrow().operation(), Operation::Wait);
    }
}
