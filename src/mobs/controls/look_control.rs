//! Original ordinary LookControl (`bzo`, 20–117). Derived protected overrides
//! in other control classes require their own dispatch, not this base tick.
use super::super::bat::{mth_atan2, wrap_degrees};
use super::{clamp, rotate_if_necessary, OrientationHost, PositionHost};

#[derive(Debug, Default)]
pub struct LookControlState {
    y_max_rot_speed: f32,
    x_max_rot_angle: f32,
    look_at_cooldown: i32,
    wanted_x: f64,
    wanted_y: f64,
    wanted_z: f64,
}
impl LookControlState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_look_at(&mut self, x: f64, y: f64, z: f64, yaw_speed: f32, pitch_angle: f32) {
        self.wanted_x = x;
        self.wanted_y = y;
        self.wanted_z = z;
        self.y_max_rot_speed = yaw_speed;
        self.x_max_rot_angle = pitch_angle;
        self.look_at_cooldown = 2;
    }
    pub fn is_looking_at_target(&self) -> bool {
        self.look_at_cooldown > 0
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
    pub fn look_at_cooldown(&self) -> i32 {
        self.look_at_cooldown
    }
}
pub trait LookControlHost: PositionHost + OrientationHost {
    type LookLease;
    type Navigation;
    /// A retained original target Entity, not a numeric-ID resolver.
    type Target;
    type BoundingBox;
    fn with_look_state<R>(
        &mut self,
        retained: &Self::LookLease,
        access: impl FnOnce(&mut LookControlState) -> R,
    ) -> Result<R, Self::Error>;
    fn eye_y(&mut self) -> Result<f64, Self::Error>;
    fn head_rotation_speed(&mut self) -> Result<i32, Self::Error>;
    fn max_head_pitch(&mut self) -> Result<i32, Self::Error>;
    fn navigation(&mut self) -> Result<Self::Navigation, Self::Error>;
    fn navigation_is_done(&mut self, navigation: &Self::Navigation) -> Result<bool, Self::Error>;
    /// Callback-free FINAL Entity.getX/getZ reads on the retained target.
    fn target_x(&mut self, target: &Self::Target) -> Result<f64, Self::Error>;
    fn target_z(&mut self, target: &Self::Target) -> Result<f64, Self::Error>;
    /// Callback-free original instanceof LivingEntity.
    fn target_is_living(&mut self, target: &Self::Target) -> Result<bool, Self::Error>;
    fn target_eye_y(&mut self, target: &Self::Target) -> Result<f64, Self::Error>;
    /// Callback-free FINAL Entity.getBoundingBox accessor. The source invokes
    /// it twice for a nonliving target, loading minY before the second access.
    fn target_bounding_box(
        &mut self,
        target: &Self::Target,
    ) -> Result<Self::BoundingBox, Self::Error>;
    /// Callback-free original AABB minY/maxY field reads.
    fn bounding_box_min_y(&mut self, bounds: &Self::BoundingBox) -> Result<f64, Self::Error>;
    fn bounding_box_max_y(&mut self, bounds: &Self::BoundingBox) -> Result<f64, Self::Error>;
}

pub fn set_look_at_with_limits<H: LookControlHost>(
    retained: &H::LookLease,
    host: &mut H,
    x: f64,
    y: f64,
    z: f64,
    yaw_speed: f32,
    pitch_angle: f32,
) -> Result<(), H::Error> {
    host.with_look_state(retained, |s| s.set_look_at(x, y, z, yaw_speed, pitch_angle))
}
pub fn set_look_at<H: LookControlHost>(
    retained: &H::LookLease,
    host: &mut H,
    position: [f64; 3],
) -> Result<(), H::Error> {
    let yaw_speed = host.head_rotation_speed()? as f32;
    let pitch_angle = host.max_head_pitch()? as f32;
    set_look_at_with_limits(
        retained,
        host,
        position[0],
        position[1],
        position[2],
        yaw_speed,
        pitch_angle,
    )
}
fn entity_coordinates<H: LookControlHost>(
    target: &H::Target,
    host: &mut H,
) -> Result<[f64; 3], H::Error> {
    let x = host.target_x(target)?;
    let y = if host.target_is_living(target)? {
        host.target_eye_y(target)?
    } else {
        let lower = host.target_bounding_box(target)?;
        let min = host.bounding_box_min_y(&lower)?;
        let upper = host.target_bounding_box(target)?;
        (min + host.bounding_box_max_y(&upper)?) / 2.0
    };
    let z = host.target_z(target)?;
    Ok([x, y, z])
}
pub fn set_look_at_entity<H: LookControlHost>(
    retained: &H::LookLease,
    target: &H::Target,
    host: &mut H,
) -> Result<(), H::Error> {
    let position = entity_coordinates(target, host)?;
    set_look_at(retained, host, position)
}
pub fn set_look_at_entity_with_limits<H: LookControlHost>(
    retained: &H::LookLease,
    target: &H::Target,
    host: &mut H,
    yaw_speed: f32,
    pitch_angle: f32,
) -> Result<(), H::Error> {
    let [x, y, z] = entity_coordinates(target, host)?;
    set_look_at_with_limits(retained, host, x, y, z, yaw_speed, pitch_angle)
}

pub fn rotate_towards(current: f32, target: f32, maximum: f32) -> f32 {
    current + clamp(wrap_degrees(target - current), -maximum, maximum)
}
fn y_rotation<H: LookControlHost>(
    retained: &H::LookLease,
    host: &mut H,
) -> Result<Option<f32>, H::Error> {
    let wanted = host.with_look_state(retained, |s| s.wanted_x)?;
    let dx = wanted - host.x()?;
    let wanted = host.with_look_state(retained, |s| s.wanted_z)?;
    let dz = wanted - host.z()?;
    Ok(
        if dz.abs() > 9.999999747378752E-6 || dx.abs() > 9.999999747378752E-6 {
            Some((mth_atan2(dz, dx) * 57.2957763671875) as f32 - 90.0)
        } else {
            None
        },
    )
}
fn x_rotation<H: LookControlHost>(
    retained: &H::LookLease,
    host: &mut H,
) -> Result<Option<f32>, H::Error> {
    let wanted = host.with_look_state(retained, |s| s.wanted_x)?;
    let dx = wanted - host.x()?;
    let wanted = host.with_look_state(retained, |s| s.wanted_y)?;
    let dy = wanted - host.eye_y()?;
    let wanted = host.with_look_state(retained, |s| s.wanted_z)?;
    let dz = wanted - host.z()?;
    let horizontal = (dx * dx + dz * dz).sqrt();
    Ok(
        if dy.abs() > 9.999999747378752E-6 || horizontal.abs() > 9.999999747378752E-6 {
            Some((-(mth_atan2(dy, horizontal) * 57.2957763671875)) as f32)
        } else {
            None
        },
    )
}

pub fn tick<H: LookControlHost>(retained: &H::LookLease, host: &mut H) -> Result<(), H::Error> {
    // Ordinary base resetXRotOnTick() is actually true, not a host no-op.
    host.set_pitch(0.0)?;
    if host.with_look_state(retained, |s| s.look_at_cooldown > 0)? {
        host.with_look_state(retained, |s| {
            s.look_at_cooldown = s.look_at_cooldown.wrapping_sub(1)
        })?;
        if let Some(target) = y_rotation(retained, host)? {
            let current = host.head_yaw()?;
            let maximum = host.with_look_state(retained, |s| s.y_max_rot_speed)?;
            host.set_head_yaw(rotate_towards(current, target, maximum))?;
        }
        if let Some(target) = x_rotation(retained, host)? {
            let current = host.pitch()?;
            let maximum = host.with_look_state(retained, |s| s.x_max_rot_angle)?;
            host.set_pitch(rotate_towards(current, target, maximum))?;
        }
    } else {
        let head = host.head_yaw()?;
        let body = host.body_yaw()?;
        host.set_head_yaw(rotate_towards(head, body, 10.0))?;
    }
    let navigation = host.navigation()?;
    if !host.navigation_is_done(&navigation)? {
        let head = host.head_yaw()?;
        let body = host.body_yaw()?;
        let maximum = host.max_head_yaw()? as f32;
        host.set_head_yaw(rotate_if_necessary(head, body, maximum))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::test_support::Host;
    use super::*;
    #[test]
    fn ordinary_default_pitch_head_and_actual_navigation_query() {
        let mut h = Host::default();
        let lease = h.looks.clone();
        tick(&lease, &mut h).unwrap();
        assert_eq!((h.pitch, h.head), (0.0, 10.0));
        assert_eq!(h.events, vec!["set_pitch", "navigation", "navigation_done"]);
        h.nav_done = false;
        h.body = 100.;
        h.head = 0.;
        tick(&lease, &mut h).unwrap();
        assert_eq!(h.head, 25.0); // source target-relative clamp after +10
    }
    #[test]
    fn same_control_nested_tick_and_failure_commits_decrement() {
        let mut h = Host::default();
        let lease = h.looks.clone();
        set_look_at_with_limits(&lease, &mut h, 1., 1., 0., 30., 40.).unwrap();
        h.hook = Some(("set_pitch", |h| {
            let lease = h.looks.clone();
            tick(&lease, h)
        }));
        tick(&lease, &mut h).unwrap();
        assert_eq!(lease.borrow().look_at_cooldown(), 0);
        assert_eq!(h.head, -60.);
        assert_eq!(h.pitch, -40.);
        set_look_at_with_limits(&lease, &mut h, 1., 1., 0., 30., 40.).unwrap();
        h.fail = Some("eye_y");
        assert_eq!(tick(&lease, &mut h), Err("eye_y"));
        assert_eq!(lease.borrow().look_at_cooldown(), 1);
        assert_eq!(h.pitch, 0.);
    }
    #[test]
    fn coordinates_reread_after_virtual_callback_and_real_entity_overloads() {
        let mut h = Host::default();
        let lease = h.looks.clone();
        set_look_at_entity(&lease, &false, &mut h).unwrap();
        assert_eq!(
            h.events,
            vec![
                "target_x",
                "target_bounds",
                "target_bounds",
                "target_z",
                "head_speed",
                "max_pitch"
            ]
        );
        assert_eq!(lease.borrow().wanted_y(), 2.);
        h.events.clear();
        set_look_at_with_limits(&lease, &mut h, 1., 1., 0., 90., 90.).unwrap();
        h.hook = Some(("eye_y", |h| {
            h.looks.borrow_mut().set_look_at(0., 2., 1., 90., 90.);
            Ok(())
        }));
        tick(&lease, &mut h).unwrap();
        assert_eq!(h.head, -90.);
        let expected = (-(mth_atan2(1., 2.0_f64.sqrt()) * 57.2957763671875)) as f32;
        assert_eq!(h.pitch.to_bits(), expected.to_bits());
        // Independent unchanged-original ControlOrderProbe eye-target-reread.
        assert_eq!(h.pitch.to_bits(), 0xc20d_0ec8);
        // The callback's full setLookAt resets cooldown after the outer decrement.
        assert_eq!(lease.borrow().look_at_cooldown(), 2);
    }
}
