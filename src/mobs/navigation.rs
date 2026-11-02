//! Java 1.21 GroundPathNavigation/PathNavigation constructor and bounded tick.
//!
//! Native owners retain the original Mob/Level/evaluator/path/finder handles.
//! Initialized null-path execution is real source work; active path traversal,
//! delayed recomputation and search report explicit missing algorithm owners.
//! No actor, NoAI/isDone flag, world result, RNG or scheduler is invented.

use crate::geometry::Vec3;
use std::collections::BTreeMap;

pub type BlockPosition = (i32, i32, i32);

/// Original BinaryHeap5–6 constructor fields. Heap algorithms are not ported by
/// constructing their real state; source Node references begin null, size0.
pub struct BinaryHeapState<N> {
    heap: [Option<N>; 128],
    size: i32,
}
impl<N> BinaryHeapState<N> {
    pub fn new() -> Self {
        Self {
            heap: std::array::from_fn(|_| None),
            size: 0,
        }
    }
    pub fn node_slots(&self) -> usize {
        self.heap.len()
    }
    pub fn size(&self) -> i32 {
        self.size
    }
}
impl<N> Default for BinaryHeapState<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Source PathFinder27–37: Node[32], BinaryHeap, actual evaluator, raw budget.
pub struct PathFinderState<E, N> {
    neighbors: [Option<N>; 32],
    open_set: BinaryHeapState<N>,
    evaluator: E,
    max_visited_nodes: i32,
}
impl<E, N> PathFinderState<E, N> {
    pub fn new(evaluator: E, max_visited_nodes: i32) -> Self {
        Self {
            neighbors: std::array::from_fn(|_| None),
            open_set: BinaryHeapState::new(),
            evaluator,
            max_visited_nodes,
        }
    }
    pub fn neighbor_slots(&self) -> usize {
        self.neighbors.len()
    }
    pub fn open_set(&self) -> &BinaryHeapState<N> {
        &self.open_set
    }
    pub fn evaluator(&self) -> &E {
        &self.evaluator
    }
    pub fn max_visited_nodes(&self) -> i32 {
        self.max_visited_nodes
    }
    /// Explicit boundary for the original findPath(region,Mob,targets,range,
    /// reach,multiplier) entry. No empty result substitutes for missing search.
    /// This refusal is not a port of the source search algorithm41–145.
    pub fn find_path<R, M, T, P>(
        &mut self,
        _region: &R,
        _mob: &M,
        _targets: &T,
        _max_range: f32,
        _reach_range: i32,
        _multiplier: f32,
    ) -> Result<Option<P>, UnsupportedNavigation> {
        Err(UnsupportedNavigation::PathSearchRequiresAlgorithmOwner)
    }
}

/// Actual inherited NodeEvaluator constructor22–33 field domain. Private cache
/// storage has only constructor-empty observations in this stage; no fastutil
/// lookup, hashing, mutable views, prepare/done or node algorithm is claimed.
pub struct NodeEvaluatorState<M, C, N> {
    current_context: Option<C>,
    mob: Option<M>,
    nodes: BTreeMap<i32, N>,
    entity_width: i32,
    entity_height: i32,
    entity_depth: i32,
    can_pass_doors: bool,
    can_open_doors: bool,
    can_float: bool,
    can_walk_over_fences: bool,
}
impl<M, C, N> NodeEvaluatorState<M, C, N> {
    fn new() -> Self {
        Self {
            current_context: None,
            mob: None,
            nodes: BTreeMap::new(),
            entity_width: 0,
            entity_height: 0,
            entity_depth: 0,
            can_pass_doors: false,
            can_open_doors: false,
            can_float: false,
            can_walk_over_fences: false,
        }
    }
    pub fn can_pass_doors(&self) -> bool {
        self.can_pass_doors
    }
    pub fn set_can_pass_doors(&mut self, value: bool) {
        self.can_pass_doors = value;
    }
    pub fn can_open_doors(&self) -> bool {
        self.can_open_doors
    }
    pub fn set_can_open_doors(&mut self, value: bool) {
        self.can_open_doors = value;
    }
    pub fn can_float(&self) -> bool {
        self.can_float
    }
    pub fn set_can_float(&mut self, value: bool) {
        self.can_float = value;
    }
    pub fn can_walk_over_fences(&self) -> bool {
        self.can_walk_over_fences
    }
    pub fn set_can_walk_over_fences(&mut self, value: bool) {
        self.can_walk_over_fences = value;
    }
}

/// Original WalkNodeEvaluator38–43 fields. Integer-key caches are deterministic
/// native empty storage; collision cache has no successful lookup/insertion API.
/// None of these are replacement world/path-type answers. Search is refused.
pub struct WalkNodeEvaluatorState<M, C, N, T, B> {
    base: NodeEvaluatorState<M, C, N>,
    path_types_by_pos_cache_by_mob: BTreeMap<i64, T>,
    collision_cache: Vec<(B, bool)>,
    reusable_neighbors: [Option<N>; 4],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvaluatorConstructorLayout {
    pub has_context: bool,
    pub has_mob: bool,
    pub nodes: usize,
    pub dimensions: (i32, i32, i32),
    pub can_pass_doors: bool,
    pub can_open_doors: bool,
    pub can_float: bool,
    pub can_walk_over_fences: bool,
    pub path_types: usize,
    pub collisions: usize,
    pub reusable_neighbor_slots: usize,
}
impl<M, C, N, T, B> WalkNodeEvaluatorState<M, C, N, T, B> {
    pub fn new() -> Self {
        Self {
            base: NodeEvaluatorState::new(),
            path_types_by_pos_cache_by_mob: BTreeMap::new(),
            collision_cache: Vec::new(),
            reusable_neighbors: std::array::from_fn(|_| None),
        }
    }
    pub fn base(&self) -> &NodeEvaluatorState<M, C, N> {
        &self.base
    }
    pub fn base_mut(&mut self) -> &mut NodeEvaluatorState<M, C, N> {
        &mut self.base
    }
    pub fn constructor_layout(&self) -> EvaluatorConstructorLayout {
        EvaluatorConstructorLayout {
            has_context: self.base.current_context.is_some(),
            has_mob: self.base.mob.is_some(),
            nodes: self.base.nodes.len(),
            dimensions: (
                self.base.entity_width,
                self.base.entity_height,
                self.base.entity_depth,
            ),
            can_pass_doors: self.base.can_pass_doors,
            can_open_doors: self.base.can_open_doors,
            can_float: self.base.can_float,
            can_walk_over_fences: self.base.can_walk_over_fences,
            path_types: self.path_types_by_pos_cache_by_mob.len(),
            collisions: self.collision_cache.len(),
            reusable_neighbor_slots: self.reusable_neighbors.len(),
        }
    }
}
impl<M, C, N, T, B> Default for WalkNodeEvaluatorState<M, C, N, T, B> {
    fn default() -> Self {
        Self::new()
    }
}

/// The original navigation fields, not another physical actor or mob tick.
/// Optional evaluator/finder represent their actual null constructor prefix;
/// successful construction installs both real retained owners before returning.
pub struct GroundNavigationState<M, L, P, E, F> {
    mob: M,
    level: L,
    path: Option<P>,
    speed_modifier: f64,
    tick: i32,
    last_stuck_check: i32,
    last_stuck_check_pos: Vec3,
    timeout_cached_node: BlockPosition,
    timeout_timer: i64,
    last_timeout_check: i64,
    timeout_limit: f64,
    max_distance_to_waypoint: f32,
    has_delayed_recomputation: bool,
    time_last_recompute: i64,
    node_evaluator: Option<E>,
    target_pos: Option<BlockPosition>,
    reach_range: i32,
    max_visited_nodes_multiplier: f32,
    path_finder: Option<F>,
    is_stuck: bool,
    avoid_sun: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavigationConstructorLayout {
    pub has_path: bool,
    pub speed_modifier: f64,
    pub tick: i32,
    pub last_stuck_check: i32,
    pub last_stuck_check_pos: Vec3,
    pub timeout_cached_node: BlockPosition,
    pub timeout_timer: i64,
    pub last_timeout_check: i64,
    pub timeout_limit: f64,
    pub max_distance_to_waypoint: f32,
    pub delayed: bool,
    pub time_last_recompute: i64,
    pub has_evaluator: bool,
    pub target_pos: Option<BlockPosition>,
    pub reach_range: i32,
    pub max_visited_nodes_multiplier: f32,
    pub has_path_finder: bool,
    pub is_stuck: bool,
    pub avoid_sun: bool,
}
impl<M, L, P, E, F> GroundNavigationState<M, L, P, E, F> {
    fn constructor_prefix(mob: M, level: L) -> Self {
        Self {
            mob,
            level,
            path: None,
            speed_modifier: 0.0,
            tick: 0,
            last_stuck_check: 0,
            last_stuck_check_pos: Vec3::ZERO,
            timeout_cached_node: (0, 0, 0),
            timeout_timer: 0,
            last_timeout_check: 0,
            timeout_limit: 0.0,
            max_distance_to_waypoint: 0.5,
            has_delayed_recomputation: false,
            time_last_recompute: 0,
            node_evaluator: None,
            target_pos: None,
            reach_range: 0,
            max_visited_nodes_multiplier: 1.0,
            path_finder: None,
            is_stuck: false,
            avoid_sun: false,
        }
    }
    pub fn mob(&self) -> &M {
        &self.mob
    }
    pub fn level(&self) -> &L {
        &self.level
    }
    pub fn path(&self) -> Option<&P> {
        self.path.as_ref()
    }
    pub fn node_evaluator(&self) -> Option<&E> {
        self.node_evaluator.as_ref()
    }
    pub fn path_finder(&self) -> Option<&F> {
        self.path_finder.as_ref()
    }
    pub fn constructor_layout(&self) -> NavigationConstructorLayout {
        NavigationConstructorLayout {
            has_path: self.path.is_some(),
            speed_modifier: self.speed_modifier,
            tick: self.tick,
            last_stuck_check: self.last_stuck_check,
            last_stuck_check_pos: self.last_stuck_check_pos,
            timeout_cached_node: self.timeout_cached_node,
            timeout_timer: self.timeout_timer,
            last_timeout_check: self.last_timeout_check,
            timeout_limit: self.timeout_limit,
            max_distance_to_waypoint: self.max_distance_to_waypoint,
            delayed: self.has_delayed_recomputation,
            time_last_recompute: self.time_last_recompute,
            has_evaluator: self.node_evaluator.is_some(),
            target_pos: self.target_pos,
            reach_range: self.reach_range,
            max_visited_nodes_multiplier: self.max_visited_nodes_multiplier,
            has_path_finder: self.path_finder.is_some(),
            is_stuck: self.is_stuck,
            avoid_sun: self.avoid_sun,
        }
    }
    /// Exact source107–108,89–90,96–97 raw field setters.
    pub fn set_speed_modifier(&mut self, value: f64) {
        self.speed_modifier = value;
    }
    pub fn reset_max_visited_nodes_multiplier(&mut self) {
        self.max_visited_nodes_multiplier = 1.0;
    }
    pub fn set_max_visited_nodes_multiplier(&mut self, value: f32) {
        self.max_visited_nodes_multiplier = value;
    }
    /// Source435–436 clears only path, not timing/stuck/speed fields.
    pub fn stop(&mut self) {
        self.path = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedNavigation {
    DelayedRecomputationRequiresAlgorithmOwner,
    ActivePathRequiresAlgorithmOwner,
    PathSearchRequiresAlgorithmOwner,
}
#[derive(Debug, PartialEq, Eq)]
pub enum NavigationError<E> {
    Host(E),
    NullNodeEvaluator,
    Unsupported(UnsupportedNavigation),
}
impl<E> From<E> for NavigationError<E> {
    fn from(error: E) -> Self {
        Self::Host(error)
    }
}

pub trait NavigationHost {
    type Error;
    type Lease;
    type Mob: Clone;
    type Level: Clone;
    type Path: Clone;
    type Evaluator: Clone;
    type PathFinder;
    /// Canonical field fragments only; release every owner borrow before the
    /// next host callback. Clone retains exact object identity/lifetime.
    fn with_navigation<R>(
        &mut self,
        retained: &Self::Lease,
        access: impl FnOnce(
            &mut GroundNavigationState<
                Self::Mob,
                Self::Level,
                Self::Path,
                Self::Evaluator,
                Self::PathFinder,
            >,
        ) -> R,
    ) -> Result<R, Self::Error>;
    /// Actual retained source Path.isDone, not a detached navigation flag.
    fn path_is_done(&mut self, path: &Self::Path) -> Result<bool, Self::Error>;
}

pub trait GroundConstructionHost: NavigationHost {
    type Context;
    type Node;
    type PathType;
    type CollisionBounds;
    /// Install this native constructor-prefix object in the actual retained
    /// navigation arena before FOLLOW_RANGE or virtual factory callbacks.
    fn allocate_ground_navigation(
        &mut self,
        state: GroundNavigationState<
            Self::Mob,
            Self::Level,
            Self::Path,
            Self::Evaluator,
            Self::PathFinder,
        >,
    ) -> Result<Self::Lease, Self::Error>;
    fn follow_range_attribute(&mut self, mob: &Self::Mob) -> Result<f64, Self::Error>;
    /// Actual virtual PathNavigation.createPathFinder on this exact object.
    /// Ordinary Ground owners select create_ground_path_finder below; other
    /// classes require their own real source dispatch, never a successful stub.
    fn create_path_finder(
        &mut self,
        retained: &Self::Lease,
        max_nodes: i32,
    ) -> Result<Self::PathFinder, NavigationError<Self::Error>>;
    fn allocate_walk_evaluator(
        &mut self,
        state: WalkNodeEvaluatorState<
            Self::Mob,
            Self::Context,
            Self::Node,
            Self::PathType,
            Self::CollisionBounds,
        >,
    ) -> Result<Self::Evaluator, Self::Error>;
    fn evaluator_set_can_pass_doors(
        &mut self,
        evaluator: &Self::Evaluator,
        value: bool,
    ) -> Result<(), Self::Error>;
    fn allocate_path_finder(
        &mut self,
        state: PathFinderState<Self::Evaluator, Self::Node>,
    ) -> Result<Self::PathFinder, Self::Error>;
}

// Exact Mth.floor(double): Java d2i saturates/NaN→0, followed by wrapping i-1.
fn mth_floor(value: f64) -> i32 {
    let truncated = value as i32;
    if value < f64::from(truncated) {
        truncated.wrapping_sub(1)
    } else {
        truncated
    }
}

/// PathNavigation59–86/Ground23–24. This constructs actual navigation state;
/// source Mob createNavigation177 must then attach that same retained object.
pub fn construct_ground<H: GroundConstructionHost>(
    mob: &H::Mob,
    level: &H::Level,
    host: &mut H,
) -> Result<H::Lease, NavigationError<H::Error>> {
    let state = GroundNavigationState::constructor_prefix(mob.clone(), level.clone());
    let retained = host.allocate_ground_navigation(state)?;
    let range = host.follow_range_attribute(mob)?;
    let budget = mth_floor(range * 16.0);
    let finder = host.create_path_finder(&retained, budget)?;
    host.with_navigation(&retained, |state| state.path_finder = Some(finder))?;
    Ok(retained)
}

/// Ground.createPathFinder28–30. Preserve evaluator installation before its
/// capability setter, and reread the actual field before each later operation.
pub fn create_ground_path_finder<H: GroundConstructionHost>(
    retained: &H::Lease,
    max_nodes: i32,
    host: &mut H,
) -> Result<H::PathFinder, NavigationError<H::Error>> {
    let evaluator = host.allocate_walk_evaluator(WalkNodeEvaluatorState::new())?;
    host.with_navigation(retained, |state| state.node_evaluator = Some(evaluator))?;
    let evaluator = host
        .with_navigation(retained, |state| state.node_evaluator.clone())?
        .ok_or(NavigationError::NullNodeEvaluator)?;
    host.evaluator_set_can_pass_doors(&evaluator, true)?;
    let evaluator = host
        .with_navigation(retained, |state| state.node_evaluator.clone())?
        .ok_or(NavigationError::NullNodeEvaluator)?;
    host.allocate_path_finder(PathFinderState::new(evaluator, max_nodes))
        .map_err(NavigationError::Host)
}

/// Selected inherited base isDone424. Some paths require their actual source
/// Path.isDone owner; null derives true from the actual path field itself.
pub fn is_done<H: NavigationHost>(retained: &H::Lease, host: &mut H) -> Result<bool, H::Error> {
    let path = host.with_navigation(retained, |state| state.path.clone())?;
    match path {
        None => Ok(true),
        Some(path) => host.path_is_done(&path),
    }
}

/// Source278–285 through the real null/finished-path return. This entry must
/// receive Mob.navigation's DIRECT field owner for Mob.serverAiStep. Controls
/// separately use actual virtual Mob.getNavigation (which may delegate riding).
/// Active paths and delayed recomputation stop explicitly at their first
/// unimplemented source phase; no world/debug/movement callback is fabricated.
pub fn tick<H: NavigationHost>(
    retained: &H::Lease,
    host: &mut H,
) -> Result<(), NavigationError<H::Error>> {
    host.with_navigation(retained, |state| state.tick = state.tick.wrapping_add(1))?;
    if host.with_navigation(retained, |state| state.has_delayed_recomputation)? {
        return Err(NavigationError::Unsupported(
            UnsupportedNavigation::DelayedRecomputationRequiresAlgorithmOwner,
        ));
    }
    if is_done(retained, host)? {
        return Ok(());
    }
    Err(NavigationError::Unsupported(
        UnsupportedNavigation::ActivePathRequiresAlgorithmOwner,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    type Mob = Rc<()>;
    type Level = Rc<()>;
    type Path = Rc<RefCell<bool>>;
    type EvalState = WalkNodeEvaluatorState<Mob, (), u32, (), ()>;
    type Eval = Rc<RefCell<EvalState>>;
    type Finder = Rc<RefCell<PathFinderState<Eval, u32>>>;
    type State = GroundNavigationState<Mob, Level, Path, Eval, Finder>;
    type Lease = Rc<RefCell<State>>;
    struct Host {
        mob: Mob,
        level: Level,
        state: Option<Lease>,
        evaluator: Option<Eval>,
        finder: Option<Finder>,
        range: f64,
        calls: Vec<String>,
        reenter_range: bool,
        stop_in_path: bool,
        fail: Option<&'static str>,
    }
    impl Host {
        fn new() -> Self {
            Self {
                mob: Rc::new(()),
                level: Rc::new(()),
                state: None,
                evaluator: None,
                finder: None,
                range: 16.0,
                calls: Vec::new(),
                reenter_range: false,
                stop_in_path: false,
                fail: None,
            }
        }
        fn event(&mut self, event: impl Into<String>) -> Result<(), &'static str> {
            if let Some(state) = &self.state {
                drop(
                    state
                        .try_borrow_mut()
                        .expect("navigation borrowed across callback"),
                );
            }
            let event = event.into();
            let fail = self.fail == Some(event.as_str());
            self.calls.push(event);
            if fail {
                Err("controlled owner failure")
            } else {
                Ok(())
            }
        }
        fn construct(&mut self) -> Result<Lease, NavigationError<&'static str>> {
            let mob = self.mob.clone();
            let level = self.level.clone();
            construct_ground(&mob, &level, self)
        }
    }
    impl NavigationHost for Host {
        type Error = &'static str;
        type Lease = Lease;
        type Mob = Mob;
        type Level = Level;
        type Path = Path;
        type Evaluator = Eval;
        type PathFinder = Finder;
        fn with_navigation<R>(
            &mut self,
            retained: &Lease,
            access: impl FnOnce(&mut State) -> R,
        ) -> Result<R, Self::Error> {
            assert!(Rc::ptr_eq(retained, self.state.as_ref().unwrap()));
            Ok(access(&mut retained.borrow_mut()))
        }
        fn path_is_done(&mut self, path: &Path) -> Result<bool, Self::Error> {
            self.event("path.isDone")?;
            if self.stop_in_path {
                let state = self.state.as_ref().unwrap().clone();
                state.borrow_mut().stop();
            }
            Ok(*path.borrow())
        }
    }
    impl GroundConstructionHost for Host {
        type Context = ();
        type Node = u32;
        type PathType = ();
        type CollisionBounds = ();
        fn allocate_ground_navigation(&mut self, state: State) -> Result<Lease, Self::Error> {
            self.event("new.navigation")?;
            assert!(Rc::ptr_eq(state.mob(), &self.mob));
            assert!(Rc::ptr_eq(state.level(), &self.level));
            let lease = Rc::new(RefCell::new(state));
            self.state = Some(lease.clone());
            Ok(lease)
        }
        fn follow_range_attribute(&mut self, mob: &Mob) -> Result<f64, Self::Error> {
            self.event("attribute.FOLLOW_RANGE")?;
            assert!(Rc::ptr_eq(mob, &self.mob));
            let lease = self.state.as_ref().unwrap().clone();
            let layout = lease.borrow().constructor_layout();
            assert!(!layout.has_evaluator && !layout.has_path_finder);
            if self.reenter_range {
                tick(&lease, self).unwrap();
                lease.borrow_mut().set_speed_modifier(9.0);
            }
            Ok(self.range)
        }
        fn create_path_finder(
            &mut self,
            retained: &Lease,
            budget: i32,
        ) -> Result<Finder, NavigationError<Self::Error>> {
            self.event(format!("virtual.createPathFinder:{budget}"))?;
            create_ground_path_finder(retained, budget, self)
        }
        fn allocate_walk_evaluator(&mut self, state: EvalState) -> Result<Eval, Self::Error> {
            self.event("new.walkEvaluator")?;
            let layout = state.constructor_layout();
            assert_eq!(layout.dimensions, (0, 0, 0));
            assert!(
                !layout.has_context
                    && !layout.has_mob
                    && !layout.can_pass_doors
                    && !layout.can_open_doors
                    && !layout.can_float
                    && !layout.can_walk_over_fences
            );
            assert_eq!(
                (
                    layout.nodes,
                    layout.path_types,
                    layout.collisions,
                    layout.reusable_neighbor_slots
                ),
                (0, 0, 0, 4)
            );
            let evaluator = Rc::new(RefCell::new(state));
            self.evaluator = Some(evaluator.clone());
            Ok(evaluator)
        }
        fn evaluator_set_can_pass_doors(
            &mut self,
            evaluator: &Eval,
            value: bool,
        ) -> Result<(), Self::Error> {
            self.event("setCanPassDoors")?;
            assert!(Rc::ptr_eq(
                evaluator,
                self.state
                    .as_ref()
                    .unwrap()
                    .borrow()
                    .node_evaluator()
                    .unwrap()
            ));
            evaluator.borrow_mut().base_mut().set_can_pass_doors(value);
            Ok(())
        }
        fn allocate_path_finder(
            &mut self,
            state: PathFinderState<Eval, u32>,
        ) -> Result<Finder, Self::Error> {
            self.event("new.pathFinder")?;
            assert_eq!(state.neighbor_slots(), 32);
            assert_eq!(state.open_set().node_slots(), 128);
            assert_eq!(state.open_set().size(), 0);
            assert!(state.evaluator().borrow().base().can_pass_doors());
            let finder = Rc::new(RefCell::new(state));
            self.finder = Some(finder.clone());
            Ok(finder)
        }
    }
    #[test]
    fn source_constructor_state_order_budget_and_real_null_tick() {
        let mut host = Host::new();
        host.reenter_range = true;
        host.range = 3.51;
        let lease = host.construct().unwrap();
        assert_eq!(
            host.calls,
            [
                "new.navigation",
                "attribute.FOLLOW_RANGE",
                "virtual.createPathFinder:56",
                "new.walkEvaluator",
                "setCanPassDoors",
                "new.pathFinder"
            ]
        );
        let layout = lease.borrow().constructor_layout();
        assert_eq!(layout.tick, 1);
        assert_eq!(layout.speed_modifier, 9.0);
        assert_eq!(layout.max_distance_to_waypoint, 0.5);
        assert_eq!(layout.max_visited_nodes_multiplier, 1.0);
        assert_eq!(layout.last_stuck_check_pos, Vec3::ZERO);
        assert_eq!(layout.timeout_cached_node, (0, 0, 0));
        assert!(!layout.has_path && !layout.delayed && !layout.is_stuck && !layout.avoid_sun);
        assert!(layout.has_evaluator && layout.has_path_finder);
        assert_eq!(
            host.finder.as_ref().unwrap().borrow().max_visited_nodes(),
            56
        );
        host.calls.clear();
        tick(&lease, &mut host).unwrap();
        assert!(host.calls.is_empty());
        assert_eq!(lease.borrow().constructor_layout().tick, 2);
        lease.borrow_mut().tick = i32::MAX;
        tick(&lease, &mut host).unwrap();
        assert_eq!(lease.borrow().constructor_layout().tick, i32::MIN);
        assert_eq!(mth_floor(f64::NAN), 0);
        assert_eq!(mth_floor(f64::INFINITY), i32::MAX);
        assert_eq!(mth_floor(f64::NEG_INFINITY), i32::MAX);
        assert_eq!(mth_floor(-0.1), -1);
    }
    #[test]
    fn constructor_failure_keeps_installed_evaluator_and_refuses_search() {
        let mut host = Host::new();
        host.fail = Some("new.pathFinder");
        assert!(host.construct().is_err());
        let lease = host.state.as_ref().unwrap();
        assert!(lease.borrow().node_evaluator().is_some());
        assert!(lease.borrow().path_finder().is_none());
        assert!(host
            .evaluator
            .as_ref()
            .unwrap()
            .borrow()
            .base()
            .can_pass_doors());
        let mut finder = PathFinderState::<(), u32>::new((), -7);
        let result: Result<Option<()>, _> = finder.find_path(&(), &(), &(), 1.0, 0, 1.0);
        assert_eq!(
            result,
            Err(UnsupportedNavigation::PathSearchRequiresAlgorithmOwner)
        );
        assert_eq!(finder.max_visited_nodes(), -7);
    }
    #[test]
    fn actual_path_and_delayed_state_are_checked_and_callback_errors_keep_tick() {
        let mut host = Host::new();
        let lease = host.construct().unwrap();
        host.calls.clear();
        lease.borrow_mut().has_delayed_recomputation = true;
        assert_eq!(
            tick(&lease, &mut host),
            Err(NavigationError::Unsupported(
                UnsupportedNavigation::DelayedRecomputationRequiresAlgorithmOwner
            ))
        );
        assert_eq!(lease.borrow().tick, 1);
        assert!(host.calls.is_empty());
        lease.borrow_mut().has_delayed_recomputation = false;
        let path = Rc::new(RefCell::new(false));
        lease.borrow_mut().path = Some(path.clone());
        assert_eq!(
            tick(&lease, &mut host),
            Err(NavigationError::Unsupported(
                UnsupportedNavigation::ActivePathRequiresAlgorithmOwner
            ))
        );
        assert_eq!(host.calls, ["path.isDone"]);
        *path.borrow_mut() = true;
        host.stop_in_path = true;
        host.calls.clear();
        tick(&lease, &mut host).unwrap();
        assert!(lease.borrow().path.is_none());
        assert_eq!(host.calls, ["path.isDone"]);
        lease.borrow_mut().path = Some(path);
        host.fail = Some("path.isDone");
        let before = lease.borrow().tick;
        assert_eq!(
            tick(&lease, &mut host),
            Err(NavigationError::Host("controlled owner failure"))
        );
        assert_eq!(lease.borrow().tick, before.wrapping_add(1));
        assert!(lease.borrow().path.is_some());
    }
}
