//! Source-ordered Java 1.21 `Mob.serverAiStep` (`btp.fm`, 780–825).
//!
//! This dispatch owns no actor, selector, navigation, RNG or scheduler. Its
//! mandatory host operates on the actual mob's existing owners. In particular,
//! empty Bat goals do not authorize skipping navigation or control callbacks.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileEvent {
    Push(&'static str),
    PopPush(&'static str),
    Pop,
}

pub trait MobAiStepHost {
    type Error;
    /// A retained handle to the exact level profiler acquired at this call.
    type Profiler;

    fn no_action_time(&mut self) -> Result<i32, Self::Error>;
    fn set_no_action_time(&mut self, value: i32) -> Result<(), Self::Error>;
    fn level_profiler(&mut self) -> Result<Self::Profiler, Self::Error>;
    fn profile(
        &mut self,
        profiler: &mut Self::Profiler,
        event: ProfileEvent,
    ) -> Result<(), Self::Error>;
    fn sensing_tick(&mut self) -> Result<(), Self::Error>;
    fn actor_tick_count(&mut self) -> Result<i32, Self::Error>;
    fn actor_entity_id(&mut self) -> Result<i32, Self::Error>;
    fn target_selector_tick(&mut self) -> Result<(), Self::Error>;
    fn goal_selector_tick(&mut self) -> Result<(), Self::Error>;
    fn target_selector_tick_running_goals(&mut self, tick_all: bool) -> Result<(), Self::Error>;
    fn goal_selector_tick_running_goals(&mut self, tick_all: bool) -> Result<(), Self::Error>;
    fn navigation_tick(&mut self) -> Result<(), Self::Error>;
    fn custom_server_ai_step(&mut self) -> Result<(), Self::Error>;
    fn move_control_tick(&mut self) -> Result<(), Self::Error>;
    fn look_control_tick(&mut self) -> Result<(), Self::Error>;
    fn jump_control_tick(&mut self) -> Result<(), Self::Error>;
    fn send_debug_packets(&mut self) -> Result<(), Self::Error>;
}

/// Invoked by the actual LivingEntity.aiStep effective-AI branch, before jump
/// and travel. The inherited caller owns NoAI/immobility/side admission. Errors
/// stop at the failed callback without rolling back the executed prefix or
/// synthesizing profiler pops that Java does not perform in this method.
pub fn server_ai_step<H: MobAiStepHost>(host: &mut H) -> Result<(), H::Error> {
    let next_no_action_time = host.no_action_time()?.wrapping_add(1);
    host.set_no_action_time(next_no_action_time)?;
    let mut profiler = host.level_profiler()?;
    host.profile(&mut profiler, ProfileEvent::Push("sensing"))?;
    host.sensing_tick()?;
    host.profile(&mut profiler, ProfileEvent::Pop)?;

    let staggered_tick = host
        .actor_tick_count()?
        .wrapping_add(host.actor_entity_id()?);
    let full_tick = staggered_tick % 2 == 0 || host.actor_tick_count()? <= 1;
    host.profile(&mut profiler, ProfileEvent::Push("targetSelector"))?;
    if full_tick {
        host.target_selector_tick()?;
    } else {
        host.target_selector_tick_running_goals(false)?;
    }
    host.profile(&mut profiler, ProfileEvent::Pop)?;
    host.profile(&mut profiler, ProfileEvent::Push("goalSelector"))?;
    if full_tick {
        host.goal_selector_tick()?;
    } else {
        host.goal_selector_tick_running_goals(false)?;
    }
    host.profile(&mut profiler, ProfileEvent::Pop)?;
    host.profile(&mut profiler, ProfileEvent::Push("navigation"))?;
    host.navigation_tick()?;
    host.profile(&mut profiler, ProfileEvent::Pop)?;
    host.profile(&mut profiler, ProfileEvent::Push("mob tick"))?;
    host.custom_server_ai_step()?;
    host.profile(&mut profiler, ProfileEvent::Pop)?;
    host.profile(&mut profiler, ProfileEvent::Push("controls"))?;
    host.profile(&mut profiler, ProfileEvent::Push("move"))?;
    host.move_control_tick()?;
    host.profile(&mut profiler, ProfileEvent::PopPush("look"))?;
    host.look_control_tick()?;
    host.profile(&mut profiler, ProfileEvent::PopPush("jump"))?;
    host.jump_control_tick()?;
    host.profile(&mut profiler, ProfileEvent::Pop)?;
    host.profile(&mut profiler, ProfileEvent::Pop)?;
    host.send_debug_packets()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Host {
        tick: i32,
        id: i32,
        idle: i32,
        trace: Vec<String>,
        fail_navigation: bool,
    }
    impl Host {
        fn record(&mut self, event: &str) -> Result<(), &'static str> {
            self.trace.push(event.into());
            Ok(())
        }
    }
    impl MobAiStepHost for Host {
        type Error = &'static str;
        type Profiler = usize;
        fn no_action_time(&mut self) -> Result<i32, Self::Error> {
            Ok(self.idle)
        }
        fn set_no_action_time(&mut self, n: i32) -> Result<(), Self::Error> {
            self.idle = n;
            self.record("idle")
        }
        fn level_profiler(&mut self) -> Result<usize, Self::Error> {
            self.record("profiler")?;
            Ok(42)
        }
        fn profile(&mut self, p: &mut usize, e: ProfileEvent) -> Result<(), Self::Error> {
            assert_eq!(*p, 42);
            self.trace.push(format!("{e:?}"));
            Ok(())
        }
        fn sensing_tick(&mut self) -> Result<(), Self::Error> {
            self.record("sensing")
        }
        fn actor_tick_count(&mut self) -> Result<i32, Self::Error> {
            self.record("tick")?;
            Ok(self.tick)
        }
        fn actor_entity_id(&mut self) -> Result<i32, Self::Error> {
            self.record("id")?;
            Ok(self.id)
        }
        fn target_selector_tick(&mut self) -> Result<(), Self::Error> {
            self.record("target-full")
        }
        fn goal_selector_tick(&mut self) -> Result<(), Self::Error> {
            self.record("goal-full")
        }
        fn target_selector_tick_running_goals(&mut self, all: bool) -> Result<(), Self::Error> {
            assert!(!all);
            self.record("target-running")
        }
        fn goal_selector_tick_running_goals(&mut self, all: bool) -> Result<(), Self::Error> {
            assert!(!all);
            self.record("goal-running")
        }
        fn navigation_tick(&mut self) -> Result<(), Self::Error> {
            self.record("navigation")?;
            if self.fail_navigation {
                Err("navigation unavailable")
            } else {
                Ok(())
            }
        }
        fn custom_server_ai_step(&mut self) -> Result<(), Self::Error> {
            self.record("custom")
        }
        fn move_control_tick(&mut self) -> Result<(), Self::Error> {
            self.record("move")
        }
        fn look_control_tick(&mut self) -> Result<(), Self::Error> {
            self.record("look")
        }
        fn jump_control_tick(&mut self) -> Result<(), Self::Error> {
            self.record("jump")
        }
        fn send_debug_packets(&mut self) -> Result<(), Self::Error> {
            self.record("debug")
        }
    }
    #[test]
    fn actor_identity_cadence_and_source_phase_order() {
        let mut h = Host {
            tick: 8,
            id: 3,
            idle: i32::MAX,
            trace: vec![],
            fail_navigation: false,
        };
        server_ai_step(&mut h).unwrap();
        assert_eq!(h.idle, i32::MIN);
        assert_eq!(
            h.trace,
            [
                "idle",
                "profiler",
                "Push(\"sensing\")",
                "sensing",
                "Pop",
                "tick",
                "id",
                "tick",
                "Push(\"targetSelector\")",
                "target-running",
                "Pop",
                "Push(\"goalSelector\")",
                "goal-running",
                "Pop",
                "Push(\"navigation\")",
                "navigation",
                "Pop",
                "Push(\"mob tick\")",
                "custom",
                "Pop",
                "Push(\"controls\")",
                "Push(\"move\")",
                "move",
                "PopPush(\"look\")",
                "look",
                "PopPush(\"jump\")",
                "jump",
                "Pop",
                "Pop",
                "debug"
            ]
        );
        h.trace.clear();
        h.tick = 1;
        server_ai_step(&mut h).unwrap();
        assert!(h.trace.iter().any(|e| e == "target-full"));
        assert_eq!(h.trace.iter().filter(|e| *e == "tick").count(), 1); // even branch skips second field read
        h.trace.clear();
        h.tick = i32::MAX;
        h.id = 2;
        server_ai_step(&mut h).unwrap();
        assert!(h.trace.iter().any(|e| e == "target-running")); // wrapping sum is odd, actor tick is still positive
        h.trace.clear();
        h.tick = -3;
        h.id = 2;
        server_ai_step(&mut h).unwrap();
        assert!(h.trace.iter().any(|e| e == "target-full")); // actor tick<=1 branch
    }
    #[test]
    fn failure_preserves_prefix_without_fabricated_profiler_cleanup() {
        let mut h = Host {
            tick: 2,
            id: 2,
            idle: 5,
            trace: vec![],
            fail_navigation: true,
        };
        assert_eq!(server_ai_step(&mut h), Err("navigation unavailable"));
        assert_eq!(h.idle, 6);
        assert_eq!(
            &h.trace[h.trace.len() - 2..],
            ["Push(\"navigation\")", "navigation"]
        );
        assert!(!h
            .trace
            .iter()
            .any(|e| e == "custom" || e == "move" || e == "debug"));
    }
}
