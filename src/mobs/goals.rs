//! Java 1.21 ordinary GoalSelector/WrappedGoal source owners.
//!
//! The selected selector executor covers actual initialized empty collections,
//! including every profiler call. Nonempty traversal/arbitration and raw set
//! views are explicit unsupported domains, not successful empty callbacks.
//! No actor, goal implementation, scheduler, identity or RNG is created here.

use super::ai_step::ProfileEvent;

/// Source Goal.Flag60–64 / EnumSet ordinal order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum GoalFlag {
    Move = 0,
    Look = 1,
    Jump = 2,
    Target = 3,
}

/// Actual nonnull Goal.Flag EnumSet value domain. No arbitrary Java null/view
/// operations or replacement source goal flags are implied by this bitset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GoalFlags(u8);
impl GoalFlags {
    pub fn contains(self, flag: GoalFlag) -> bool {
        self.0 & (1 << flag as usize) != 0
    }
    pub fn insert(&mut self, flag: GoalFlag) {
        self.0 |= 1 << flag as usize;
    }
    pub fn remove(&mut self, flag: GoalFlag) {
        self.0 &= !(1 << flag as usize);
    }
}

/// Source WrappedGoal11–14, including Goal's inherited empty flag EnumSet.
/// G retains the actual supplied Goal; no invented AI callback is attached.
pub struct WrappedGoalState<G> {
    inherited_flags: GoalFlags,
    priority: i32,
    goal: G,
    running: bool,
}
impl<G> WrappedGoalState<G> {
    pub fn new(priority: i32, goal: G) -> Self {
        Self {
            inherited_flags: GoalFlags::default(),
            priority,
            goal,
            running: false,
        }
    }
    /// Source79,83,87 getters. Wrapper.getFlags75 delegates to its real Goal,
    /// so the unused inherited flag field must not substitute for that call.
    pub fn is_running(&self) -> bool {
        self.running
    }
    pub fn priority(&self) -> i32 {
        self.priority
    }
    pub fn goal(&self) -> &G {
        &self.goal
    }
    pub fn inherited_constructor_flags(&self) -> GoalFlags {
        self.inherited_flags
    }
}

pub trait WrappedGoalHost {
    type Error;
    type Lease;
    /// Clone retains the same actual Goal object and lifetime, not a snapshot.
    type Goal: Clone;
    fn with_wrapped_goal<R>(
        &mut self,
        retained: &Self::Lease,
        access: impl FnOnce(&mut WrappedGoalState<Self::Goal>) -> R,
    ) -> Result<R, Self::Error>;
    fn goal_start(&mut self, goal: &Self::Goal) -> Result<(), Self::Error>;
    fn goal_stop(&mut self, goal: &Self::Goal) -> Result<(), Self::Error>;
}

/// Source37–42: set isRunning BEFORE the actual Goal.start callback. Reentry
/// operates on this same retained wrapper with every state borrow released.
pub fn start_wrapped_goal<H: WrappedGoalHost>(
    retained: &H::Lease,
    host: &mut H,
) -> Result<(), H::Error> {
    let goal = host.with_wrapped_goal(retained, |state| {
        if state.running {
            None
        } else {
            state.running = true;
            Some(state.goal.clone())
        }
    })?;
    if let Some(goal) = goal {
        host.goal_start(&goal)?;
    }
    Ok(())
}

/// Source46–51: clear isRunning BEFORE actual Goal.stop; a reentrant start can
/// leave it true, and later callback failures do not roll the field back.
pub fn stop_wrapped_goal<H: WrappedGoalHost>(
    retained: &H::Lease,
    host: &mut H,
) -> Result<(), H::Error> {
    let goal = host.with_wrapped_goal(retained, |state| {
        if !state.running {
            None
        } else {
            state.running = false;
            Some(state.goal.clone())
        }
    })?;
    if let Some(goal) = goal {
        host.goal_stop(&goal)?;
    }
    Ok(())
}

// Original default ObjectLinkedOpenHashSet constructor fields, before any
// insertion: expected16, f=.75, n/minN32, mask31, maxFill24, key/link[n+1],
// first/last=-1. These are the actual native selector's owned collection fields,
// not a copied Vec of goals. Nonempty set operations/iteration are NOT ported.
struct LinkedGoalSet<W> {
    keys: Vec<Option<W>>,
    links: Vec<i64>,
    size: i32,
    first: i32,
    last: i32,
    contains_null: bool,
    n: i32,
    min_n: i32,
    mask: i32,
    max_fill: i32,
    load_factor: f32,
}
impl<W> LinkedGoalSet<W> {
    fn new() -> Self {
        Self {
            keys: (0..33).map(|_| None).collect(),
            links: vec![0; 33],
            size: 0,
            first: -1,
            last: -1,
            contains_null: false,
            n: 32,
            min_n: 32,
            mask: 31,
            max_fill: 24,
            load_factor: 0.75,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InitialLinkedSetLayout {
    pub key_slots: usize,
    pub link_slots: usize,
    pub size: i32,
    pub first: i32,
    pub last: i32,
    pub contains_null: bool,
    pub n: i32,
    pub min_n: i32,
    pub mask: i32,
    pub max_fill: i32,
    pub load_factor: f32,
}

/// Source GoalSelector27–34. S is its actual profiler Supplier; W retains actual
/// wrapper identity. The four slots are the nonnull Goal.Flag EnumMap domain.
pub struct GoalSelectorState<S, W> {
    locked_flags: [Option<W>; 4],
    available_goals: LinkedGoalSet<W>,
    disabled_flags: GoalFlags,
    supplier: S,
}
impl<S, W> GoalSelectorState<S, W> {
    /// Mob must call Level.getProfilerSupplier separately for each source
    /// selector and attach each returned owner at its actual constructor phase.
    pub fn new(supplier: S) -> Self {
        Self {
            locked_flags: std::array::from_fn(|_| None),
            available_goals: LinkedGoalSet::new(),
            disabled_flags: GoalFlags::default(),
            supplier,
        }
    }
    pub fn disabled_flags(&self) -> GoalFlags {
        self.disabled_flags
    }
    pub fn locked_flag_count(&self) -> usize {
        self.locked_flags
            .iter()
            .filter(|entry| entry.is_some())
            .count()
    }
    pub fn available_goal_count(&self) -> i32 {
        self.available_goals.size
    }
    /// Read-only constructor evidence, not a mutable Java collection view.
    pub fn initial_linked_set_layout(&self) -> InitialLinkedSetLayout {
        let set = &self.available_goals;
        InitialLinkedSetLayout {
            key_slots: set.keys.len(),
            link_slots: set.links.len(),
            size: set.size,
            first: set.first,
            last: set.last,
            contains_null: set.contains_null,
            n: set.n,
            min_n: set.min_n,
            mask: set.mask,
            max_fill: set.max_fill,
            load_factor: set.load_factor,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoalPhase {
    CleanupGoals,
    CleanupLocks,
    UpdateGoals,
    RunningGoals,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedGoals {
    NonemptyAvailable(GoalPhase),
    NonemptyLockedFlags,
    /// addGoal41–42 would construct/add a real wrapper and run LinkedHashSet
    /// equality/hash semantics; this module refuses rather than discarding it.
    AddGoalRequiresLinkedSetOwner,
    RawAvailableGoalsView,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GoalError<E> {
    Host(E),
    Unsupported(UnsupportedGoals),
}
impl<E> From<E> for GoalError<E> {
    fn from(error: E) -> Self {
        Self::Host(error)
    }
}

pub trait GoalSelectorHost {
    type Error;
    type Lease;
    /// Clone retains the same actual Supplier, never another Level/profiler.
    type Supplier: Clone;
    type Wrapper;
    type Profiler;
    /// Canonical in-place fields; no callback or retained borrow inside access.
    fn with_selector<R>(
        &mut self,
        retained: &Self::Lease,
        access: impl FnOnce(&mut GoalSelectorState<Self::Supplier, Self::Wrapper>) -> R,
    ) -> Result<R, Self::Error>;
    fn supplier_get(&mut self, supplier: &Self::Supplier) -> Result<Self::Profiler, Self::Error>;
    fn profile(
        &mut self,
        profiler: &mut Self::Profiler,
        event: ProfileEvent,
    ) -> Result<(), Self::Error>;
}

fn require_empty_available<H: GoalSelectorHost>(
    retained: &H::Lease,
    phase: GoalPhase,
    host: &mut H,
) -> Result<(), GoalError<H::Error>> {
    let empty = host.with_selector(retained, |state| {
        state.available_goals.size == 0 && !state.available_goals.contains_null
    })?;
    if empty {
        Ok(())
    } else {
        Err(GoalError::Unsupported(UnsupportedGoals::NonemptyAvailable(
            phase,
        )))
    }
}

/// Selected ordinary source tick78–104 for initialized empty-goal collections.
/// Check the ACTUAL collections after callbacks at each source traversal point;
/// reentry cannot turn an unsupported nonempty selector into successful no-op AI.
pub fn tick<H: GoalSelectorHost>(
    retained: &H::Lease,
    host: &mut H,
) -> Result<(), GoalError<H::Error>> {
    let supplier = host.with_selector(retained, |state| state.supplier.clone())?;
    let mut profiler = host.supplier_get(&supplier)?;
    host.profile(&mut profiler, ProfileEvent::Push("goalCleanup"))?;
    require_empty_available(retained, GoalPhase::CleanupGoals, host)?;
    if host.with_selector(retained, |state| {
        state.locked_flags.iter().any(|entry| entry.is_some())
    })? {
        return Err(GoalError::Unsupported(
            UnsupportedGoals::NonemptyLockedFlags,
        ));
    }
    host.profile(&mut profiler, ProfileEvent::Pop)?;
    host.profile(&mut profiler, ProfileEvent::Push("goalUpdate"))?;
    require_empty_available(retained, GoalPhase::UpdateGoals, host)?;
    host.profile(&mut profiler, ProfileEvent::Pop)?;
    // Source performs a second Supplier.get inside this ordinary virtual body.
    tick_running_goals(retained, true, host)
}

/// Source107–116 selected empty-goal body. Both tickAll values still obtain the
/// actual Supplier/profiler; neither silently substitutes an empty callback.
pub fn tick_running_goals<H: GoalSelectorHost>(
    retained: &H::Lease,
    _tick_all: bool,
    host: &mut H,
) -> Result<(), GoalError<H::Error>> {
    let supplier = host.with_selector(retained, |state| state.supplier.clone())?;
    let mut profiler = host.supplier_get(&supplier)?;
    host.profile(&mut profiler, ProfileEvent::Push("goalTick"))?;
    require_empty_available(retained, GoalPhase::RunningGoals, host)?;
    host.profile(&mut profiler, ProfileEvent::Pop)?;
    Ok(())
}

/// Source123–136, on the actual source disabled EnumSet. No Goal is started or
/// stopped by these setters themselves. Source ordinary selector class only.
pub fn set_control_flag<H: GoalSelectorHost>(
    retained: &H::Lease,
    flag: GoalFlag,
    enabled: bool,
    host: &mut H,
) -> Result<(), H::Error> {
    host.with_selector(retained, |state| {
        if enabled {
            state.disabled_flags.remove(flag)
        } else {
            state.disabled_flags.insert(flag)
        }
    })
}

/// Explicit unsupported boundary, not an implementation of addGoal41–42.
pub fn add_goal<S, W, G>(
    _state: &mut GoalSelectorState<S, W>,
    _priority: i32,
    _goal: G,
) -> Result<(), UnsupportedGoals> {
    Err(UnsupportedGoals::AddGoalRequiresLinkedSetOwner)
}

/// Source getAvailableGoals119 returns its actual mutable set. That full view
/// is not replaced by copied/sorted goals or an immutable empty slice here.
pub fn available_goals_view<S, W>(
    _state: &GoalSelectorState<S, W>,
) -> Result<(), UnsupportedGoals> {
    Err(UnsupportedGoals::RawAvailableGoalsView)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    type State = GoalSelectorState<Rc<()>, Rc<()>>;
    struct Host {
        state: Rc<RefCell<State>>,
        supplier: Rc<()>,
        gets: i32,
        calls: Vec<String>,
        reenter: bool,
        nonempty: Option<GoalPhase>,
        fail: Option<&'static str>,
    }
    impl Host {
        fn new() -> Self {
            let supplier = Rc::new(());
            Self {
                state: Rc::new(RefCell::new(State::new(supplier.clone()))),
                supplier,
                gets: 0,
                calls: Vec::new(),
                reenter: false,
                nonempty: None,
                fail: None,
            }
        }
    }
    impl GoalSelectorHost for Host {
        type Error = &'static str;
        type Lease = Rc<RefCell<State>>;
        type Supplier = Rc<()>;
        type Wrapper = Rc<()>;
        type Profiler = i32;
        fn with_selector<R>(
            &mut self,
            retained: &Self::Lease,
            access: impl FnOnce(&mut State) -> R,
        ) -> Result<R, Self::Error> {
            assert!(Rc::ptr_eq(retained, &self.state));
            Ok(access(&mut retained.borrow_mut()))
        }
        fn supplier_get(&mut self, supplier: &Rc<()>) -> Result<i32, Self::Error> {
            assert!(Rc::ptr_eq(supplier, &self.supplier));
            drop(self.state.try_borrow_mut().unwrap());
            self.gets += 1;
            self.calls.push(format!("supplier:{}", self.gets));
            Ok(self.gets)
        }
        fn profile(&mut self, profiler: &mut i32, event: ProfileEvent) -> Result<(), Self::Error> {
            drop(self.state.try_borrow_mut().unwrap());
            let text = format!("{event:?}:{profiler}");
            self.calls.push(text.clone());
            if self.fail == Some(text.as_str()) {
                return Err("controlled profiler failure");
            }
            if self.reenter && event == ProfileEvent::Push("goalCleanup") {
                self.reenter = false;
                let lease = self.state.clone();
                set_control_flag(&lease, GoalFlag::Jump, false, self)?;
                tick_running_goals(&lease, false, self).unwrap();
            }
            let phase = match event {
                ProfileEvent::Push("goalCleanup") => Some(GoalPhase::CleanupGoals),
                ProfileEvent::Push("goalUpdate") => Some(GoalPhase::UpdateGoals),
                ProfileEvent::Push("goalTick") => Some(GoalPhase::RunningGoals),
                _ => None,
            };
            if self.nonempty.is_some() && phase == self.nonempty {
                let mut state = self.state.borrow_mut();
                state.available_goals.keys[0] = Some(Rc::new(()));
                state.available_goals.size = 1;
            }
            Ok(())
        }
    }
    #[test]
    fn actual_empty_state_has_source_constructor_and_all_profiler_work() {
        let mut host = Host::new();
        let lease = host.state.clone();
        let layout = lease.borrow().initial_linked_set_layout();
        assert_eq!(
            (
                layout.key_slots,
                layout.link_slots,
                layout.n,
                layout.min_n,
                layout.mask,
                layout.max_fill,
                layout.first,
                layout.last
            ),
            (33, 33, 32, 32, 31, 24, -1, -1)
        );
        assert_eq!(layout.load_factor, 0.75);
        assert_eq!(lease.borrow().available_goal_count(), 0);
        assert_eq!(lease.borrow().locked_flag_count(), 0);
        tick(&lease, &mut host).unwrap();
        assert_eq!(
            host.calls,
            [
                "supplier:1",
                "Push(\"goalCleanup\"):1",
                "Pop:1",
                "Push(\"goalUpdate\"):1",
                "Pop:1",
                "supplier:2",
                "Push(\"goalTick\"):2",
                "Pop:2"
            ]
        );
        host.calls.clear();
        tick_running_goals(&lease, false, &mut host).unwrap();
        assert_eq!(host.calls, ["supplier:3", "Push(\"goalTick\"):3", "Pop:3"]);
        host.reenter = true;
        host.calls.clear();
        tick(&lease, &mut host).unwrap();
        assert!(lease.borrow().disabled_flags().contains(GoalFlag::Jump));
        assert_eq!(
            &host.calls[..5],
            [
                "supplier:4",
                "Push(\"goalCleanup\"):4",
                "supplier:5",
                "Push(\"goalTick\"):5",
                "Pop:5"
            ]
        );
        set_control_flag(&lease, GoalFlag::Jump, true, &mut host).unwrap();
        assert!(!lease.borrow().disabled_flags().contains(GoalFlag::Jump));
    }
    #[test]
    fn reentrant_nonempty_and_failed_profiler_refuse_without_fake_cleanup() {
        for phase in [
            GoalPhase::CleanupGoals,
            GoalPhase::UpdateGoals,
            GoalPhase::RunningGoals,
        ] {
            let mut host = Host::new();
            host.nonempty = Some(phase);
            let lease = host.state.clone();
            assert_eq!(
                tick(&lease, &mut host),
                Err(GoalError::Unsupported(UnsupportedGoals::NonemptyAvailable(
                    phase
                )))
            );
            assert!(host.calls.last().unwrap().starts_with("Push"));
            assert_eq!(lease.borrow().available_goal_count(), 1);
        }
        let mut host = Host::new();
        host.state.borrow_mut().locked_flags[0] = Some(Rc::new(()));
        let lease = host.state.clone();
        assert_eq!(
            tick(&lease, &mut host),
            Err(GoalError::Unsupported(
                UnsupportedGoals::NonemptyLockedFlags
            ))
        );
        let mut host = Host::new();
        host.fail = Some("Pop:1");
        let lease = host.state.clone();
        assert_eq!(
            tick(&lease, &mut host),
            Err(GoalError::Host("controlled profiler failure"))
        );
        assert_eq!(host.gets, 1);
        assert_eq!(
            add_goal(&mut lease.borrow_mut(), 1, Rc::new(())),
            Err(UnsupportedGoals::AddGoalRequiresLinkedSetOwner)
        );
        assert_eq!(
            available_goals_view(&lease.borrow()),
            Err(UnsupportedGoals::RawAvailableGoalsView)
        );
    }
    struct WrapperHost {
        state: Rc<RefCell<WrappedGoalState<Rc<()>>>>,
        goal: Rc<()>,
        events: Vec<&'static str>,
        restart: bool,
        fail: bool,
    }
    impl WrappedGoalHost for WrapperHost {
        type Error = &'static str;
        type Lease = Rc<RefCell<WrappedGoalState<Rc<()>>>>;
        type Goal = Rc<()>;
        fn with_wrapped_goal<R>(
            &mut self,
            retained: &Self::Lease,
            access: impl FnOnce(&mut WrappedGoalState<Rc<()>>) -> R,
        ) -> Result<R, Self::Error> {
            assert!(Rc::ptr_eq(retained, &self.state));
            Ok(access(&mut retained.borrow_mut()))
        }
        fn goal_start(&mut self, goal: &Rc<()>) -> Result<(), Self::Error> {
            assert!(Rc::ptr_eq(goal, &self.goal));
            assert!(self.state.borrow().running);
            self.events.push("start");
            let lease = self.state.clone();
            start_wrapped_goal(&lease, self)?;
            if self.fail {
                Err("goal failure")
            } else {
                Ok(())
            }
        }
        fn goal_stop(&mut self, goal: &Rc<()>) -> Result<(), Self::Error> {
            assert!(Rc::ptr_eq(goal, &self.goal));
            assert!(!self.state.borrow().running);
            self.events.push("stop");
            if self.restart {
                let lease = self.state.clone();
                start_wrapped_goal(&lease, self)?;
            }
            Ok(())
        }
    }
    #[test]
    fn wrapper_mutation_precedes_same_owner_callbacks_and_keeps_failure_prefix() {
        let goal = Rc::new(());
        let state = Rc::new(RefCell::new(WrappedGoalState::new(8, goal.clone())));
        let mut host = WrapperHost {
            state: state.clone(),
            goal,
            events: Vec::new(),
            restart: false,
            fail: true,
        };
        assert_eq!(state.borrow().priority(), 8);
        assert_eq!(
            state.borrow().inherited_constructor_flags(),
            GoalFlags::default()
        );
        assert_eq!(start_wrapped_goal(&state, &mut host), Err("goal failure"));
        assert!(state.borrow().running);
        assert_eq!(host.events, ["start"]);
        host.fail = false;
        host.restart = true;
        stop_wrapped_goal(&state, &mut host).unwrap();
        assert!(state.borrow().running);
        assert_eq!(host.events, ["start", "stop", "start"]);
    }
}
