//! Original BodyRotationControl (`bzk`, 16–75). Mob.tickHeadTurn calls this
//! method on the server too, despite the original name `clientTick`.
use super::{clamp, rotate_if_necessary, OrientationHost, PositionHost};

#[derive(Debug, Default)]
pub struct BodyRotationState {
    head_stable_time: i32,
    last_stable_head_yaw: f32,
}
impl BodyRotationState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn head_stable_time(&self) -> i32 {
        self.head_stable_time
    }
    pub fn last_stable_head_yaw(&self) -> f32 {
        self.last_stable_head_yaw
    }
}
pub trait BodyRotationHost: OrientationHost + PositionHost {
    type BodyLease;
    fn with_body_state<R>(
        &mut self,
        retained: &Self::BodyLease,
        access: impl FnOnce(&mut BodyRotationState) -> R,
    ) -> Result<R, Self::Error>;
    /// Callback-free reads of Entity.xo/zo (bsr.L/N), not xOld/zOld.
    fn xo(&mut self) -> Result<f64, Self::Error>;
    fn zo(&mut self) -> Result<f64, Self::Error>;
    /// Actual getFirstPassenger() followed by instanceof Mob, retaining that
    /// returned passenger across the type check; empty returns false.
    fn first_passenger_is_mob(&mut self) -> Result<bool, Self::Error>;
}

fn rotate_body<H: BodyRotationHost>(host: &mut H) -> Result<(), H::Error> {
    let body = host.body_yaw()?;
    let head = host.head_yaw()?;
    let maximum = host.max_head_yaw()? as f32;
    host.set_body_yaw(rotate_if_necessary(body, head, maximum))
}
fn rotate_head<H: BodyRotationHost>(host: &mut H) -> Result<(), H::Error> {
    let head = host.head_yaw()?;
    let body = host.body_yaw()?;
    let maximum = host.max_head_yaw()? as f32;
    host.set_head_yaw(rotate_if_necessary(head, body, maximum))
}

pub fn client_tick<H: BodyRotationHost>(
    retained: &H::BodyLease,
    host: &mut H,
) -> Result<(), H::Error> {
    let dx = host.x()? - host.xo()?;
    let dz = host.z()? - host.zo()?;
    if dx * dx + dz * dz > 2.500000277905201E-7 {
        let yaw = host.yaw()?;
        host.set_body_yaw(yaw)?;
        rotate_head(host)?;
        let head = host.head_yaw()?;
        host.with_body_state(retained, |state| {
            state.last_stable_head_yaw = head;
            state.head_stable_time = 0;
        })?;
        return Ok(());
    }
    if !host.first_passenger_is_mob()? {
        let head = host.head_yaw()?;
        let last = host.with_body_state(retained, |state| state.last_stable_head_yaw)?;
        if (head - last).abs() > 15.0 {
            host.with_body_state(retained, |state| state.head_stable_time = 0)?;
            let head = host.head_yaw()?;
            host.with_body_state(retained, |state| state.last_stable_head_yaw = head)?;
            rotate_body(host)?;
        } else {
            let time = host.with_body_state(retained, |state| {
                state.head_stable_time = state.head_stable_time.wrapping_add(1);
                state.head_stable_time
            })?;
            if time > 10 {
                let delay = host
                    .with_body_state(retained, |state| state.head_stable_time.wrapping_sub(10))?;
                let progress = clamp(delay as f32 / 10.0, 0.0, 1.0);
                let maximum = host.max_head_yaw()? as f32 * (1.0 - progress);
                let body = host.body_yaw()?;
                let head = host.head_yaw()?;
                host.set_body_yaw(rotate_if_necessary(body, head, maximum))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::ControlHost;
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    struct Host {
        head: f32,
        body: f32,
        yaw: f32,
        x: f64,
        xo: f64,
        passenger: bool,
        lease: Rc<RefCell<BodyRotationState>>,
        reenter: bool,
    }
    impl ControlHost for Host {
        type Error = ();
    }
    impl PositionHost for Host {
        fn x(&mut self) -> Result<f64, ()> {
            Ok(self.x)
        }
        fn y(&mut self) -> Result<f64, ()> {
            Ok(0.0)
        }
        fn z(&mut self) -> Result<f64, ()> {
            Ok(0.0)
        }
    }
    impl OrientationHost for Host {
        fn yaw(&mut self) -> Result<f32, ()> {
            Ok(self.yaw)
        }
        fn pitch(&mut self) -> Result<f32, ()> {
            Ok(0.0)
        }
        fn set_yaw(&mut self, v: f32) -> Result<(), ()> {
            self.yaw = v;
            Ok(())
        }
        fn set_pitch(&mut self, _: f32) -> Result<(), ()> {
            Ok(())
        }
        fn head_yaw(&mut self) -> Result<f32, ()> {
            Ok(self.head)
        }
        fn body_yaw(&mut self) -> Result<f32, ()> {
            Ok(self.body)
        }
        fn set_head_yaw(&mut self, v: f32) -> Result<(), ()> {
            self.head = v;
            Ok(())
        }
        fn set_body_yaw(&mut self, v: f32) -> Result<(), ()> {
            self.body = v;
            Ok(())
        }
        fn max_head_yaw(&mut self) -> Result<i32, ()> {
            if self.reenter {
                self.reenter = false;
                let lease = self.lease.clone();
                client_tick(&lease, self)?;
            }
            Ok(75)
        }
    }
    impl BodyRotationHost for Host {
        type BodyLease = Rc<RefCell<BodyRotationState>>;
        fn with_body_state<R>(
            &mut self,
            l: &Self::BodyLease,
            f: impl FnOnce(&mut BodyRotationState) -> R,
        ) -> Result<R, ()> {
            Ok(f(&mut l.borrow_mut()))
        }
        fn xo(&mut self) -> Result<f64, ()> {
            Ok(self.xo)
        }
        fn zo(&mut self) -> Result<f64, ()> {
            Ok(0.0)
        }
        fn first_passenger_is_mob(&mut self) -> Result<bool, ()> {
            Ok(self.passenger)
        }
    }
    #[test]
    fn original_old_position_strict_delay_wrap_and_reentry() {
        let lease = Rc::new(RefCell::new(BodyRotationState::new()));
        let mut h = Host {
            head: 0.,
            body: 90.,
            yaw: 40.,
            x: 0.,
            xo: 0.,
            passenger: false,
            lease: lease.clone(),
            reenter: false,
        };
        for _ in 0..10 {
            client_tick(&lease, &mut h).unwrap();
        }
        assert_eq!(h.body, 90.);
        client_tick(&lease, &mut h).unwrap();
        assert_eq!(h.body, 67.5);
        lease.borrow_mut().head_stable_time = i32::MAX;
        client_tick(&lease, &mut h).unwrap();
        assert_eq!(lease.borrow().head_stable_time(), i32::MIN);
        h.xo = -1.;
        h.reenter = true;
        client_tick(&lease, &mut h).unwrap();
        assert_eq!(h.body, 40.);
        assert_eq!(lease.borrow().head_stable_time(), 0);
        assert_eq!(lease.borrow().last_stable_head_yaw(), h.head);
    }
}
