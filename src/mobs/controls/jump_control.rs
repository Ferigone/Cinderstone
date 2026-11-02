//! Original JumpControl (`bzn`, 9–20), with its own retained request field.
use super::ControlHost;

#[derive(Debug, Default)]
pub struct JumpControlState {
    jump: bool,
}
impl JumpControlState {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn jump(&mut self) {
        self.jump = true;
    }
    pub fn requested(&self) -> bool {
        self.jump
    }
}

pub trait JumpControlHost: ControlHost {
    type JumpLease;
    fn with_jump_state<R>(
        &mut self,
        retained: &Self::JumpLease,
        access: impl FnOnce(&mut JumpControlState) -> R,
    ) -> Result<R, Self::Error>;
    /// Actual LivingEntity.setJumping; a normal callback may request another
    /// jump on this same control. The outer tick clears it AFTER that callback.
    fn set_jumping(&mut self, jumping: bool) -> Result<(), Self::Error>;
}

pub fn jump<H: JumpControlHost>(retained: &H::JumpLease, host: &mut H) -> Result<(), H::Error> {
    host.with_jump_state(retained, JumpControlState::jump)
}
pub fn tick<H: JumpControlHost>(retained: &H::JumpLease, host: &mut H) -> Result<(), H::Error> {
    let requested = host.with_jump_state(retained, |state| state.jump)?;
    host.set_jumping(requested)?;
    host.with_jump_state(retained, |state| state.jump = false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    struct Host {
        state: Rc<RefCell<JumpControlState>>,
        calls: Vec<bool>,
        reenter: bool,
        fail: bool,
    }
    impl ControlHost for Host {
        type Error = &'static str;
    }
    impl JumpControlHost for Host {
        type JumpLease = Rc<RefCell<JumpControlState>>;
        fn with_jump_state<R>(
            &mut self,
            retained: &Self::JumpLease,
            access: impl FnOnce(&mut JumpControlState) -> R,
        ) -> Result<R, Self::Error> {
            Ok(access(&mut retained.borrow_mut()))
        }
        fn set_jumping(&mut self, requested: bool) -> Result<(), Self::Error> {
            self.calls.push(requested);
            // No control borrow may be held here, including in the default case.
            if self.reenter {
                let lease = self.state.clone();
                jump(&lease, self)?;
            }
            if self.fail {
                Err("setter")
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn default_write_reentry_and_failure_prefix() {
        let lease = Rc::new(RefCell::new(JumpControlState::new()));
        let mut host = Host {
            state: lease.clone(),
            calls: vec![],
            reenter: true,
            fail: false,
        };
        tick(&lease, &mut host).unwrap();
        assert_eq!(host.calls, vec![false]);
        assert!(!lease.borrow().requested());
        jump(&lease, &mut host).unwrap();
        host.fail = true;
        assert_eq!(tick(&lease, &mut host), Err("setter"));
        assert!(lease.borrow().requested());
    }
}
