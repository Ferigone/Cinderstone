//! Java 1.21 TimeCommand registration and single-level clock transaction.
//! The caller locks the actual clock; parsing never retains a world/player owner.
use super::{
    arguments::ArgumentValue,
    dispatcher::{ArgumentBuilder, CommandContext, CommandDispatcher},
    exception::CommandSyntaxException,
    simple_arguments::TimeArgumentType,
    suggestion::{SuggestionError, Suggestions},
};
use crate::tick::TickClock;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeResponse {
    pub result: i32,
    pub key: &'static str,
    pub value: i32,
    pub broadcast_to_ops: bool,
    pub day_time: Option<i64>,
}

#[derive(Clone)]
struct Source {
    permission: u8,
    game_time: i64,
    day_time: i64,
    response: Rc<RefCell<Option<TimeResponse>>>,
}
#[derive(Clone, Copy)]
enum Action { Set(i32), Add, DayTime, GameTime, Day }

fn run(context: &CommandContext<Source>, action: Action) -> i32 {
    let source = context.source();
    let amount = || match context.argument("time") {
        Some(ArgumentValue::Integer(value)) => *value,
        _ => unreachable!("TimeArgument supplies the registered integer argument"),
    };
    let (day_time, value, key, broadcast_to_ops) = match action {
        Action::Set(value) => (Some(i64::from(value)), value, "commands.time.set", true),
        Action::Add => {
            let day = source.day_time.wrapping_add(i64::from(amount()));
            (Some(day), (day % 24000) as i32, "commands.time.set", true)
        }
        Action::DayTime => (None, (source.day_time % 24000) as i32, "commands.time.query", false),
        Action::GameTime => (None, (source.game_time % i64::from(i32::MAX)) as i32, "commands.time.query", false),
        Action::Day => (None, ((source.day_time / 24000) % i64::from(i32::MAX)) as i32, "commands.time.query", false),
    };
    let result = day_time.map(|day| (day % 24000) as i32).unwrap_or(value);
    *source.response.borrow_mut() = Some(TimeResponse { result, key, value, broadcast_to_ops, day_time });
    result
}

fn dispatcher() -> CommandDispatcher<Source> {
    let mut dispatcher = CommandDispatcher::new();
    let mut set = ArgumentBuilder::literal("set");
    for (name, ticks) in [("day", 1000), ("noon", 6000), ("night", 13000), ("midnight", 18000)] {
        set = set.then(ArgumentBuilder::literal(name)
            .executes(Rc::new(move |context| Ok(run(context, Action::Set(ticks))))));
    }
    set = set.then(ArgumentBuilder::argument("time", Rc::new(TimeArgumentType::new()))
        .executes(Rc::new(|context| {
            let value = match context.argument("time") {
                Some(ArgumentValue::Integer(value)) => *value,
                _ => unreachable!("registered time argument"),
            };
            Ok(run(context, Action::Set(value)))
        })));
    let add = ArgumentBuilder::literal("add")
        .then(ArgumentBuilder::argument("time", Rc::new(TimeArgumentType::new()))
            .executes(Rc::new(|context| Ok(run(context, Action::Add)))));
    let mut query = ArgumentBuilder::literal("query");
    for (name, action) in [("daytime", Action::DayTime), ("gametime", Action::GameTime), ("day", Action::Day)] {
        query = query.then(ArgumentBuilder::literal(name)
            .executes(Rc::new(move |context| Ok(run(context, action)))));
    }
    dispatcher.register(ArgumentBuilder::literal("time")
        .requires(Rc::new(|source: &Source| source.permission >= 2))
        .then(set).then(add).then(query));
    dispatcher
}

fn source(permission: u8, game_time: i64, day_time: i64) -> Source {
    Source { permission, game_time, day_time, response: Rc::new(RefCell::new(None)) }
}

/// Execute one ordinary command while the host retains its actual clock lock.
/// A failed parse/permission check does not change any clock field.
pub fn execute(input: &str, permission: u8, clock: &mut TickClock) -> Result<TimeResponse, CommandSyntaxException> {
    let source = source(permission, clock.game_time, clock.day_time);
    dispatcher().execute_str(input, &source)?;
    let response = source.response.borrow_mut().take().expect("registered time command executed");
    if let Some(day_time) = response.day_time { clock.day_time = day_time; }
    Ok(response)
}

pub fn suggestions(input: &str, permission: u8) -> Result<Suggestions, SuggestionError> {
    if permission < 2 { return Ok(Suggestions::empty()); }
    let dispatcher = dispatcher();
    dispatcher.completion_suggestions_at_end(&dispatcher.parse(input, &source(permission, 0, 0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_server_execute_store_results_and_clock_states() {
        let cases: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/time-command-execution-vectors.json")).unwrap();
        for case in cases.as_array().unwrap() {
            let mut clock = TickClock::new(case["game_time"].as_i64().unwrap(), case["day_time"].as_i64().unwrap(), false);
            let result = execute(case["input"].as_str().unwrap(), 4, &mut clock).unwrap();
            assert_eq!(result.result, case["result"].as_i64().unwrap() as i32, "{case}");
            assert_eq!(clock.day_time, case["after_day_time"].as_i64().unwrap(), "{case}");
            assert_eq!(clock.game_time, case["game_time"].as_i64().unwrap());
            assert!(!clock.daylight_cycle);
        }
    }
    #[test]
    fn original_time_registration_parse_suggestions_and_permission_trees() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/time-command-vectors.json")).unwrap();
        for level in vectors.as_array().unwrap() {
            let permission = level["permission"].as_u64().unwrap() as u8;
            let dispatcher = dispatcher();
            let source = source(permission, 0, 0);
            let (tree, root) = super::super::tree_packet::usable_tree(dispatcher.tree(), dispatcher.root(), &source);
            let bytes = super::super::tree_packet::encode_commands_packet(&tree, root).unwrap();
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(hex, level["packet_hex"].as_str().unwrap());
            for case in level["cases"].as_array().unwrap() {
                let input = case["input"].as_str().unwrap();
                let parsed = dispatcher.parse(input, &source);
                assert_eq!(parsed.reader.cursor(), case["cursor"].as_u64().unwrap() as usize, "{case}");
                assert_eq!(parsed.reader.can_read(), case["remaining"].as_bool().unwrap(), "{case}");
                let context = parsed.context.build(input);
                assert_eq!(context.command().is_some(), case["executable"].as_bool().unwrap(), "{case}");
                let ticks = match context.argument("time") { Some(ArgumentValue::Integer(n)) => Some(*n), _ => None };
                assert_eq!(ticks, case["ticks"].as_i64().map(|n| n as i32), "{case}");
                let result = suggestions(input, permission).unwrap();
                assert_eq!(result.range().start(), case["suggestions"]["start"].as_u64().unwrap() as usize, "{case}");
                assert_eq!(result.range().end(), case["suggestions"]["end"].as_u64().unwrap() as usize, "{case}");
                let texts = result.list().iter().map(|s| s.text()).collect::<Vec<_>>();
                let expected = case["suggestions"]["texts"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect::<Vec<_>>();
                assert_eq!(texts, expected, "{case}");
            }
        }
    }
    #[test]
    fn clock_transaction_preserves_age_and_cycle_and_denies_before_mutation() {
        let mut clock = TickClock::new(i64::MAX, -24001, false);
        let before = clock;
        assert!(execute("time set day", 1, &mut clock).is_err());
        assert_eq!(clock, before);
        assert!(execute("time add -1", 4, &mut clock).is_err());
        assert_eq!(clock, before);
        assert_eq!(execute("time query day", 2, &mut clock).unwrap().result, -1);
        let set = execute("time set 2d", 2, &mut clock).unwrap();
        assert_eq!((set.result, set.value), (0, 48000));
        let add = execute("time add 0.5s", 2, &mut clock).unwrap();
        assert_eq!((clock.day_time, add.result, add.value), (48010, 10, 10));
        assert_eq!(clock.game_time, i64::MAX);
        assert!(!clock.daylight_cycle);
        clock.day_time = i64::MAX;
        execute("time add 1", 2, &mut clock).unwrap();
        assert_eq!(clock.day_time, i64::MIN);
    }
}
