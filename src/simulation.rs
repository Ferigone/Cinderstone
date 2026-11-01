//! Vanilla 1.21 server/level phase ordering, with injected simulation callbacks.
//!
//! This drives ordering and gates. Chunk/entity/block/raid/function callbacks
//! retain responsibility for the actual mechanics and their own predicates.
use crate::{
    protocol::{Packet, Writer},
    tick::{TickClock, TickRateManager, WeatherChange, WeatherState},
};

pub const TIME_SYNC_INTERVAL: i32 = 20;
pub const MAX_SCHEDULED_TICKS_PER_TICK: usize = 65_536;
pub const EMPTY_TIME_NO_ENTITY_TICKS: i32 = 300;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutosaveRate {
    pub tick_rate: f32,
    pub sprinting: bool,
    pub average_tick_nanos: i64,
}
impl Default for AutosaveRate {
    fn default() -> Self {
        Self {
            tick_rate: 20.0,
            sprinting: false,
            average_tick_nanos: 50_000_000,
        }
    }
}
/// MinecraftServer.computeNextAutosaveInterval: five minutes at the current
/// rate, at least 100 server ticks. Sprint uses measured average tick duration.
pub fn autosave_interval(rate: AutosaveRate) -> i32 {
    let ticks_per_second = if rate.sprinting {
        1_000_000_000_f32 / rate.average_tick_nanos.wrapping_add(1) as f32
    } else {
        rate.tick_rate
    };
    ((ticks_per_second * 300.0) as i32).max(100)
}
/// IDs verified against the official 1.21 packet data-generator report.
pub fn ticking_state_packet(rate: &TickRateManager) -> Packet {
    let mut payload = Writer::new();
    payload.f32(rate.tick_rate()).bool(rate.is_frozen());
    Packet {
        id: 0x71,
        payload: payload.into_inner(),
    }
}
pub fn ticking_step_packet(rate: &TickRateManager) -> Packet {
    let mut payload = Writer::new();
    payload.varint(rate.frozen_ticks_to_run());
    Packet {
        id: 0x72,
        payload: payload.into_inner(),
    }
}
/// ServerTickRateManager.updateJoiningPlayer sends these two packets in order.
pub fn joining_tick_packets(rate: &TickRateManager) -> [Packet; 2] {
    [ticking_state_packet(rate), ticking_step_packet(rate)]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SprintReport {
    pub ticks: i64,
    pub ticks_per_second: i32,
    pub milliseconds_per_tick: f64,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TickRateEffect {
    StateToClients,
    StepTicksToClients,
    RateChanged,
    SprintFinished(SprintReport),
}
/// ServerTickRateManager lifecycle. Monotonic nanoseconds are injected so the
/// sprint state can be reproduced independently of host scheduling. Effects
/// preserve original notification order; packet encoding belongs to callers.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ServerTickRateManager {
    pub rate: TickRateManager,
    pub remaining_sprint_ticks: i64,
    pub sprint_tick_start_nanos: i64,
    pub sprint_time_spent_nanos: i64,
    pub scheduled_sprint_ticks: i64,
    pub previous_frozen: bool,
}
impl ServerTickRateManager {
    pub fn is_sprinting(&self) -> bool {
        self.scheduled_sprint_ticks > 0
    }
    pub fn set_frozen(&mut self, frozen: bool, mut effect: impl FnMut(TickRateEffect)) {
        self.rate.set_frozen(frozen);
        effect(TickRateEffect::StateToClients);
    }
    pub fn set_tick_rate(&mut self, rate: f32, mut effect: impl FnMut(TickRateEffect)) {
        self.rate.set_tick_rate(rate);
        effect(TickRateEffect::RateChanged);
        effect(TickRateEffect::StateToClients);
    }
    pub fn step_game_if_paused(
        &mut self,
        ticks: i32,
        mut effect: impl FnMut(TickRateEffect),
    ) -> bool {
        if !self.rate.is_frozen() {
            return false;
        }
        self.rate.set_frozen_ticks_to_run(ticks);
        effect(TickRateEffect::StepTicksToClients);
        true
    }
    pub fn stop_stepping(&mut self, mut effect: impl FnMut(TickRateEffect)) -> bool {
        if self.rate.frozen_ticks_to_run() <= 0 {
            return false;
        }
        self.rate.set_frozen_ticks_to_run(0);
        effect(TickRateEffect::StepTicksToClients);
        true
    }
    pub fn request_sprint(&mut self, ticks: i32, effect: impl FnMut(TickRateEffect)) -> bool {
        let was_sprinting = self.remaining_sprint_ticks > 0;
        self.sprint_time_spent_nanos = 0;
        self.scheduled_sprint_ticks = ticks as i64;
        self.remaining_sprint_ticks = ticks as i64;
        self.previous_frozen = self.rate.is_frozen();
        self.set_frozen(false, effect);
        was_sprinting
    }
    fn finish_sprint(&mut self, mut effect: impl FnMut(TickRateEffect)) {
        let ticks = self
            .scheduled_sprint_ticks
            .wrapping_sub(self.remaining_sprint_ticks);
        let milliseconds = self.sprint_time_spent_nanos.max(1) as f64 / 1_000_000.0;
        let ticks_per_second = (1000i64.wrapping_mul(ticks) as f64 / milliseconds) as i32;
        let milliseconds_per_tick = if ticks == 0 {
            self.rate.milliseconds_per_tick() as f64
        } else {
            milliseconds / ticks as f64
        };
        self.scheduled_sprint_ticks = 0;
        self.sprint_time_spent_nanos = 0;
        effect(TickRateEffect::SprintFinished(SprintReport {
            ticks,
            ticks_per_second,
            milliseconds_per_tick,
        }));
        self.remaining_sprint_ticks = 0;
        self.set_frozen(self.previous_frozen, &mut effect);
        effect(TickRateEffect::RateChanged);
    }
    pub fn stop_sprinting(&mut self, effect: impl FnMut(TickRateEffect)) -> bool {
        if self.remaining_sprint_ticks <= 0 {
            return false;
        }
        self.finish_sprint(effect);
        true
    }
    /// The server calls this while sprinting. The original method finishes a
    /// sprint whenever runsNormally and remaining<=0, even on a direct call
    /// without a scheduled sprint; preserve that behavior for API parity.
    pub fn check_should_sprint(
        &mut self,
        now_nanos: i64,
        effect: impl FnMut(TickRateEffect),
    ) -> bool {
        if !self.rate.runs_normally() {
            return false;
        }
        if self.remaining_sprint_ticks > 0 {
            self.sprint_tick_start_nanos = now_nanos;
            self.remaining_sprint_ticks = self.remaining_sprint_ticks.wrapping_sub(1);
            true
        } else {
            self.finish_sprint(effect);
            false
        }
    }
    pub fn end_tick_work(&mut self, now_nanos: i64) {
        self.sprint_time_spent_nanos = self
            .sprint_time_spent_nanos
            .wrapping_add(now_nanos.wrapping_sub(self.sprint_tick_start_nanos));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickStart {
    pub tick_count: i32,
    pub synchronize_time: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerTickSchedule {
    pub tick_count: i32,
    pub ticks_until_autosave: i32,
}
impl Default for ServerTickSchedule {
    fn default() -> Self {
        Self {
            tick_count: 0,
            ticks_until_autosave: 6000,
        }
    }
}
impl ServerTickSchedule {
    /// Call before TickRateManager.tick and all tickChildren work. Time packets
    /// use this server counter, independently of persisted world age/daytime.
    pub fn begin_tick(&mut self) -> TickStart {
        self.tick_count = self.tick_count.wrapping_add(1);
        TickStart {
            tick_count: self.tick_count,
            synchronize_time: self.tick_count % TIME_SYNC_INTERVAL == 0,
        }
    }
    /// Call after tickChildren and the optional status refresh. A true result
    /// requests saveEverything(suppressLog=true,flush=false,forced=false).
    pub fn finish_tick(&mut self, rate: AutosaveRate) -> bool {
        self.ticks_until_autosave = self.ticks_until_autosave.wrapping_sub(1);
        if self.ticks_until_autosave <= 0 {
            self.ticks_until_autosave = autosave_interval(rate);
            true
        } else {
            false
        }
    }
    /// A lower interval shortens an outstanding countdown; increasing tick rate
    /// does not postpone the already scheduled next save.
    pub fn on_tick_rate_changed(&mut self, rate: AutosaveRate) {
        let interval = autosave_interval(rate);
        if interval < self.ticks_until_autosave {
            self.ticks_until_autosave = interval;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerPhase {
    PlayerPositionSync,
    CommandFunctions,
    TimeSync(usize),
    LevelTick(usize),
    Connections,
    PlayerList,
    DevelopmentTests,
    GuiRefresh,
    SendChunks,
}
pub trait ServerChildrenHooks {
    /// Preserve MinecraftServer.getAllLevels iteration order. This is queried
    /// after command functions, because those run before dimension iteration.
    fn levels(&mut self) -> Vec<usize>;
    fn phase(&mut self, phase: ServerPhase);
    fn development_tests_enabled(&mut self) -> bool {
        false
    }
}
/// MinecraftServer.tickChildren ordering. Actor iteration inside aggregate
/// phases (position sync, connections, GUI and chunk flush) belongs to hooks.
pub fn tick_children(start: TickStart, hooks: &mut impl ServerChildrenHooks) {
    hooks.phase(ServerPhase::PlayerPositionSync);
    hooks.phase(ServerPhase::CommandFunctions);
    for level in hooks.levels() {
        if start.synchronize_time {
            hooks.phase(ServerPhase::TimeSync(level));
        }
        hooks.phase(ServerPhase::LevelTick(level));
    }
    hooks.phase(ServerPhase::Connections);
    hooks.phase(ServerPhase::PlayerList);
    if hooks.development_tests_enabled() {
        hooks.phase(ServerPhase::DevelopmentTests);
    }
    hooks.phase(ServerPhase::GuiRefresh);
    hooks.phase(ServerPhase::SendChunks);
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SleepPlayer {
    pub spectator: bool,
    pub sleeping: bool,
    pub deeply_sleeping: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SleepStatus {
    pub active_players: i32,
    pub sleeping_players: i32,
}
impl SleepStatus {
    pub fn sleepers_needed(self, percentage: i32) -> i32 {
        let numerator = self.active_players.wrapping_mul(percentage) as f32;
        let value = numerator / 100.0;
        let n = value as i32;
        let ceil = if value > n as f32 {
            n.wrapping_add(1)
        } else {
            n
        };
        ceil.max(1)
    }
    pub fn enough_sleeping(self, percentage: i32) -> bool {
        self.sleeping_players >= self.sleepers_needed(percentage)
    }
    pub fn enough_deep_sleeping(
        self,
        percentage: i32,
        players: impl IntoIterator<Item = SleepPlayer>,
    ) -> bool {
        let count = players.into_iter().filter(|p| p.deeply_sleeping).count() as i32;
        count >= self.sleepers_needed(percentage)
    }
    pub fn remove_sleepers(&mut self) {
        self.sleeping_players = 0;
    }
    pub fn update(&mut self, players: impl IntoIterator<Item = SleepPlayer>) -> bool {
        let old = *self;
        self.active_players = 0;
        self.sleeping_players = 0;
        for p in players {
            if !p.spectator {
                self.active_players = self.active_players.wrapping_add(1);
                if p.sleeping {
                    self.sleeping_players = self.sleeping_players.wrapping_add(1);
                }
            }
        }
        (old.sleeping_players > 0 || self.sleeping_players > 0) && (old != *self)
    }
}
/// Java advances to the next 24000 boundary using signed remainder, including
/// negative time and wrapping long arithmetic.
pub fn next_sleep_day(day_time: i64) -> i64 {
    let candidate = day_time.wrapping_add(24000);
    candidate.wrapping_sub(candidate % 24000)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelPhase {
    BeginHandlingTicks,
    WorldBorder,
    Weather,
    SleepDaySkip,
    WakePlayers,
    ResetWeather,
    SkyBrightness,
    AdvanceGameTime,
    /// Pass the newly incremented game time captured during AdvanceGameTime
    /// to TimerQueue.tick; later callbacks can mutate the live world clock.
    ScheduledFunctions,
    AdvanceDayTime,
    ScheduledBlocks,
    ScheduledFluids,
    Raids,
    Chunks,
    BlockEvents,
    EndHandlingTicks,
    ResetEmptyTime,
    DragonFight,
    Entities,
    BlockEntities,
    EntityManagement,
}
impl LevelPhase {
    pub fn name(self) -> &'static str {
        match self {
            Self::BeginHandlingTicks => "begin",
            Self::WorldBorder => "border",
            Self::Weather => "weather",
            Self::SleepDaySkip => "sleep_day",
            Self::WakePlayers => "wake",
            Self::ResetWeather => "reset_weather",
            Self::SkyBrightness => "sky",
            Self::AdvanceGameTime => "game_time",
            Self::ScheduledFunctions => "functions",
            Self::AdvanceDayTime => "day_time",
            Self::ScheduledBlocks => "blocks",
            Self::ScheduledFluids => "fluids",
            Self::Raids => "raids",
            Self::Chunks => "chunks",
            Self::BlockEvents => "block_events",
            Self::EndHandlingTicks => "end",
            Self::ResetEmptyTime => "reset_empty",
            Self::DragonFight => "dragon",
            Self::Entities => "entities",
            Self::BlockEntities => "block_entities",
            Self::EntityManagement => "entity_management",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelSettings {
    pub run_game_elements: bool,
    pub debug_world: bool,
    pub tick_time: bool,
    pub daylight_cycle: bool,
    pub enough_sleeping: bool,
    pub enough_deep_sleeping: bool,
    pub weather_cycle: bool,
    pub raining: bool,
    pub has_players: bool,
    pub has_forced_chunks: bool,
    pub has_dragon_fight: bool,
}
impl Default for LevelSettings {
    fn default() -> Self {
        Self {
            run_game_elements: true,
            debug_world: false,
            tick_time: true,
            daylight_cycle: true,
            enough_sleeping: false,
            enough_deep_sleeping: false,
            weather_cycle: true,
            raining: false,
            has_players: true,
            has_forced_chunks: false,
            has_dragon_fight: false,
        }
    }
}
/// Read settings at their original phase boundary: weather and scheduled
/// functions can affect state consulted later in the same level tick.
pub trait LevelTickHooks {
    fn settings(&mut self) -> LevelSettings;
    fn phase(&mut self, phase: LevelPhase);
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelTickState {
    pub empty_time: i32,
    pub handling_ticks: bool,
}
impl LevelTickState {
    pub fn tick(&mut self, hooks: &mut impl LevelTickHooks) {
        self.handling_ticks = true;
        hooks.phase(LevelPhase::BeginHandlingTicks);
        let running = hooks.settings().run_game_elements;
        if running {
            hooks.phase(LevelPhase::WorldBorder);
            hooks.phase(LevelPhase::Weather);
        }
        let settings = hooks.settings();
        // Sleep completion is deliberately independent of the freeze gate.
        if settings.enough_sleeping && settings.enough_deep_sleeping {
            if hooks.settings().daylight_cycle {
                hooks.phase(LevelPhase::SleepDaySkip);
            }
            hooks.phase(LevelPhase::WakePlayers);
            let settings = hooks.settings();
            if settings.weather_cycle && settings.raining {
                hooks.phase(LevelPhase::ResetWeather);
            }
        }
        hooks.phase(LevelPhase::SkyBrightness);
        if running && hooks.settings().tick_time {
            hooks.phase(LevelPhase::AdvanceGameTime);
            hooks.phase(LevelPhase::ScheduledFunctions);
            if hooks.settings().daylight_cycle {
                hooks.phase(LevelPhase::AdvanceDayTime);
            }
        }
        if running && !hooks.settings().debug_world {
            hooks.phase(LevelPhase::ScheduledBlocks);
            hooks.phase(LevelPhase::ScheduledFluids);
        }
        if running {
            hooks.phase(LevelPhase::Raids);
        }
        hooks.phase(LevelPhase::Chunks);
        if running {
            hooks.phase(LevelPhase::BlockEvents);
        }
        self.handling_ticks = false;
        hooks.phase(LevelPhase::EndHandlingTicks);
        let settings = hooks.settings();
        let active = settings.has_players || settings.has_forced_chunks;
        if active {
            self.empty_time = 0;
            hooks.phase(LevelPhase::ResetEmptyTime);
        }
        let tick_entities = if active {
            true
        } else {
            let old = self.empty_time;
            self.empty_time = self.empty_time.wrapping_add(1);
            old < EMPTY_TIME_NO_ENTITY_TICKS
        };
        if tick_entities {
            if running && hooks.settings().has_dragon_fight {
                hooks.phase(LevelPhase::DragonFight);
            }
            // Entity iteration still runs when frozen: TickRateManager filters
            // each entity, allowing players/player-controlled entities through.
            hooks.phase(LevelPhase::Entities);
            // This includes maintenance when frozen; actual BE ticker callbacks
            // additionally require runsNormally and shouldTickBlocksAt(position).
            hooks.phase(LevelPhase::BlockEntities);
        }
        hooks.phase(LevelPhase::EntityManagement);
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelTickPlan {
    pub phases: Vec<LevelPhase>,
    pub resulting_state: LevelTickState,
}
impl LevelTickPlan {
    /// Snapshot plan for fixed settings. Use LevelTickState.tick with live hooks
    /// if callbacks mutate settings (e.g. functions change doDaylightCycle).
    pub fn new(mut state: LevelTickState, settings: LevelSettings) -> Self {
        struct Recorder {
            settings: LevelSettings,
            phases: Vec<LevelPhase>,
        }
        impl LevelTickHooks for Recorder {
            fn settings(&mut self) -> LevelSettings {
                self.settings
            }
            fn phase(&mut self, p: LevelPhase) {
                self.phases.push(p);
            }
        }
        let mut recorder = Recorder {
            settings,
            phases: Vec::new(),
        };
        state.tick(&mut recorder);
        Self {
            phases: recorder.phases,
            resulting_state: state,
        }
    }
}
/// Apply only the three clock-mutating level phases. Scheduled timer callbacks
/// run between game-time advancement and the later daylight-cycle decision.
pub fn apply_clock_phase(clock: &mut TickClock, phase: LevelPhase) {
    match phase {
        LevelPhase::SleepDaySkip => clock.day_time = next_sleep_day(clock.day_time),
        LevelPhase::AdvanceGameTime => clock.game_time = clock.game_time.wrapping_add(1),
        LevelPhase::AdvanceDayTime => clock.day_time = clock.day_time.wrapping_add(1),
        _ => {}
    }
}

/// Environment-only phase adapter. Publish returned weather changes in their
/// original order and route each audience as specified by WeatherState. The
/// sleep rain gate uses interpolated is_raining(), rather than the rain flag.
pub fn apply_environment_phase(
    clock: &mut TickClock,
    weather: &mut WeatherState,
    phase: LevelPhase,
    has_skylight: bool,
    weather_cycle: bool,
    sample_inclusive: impl FnMut(i32, i32) -> i32,
) -> Vec<WeatherChange> {
    match phase {
        LevelPhase::Weather => weather.advance(has_skylight, weather_cycle, sample_inclusive),
        LevelPhase::ResetWeather => {
            weather.reset_weather_cycle();
            Vec::new()
        }
        _ => {
            apply_clock_phase(clock, phase);
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    fn i(q: &Value, name: &str) -> i32 {
        q[name].as_i64().unwrap() as i32
    }
    fn b(q: &Value, name: &str) -> bool {
        q[name].as_bool().unwrap()
    }
    fn settings(q: &Value) -> LevelSettings {
        LevelSettings {
            run_game_elements: b(q, "run_game_elements"),
            debug_world: b(q, "debug_world"),
            tick_time: b(q, "tick_time"),
            daylight_cycle: b(q, "daylight_cycle"),
            enough_sleeping: b(q, "enough_sleeping"),
            enough_deep_sleeping: b(q, "enough_deep_sleeping"),
            weather_cycle: b(q, "weather_cycle"),
            raining: b(q, "raining"),
            has_players: b(q, "has_players"),
            has_forced_chunks: b(q, "has_forced_chunks"),
            has_dragon_fight: b(q, "has_dragon_fight"),
        }
    }
    struct Hooks<'a> {
        q: &'a Value,
        settings: LevelSettings,
        clock: TickClock,
        phases: Vec<&'static str>,
        timer_time: i64,
    }
    impl LevelTickHooks for Hooks<'_> {
        fn settings(&mut self) -> LevelSettings {
            self.settings
        }
        fn phase(&mut self, phase: LevelPhase) {
            self.phases.push(phase.name());
            apply_clock_phase(&mut self.clock, phase);
            if phase == LevelPhase::ScheduledFunctions {
                self.timer_time = self.clock.game_time;
                if let Some(value) = self.q.get("daylight_after_functions") {
                    self.settings.daylight_cycle = value.as_bool().unwrap();
                }
                if let Some(value) = self.q.get("game_time_after_functions") {
                    self.clock.game_time = value.as_i64().unwrap();
                }
                if let Some(value) = self.q.get("day_time_after_functions") {
                    self.clock.day_time = value.as_i64().unwrap();
                }
            }
        }
    }
    #[test]
    fn actual_java_phase_counter_autosave_and_sleep_vectors() {
        let vectors: Value =
            serde_json::from_str(include_str!("../tools/simulation_vectors.json")).unwrap();
        for (index, case) in vectors["cases"].as_array().unwrap().iter().enumerate() {
            let q = &case["query"];
            let actual = match q["op"].as_str().unwrap() {
                "level" => {
                    let s = settings(q);
                    let mut hooks = Hooks {
                        q,
                        settings: s,
                        clock: TickClock::new(
                            q["game_time"].as_i64().unwrap(),
                            q["day_time"].as_i64().unwrap(),
                            s.daylight_cycle,
                        ),
                        phases: Vec::new(),
                        timer_time: i64::MIN,
                    };
                    let mut state = LevelTickState {
                        empty_time: i(q, "empty_time"),
                        handling_ticks: false,
                    };
                    state.tick(&mut hooks);
                    json!({"phases":hooks.phases,"empty_time":state.empty_time,"handling":state.handling_ticks,"game_time":hooks.clock.game_time,"day_time":hooks.clock.day_time,"timer_time":hooks.timer_time})
                }
                "children" => {
                    struct Children<'a> {
                        level: Hooks<'a>,
                        state: LevelTickState,
                        phases: Vec<String>,
                    }
                    impl ServerChildrenHooks for Children<'_> {
                        fn levels(&mut self) -> Vec<usize> {
                            vec![0]
                        }
                        fn phase(&mut self, phase: ServerPhase) {
                            match phase {
                                ServerPhase::PlayerPositionSync => {}
                                ServerPhase::CommandFunctions => {
                                    self.phases.push("server_functions".into())
                                }
                                ServerPhase::TimeSync(_) => {
                                    let payload = self.level.clock.time_update_payload();
                                    self.phases.push(format!(
                                        "time:{}:{}",
                                        i64::from_be_bytes(payload[..8].try_into().unwrap()),
                                        i64::from_be_bytes(payload[8..].try_into().unwrap())
                                    ));
                                }
                                ServerPhase::LevelTick(_) => {
                                    self.state.tick(&mut self.level);
                                    self.phases
                                        .extend(self.level.phases.iter().map(|v| (*v).to_owned()));
                                }
                                ServerPhase::Connections => self.phases.push("connections".into()),
                                ServerPhase::PlayerList => self.phases.push("player_list".into()),
                                ServerPhase::GuiRefresh => self.phases.push("gui".into()),
                                ServerPhase::SendChunks => self.phases.push("send_chunks".into()),
                                ServerPhase::DevelopmentTests => panic!(),
                            }
                        }
                    }
                    let s = settings(q);
                    let mut hooks = Children {
                        level: Hooks {
                            q,
                            settings: s,
                            clock: TickClock::new(
                                q["game_time"].as_i64().unwrap(),
                                q["day_time"].as_i64().unwrap(),
                                s.daylight_cycle,
                            ),
                            phases: Vec::new(),
                            timer_time: i64::MIN,
                        },
                        state: LevelTickState {
                            empty_time: i(q, "empty_time"),
                            handling_ticks: false,
                        },
                        phases: Vec::new(),
                    };
                    let mut schedule = ServerTickSchedule {
                        tick_count: i(q, "tick_count").wrapping_sub(1),
                        ..ServerTickSchedule::default()
                    };
                    tick_children(schedule.begin_tick(), &mut hooks);
                    json!({"phases":hooks.phases,"game_time":hooks.level.clock.game_time,"day_time":hooks.level.clock.day_time})
                }
                "counter" => {
                    let mut schedule = ServerTickSchedule {
                        tick_count: i(q, "tick_count"),
                        ticks_until_autosave: i(q, "until_save"),
                    };
                    schedule.begin_tick();
                    let rate = AutosaveRate {
                        tick_rate: f32::from_bits(
                            u32::from_str_radix(q["rate_bits"].as_str().unwrap(), 16).unwrap(),
                        ),
                        sprinting: b(q, "sprinting"),
                        average_tick_nanos: q["average_nanos"].as_i64().unwrap(),
                    };
                    let saved = schedule.finish_tick(rate);
                    let phases = if saved {
                        vec!["children", "autosave"]
                    } else {
                        vec!["children"]
                    };
                    json!({"tick_count":schedule.tick_count,"until_save":schedule.ticks_until_autosave,"phases":phases})
                }
                "rate_packets" => {
                    let mut rate = TickRateManager::new();
                    rate.set_tick_rate(f32::from_bits(
                        u32::from_str_radix(q["rate_bits"].as_str().unwrap(), 16).unwrap(),
                    ));
                    rate.set_frozen(b(q, "frozen"));
                    rate.set_frozen_ticks_to_run(i(q, "steps"));
                    json!(joining_tick_packets(&rate).into_iter().map(|packet|json!({"id":packet.id,"payload":packet.payload.iter().map(|b|format!("{b:02x}")).collect::<String>()})).collect::<Vec<_>>())
                }
                "rate" => {
                    let mut manager = ServerTickRateManager::default();
                    let mut schedule = ServerTickSchedule {
                        tick_count: i(q, "tick_count"),
                        ticks_until_autosave: i(q, "until_save"),
                    };
                    let mut out = Vec::new();
                    for (step, observed) in q["steps"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .zip(case["expected"].as_array().unwrap())
                    {
                        let clock = observed["clock"].as_i64().unwrap();
                        let mut effects = Vec::new();
                        let mut callback = |effect| effects.push(effect);
                        let mut returned = None;
                        match step[0].as_str().unwrap() {
                            "freeze" => {
                                manager.set_frozen(step[1].as_bool().unwrap(), &mut callback)
                            }
                            "step" => {
                                returned = Some(manager.step_game_if_paused(
                                    step[1].as_i64().unwrap() as i32,
                                    &mut callback,
                                ))
                            }
                            "stopstep" => returned = Some(manager.stop_stepping(&mut callback)),
                            "request" => {
                                returned = Some(manager.request_sprint(
                                    step[1].as_i64().unwrap() as i32,
                                    &mut callback,
                                ))
                            }
                            "check" => {
                                returned = Some(manager.check_should_sprint(clock, &mut callback))
                            }
                            "work" => manager.end_tick_work(clock),
                            "tick" => manager.rate.tick(),
                            "rate" => manager.set_tick_rate(
                                f32::from_bits(
                                    u32::from_str_radix(step[1].as_str().unwrap(), 16).unwrap(),
                                ),
                                &mut callback,
                            ),
                            "stop" => returned = Some(manager.stop_sprinting(&mut callback)),
                            "spent" => manager.sprint_time_spent_nanos = step[1].as_i64().unwrap(),
                            _ => panic!(),
                        }
                        let effects = effects
                            .into_iter()
                            .map(|effect| match effect {
                                TickRateEffect::StateToClients => "state".into(),
                                TickRateEffect::StepTicksToClients => "steps".into(),
                                TickRateEffect::SprintFinished(report) => {
                                    format!("report:{}", report.ticks_per_second)
                                }
                                TickRateEffect::RateChanged => {
                                    schedule.on_tick_rate_changed(AutosaveRate {
                                        tick_rate: manager.rate.tick_rate(),
                                        sprinting: manager.is_sprinting(),
                                        average_tick_nanos: q["average_nanos"].as_i64().unwrap(),
                                    });
                                    "rate_changed".into()
                                }
                            })
                            .collect::<Vec<String>>();
                        let ratebits = manager.rate.tick_rate();
                        out.push(json!({"effects":effects,"returned":returned,"clock":clock,"state":[manager.remaining_sprint_ticks,manager.scheduled_sprint_ticks,manager.sprint_time_spent_nanos,manager.previous_frozen,manager.rate.is_frozen(),manager.rate.frozen_ticks_to_run(),manager.rate.runs_normally(),format!("{:08x}",if ratebits.is_nan(){0x7fc00000}else{ratebits.to_bits()}),manager.rate.nanoseconds_per_tick(),schedule.ticks_until_autosave]}));
                    }
                    for (step, (actual, expected)) in out
                        .iter()
                        .zip(case["expected"].as_array().unwrap())
                        .enumerate()
                    {
                        assert_eq!(
                            actual, expected,
                            "rate query{index} step{step}: {}",
                            q["steps"][step]
                        );
                    }
                    json!(out)
                }
                "sleep_update" => {
                    let players = q["players"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|p| SleepPlayer {
                            spectator: p[0].as_bool().unwrap(),
                            sleeping: p[1].as_bool().unwrap(),
                            deeply_sleeping: p[2].as_bool().unwrap(),
                        })
                        .collect::<Vec<_>>();
                    let mut status = SleepStatus {
                        active_players: i(q, "active"),
                        sleeping_players: i(q, "sleeping"),
                    };
                    let changed = status.update(players.iter().copied());
                    if b(q, "remove") {
                        status.remove_sleepers();
                    }
                    let percentage = i(q, "percentage");
                    json!({"active":status.active_players,"sleeping":status.sleeping_players,"changed":changed,"needed":status.sleepers_needed(percentage),"enough":status.enough_sleeping(percentage),"deep":status.enough_deep_sleeping(percentage,players)})
                }
                "sleep" => {
                    let status = SleepStatus {
                        active_players: i(q, "active"),
                        sleeping_players: i(q, "sleeping"),
                    };
                    json!([
                        status.sleepers_needed(i(q, "percentage")),
                        status.enough_sleeping(i(q, "percentage"))
                    ])
                }
                _ => panic!(),
            };
            assert_eq!(
                actual, case["expected"],
                "actual Java simulation case{index}: {q}"
            );
        }
    }
    #[test]
    fn time_sync_uses_server_counter_and_pre_level_clock() {
        let mut schedule = ServerTickSchedule {
            tick_count: 18,
            ..ServerTickSchedule::default()
        };
        let mut clock = TickClock::new(1005, 23999, true);
        assert!(!schedule.begin_tick().synchronize_time);
        clock.advance();
        assert!(schedule.begin_tick().synchronize_time);
        assert_eq!(clock.game_time, 1006);
        let before = clock.time_update_payload();
        clock.advance();
        assert_ne!(before, clock.time_update_payload());
    }
    #[test]
    fn live_function_gamerule_changes_and_empty_window() {
        let settings = LevelSettings {
            run_game_elements: false,
            has_players: false,
            has_forced_chunks: false,
            enough_sleeping: true,
            enough_deep_sleeping: true,
            ..LevelSettings::default()
        };
        let plan = LevelTickPlan::new(
            LevelTickState {
                empty_time: 299,
                handling_ticks: false,
            },
            settings,
        );
        assert!(plan.phases.contains(&LevelPhase::SleepDaySkip));
        assert!(!plan.phases.contains(&LevelPhase::AdvanceGameTime));
        assert!(plan.phases.contains(&LevelPhase::Entities));
        assert_eq!(plan.resulting_state.empty_time, 300);
        let next = LevelTickPlan::new(plan.resulting_state, settings);
        assert!(!next.phases.contains(&LevelPhase::Entities));
        assert!(next.phases.contains(&LevelPhase::EntityManagement));
    }
}
