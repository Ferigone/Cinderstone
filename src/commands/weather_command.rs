//! Java 1.21 WeatherCommand over the caller's retained weather and level RNG.
use super::{
    arguments::ArgumentValue,
    dispatcher::{ArgumentBuilder, CommandContext, CommandDispatcher},
    exception::CommandSyntaxException,
    simple_arguments::TimeArgumentType,
    suggestion::{SuggestionError, Suggestions},
};
use crate::{random::RandomSource, tick::WeatherState};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy)]
enum Kind { Clear, Rain, Thunder }
#[derive(Clone, Copy)]
struct Action { kind: Kind, duration: i32 }
#[derive(Clone)]
struct Source { permission: u8, action: Rc<RefCell<Option<Action>>> }

#[derive(Debug, PartialEq, Eq)]
pub struct WeatherResponse {
    /// The original returns the argument, including -1 when it samples a default.
    pub result: i32,
    pub key: &'static str,
}

fn source(permission: u8) -> Source {
    Source { permission, action: Rc::new(RefCell::new(None)) }
}

fn dispatcher() -> CommandDispatcher<Source> {
    let mut dispatcher = CommandDispatcher::new();
    let mut root = ArgumentBuilder::literal("weather")
        .requires(Rc::new(|source: &Source| source.permission >= 2));
    for (name, kind) in [("clear", Kind::Clear), ("rain", Kind::Rain), ("thunder", Kind::Thunder)] {
        root = root.then(ArgumentBuilder::literal(name)
            .executes(Rc::new(move |context: &CommandContext<Source>| {
                *context.source().action.borrow_mut() = Some(Action { kind, duration: -1 });
                Ok(-1)
            }))
            .then(ArgumentBuilder::argument("duration", Rc::new(TimeArgumentType::with_minimum(1)))
                .executes(Rc::new(move |context: &CommandContext<Source>| {
                    let duration = match context.argument("duration") {
                        Some(ArgumentValue::Integer(value)) => *value,
                        _ => unreachable!("registered TimeArgument"),
                    };
                    *context.source().action.borrow_mut() = Some(Action { kind, duration });
                    Ok(duration)
                }))));
    }
    dispatcher.register(root);
    dispatcher
}

/// Parse and authorize before taking any RNG draw or changing the actual owner.
pub fn execute(
    input: &str, permission: u8, weather: &mut WeatherState, random: &mut impl RandomSource,
) -> Result<WeatherResponse, CommandSyntaxException> {
    let source = source(permission);
    dispatcher().execute_str(input, &source)?;
    let action = source.action.borrow_mut().take().expect("registered weather command executed");
    let duration = if action.duration == -1 {
        // ServerLevel.RAIN_DELAY / RAIN_DURATION / THUNDER_DURATION; inclusive.
        let (min, max) = match action.kind {
            Kind::Clear => (12_000, 180_000),
            Kind::Rain => (12_000, 24_000),
            Kind::Thunder => (3_600, 15_600),
        };
        min + random.next_bounded_i32(max - min + 1).expect("positive vanilla weather interval")
    } else { action.duration };
    let key = match action.kind {
        Kind::Clear => { weather.set_weather_parameters(duration, 0, false, false); "commands.weather.set.clear" }
        Kind::Rain => { weather.set_weather_parameters(0, duration, true, false); "commands.weather.set.rain" }
        Kind::Thunder => { weather.set_weather_parameters(0, duration, true, true); "commands.weather.set.thunder" }
    };
    Ok(WeatherResponse { result: action.duration, key })
}

pub fn suggestions(input: &str, permission: u8) -> Result<Suggestions, SuggestionError> {
    if permission < 2 { return Ok(Suggestions::empty()); }
    let dispatcher = dispatcher();
    dispatcher.completion_suggestions_at_end(&dispatcher.parse(input, &source(permission)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::random::LegacyRandomSource;

    #[test]
    fn original_weather_registration_and_execution() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!("../../fixtures/weather-command-vectors.json")).unwrap();
        for level in vectors["grammar"].as_array().unwrap() {
            let permission = level["permission"].as_u64().unwrap() as u8;
            let dispatcher = dispatcher();
            let source = source(permission);
            let (tree, root) = super::super::tree_packet::usable_tree(dispatcher.tree(), dispatcher.root(), &source);
            let bytes = super::super::tree_packet::encode_commands_packet(&tree, root).unwrap();
            let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(hex, level["packet_hex"].as_str().unwrap());
            for row in level["cases"].as_array().unwrap() {
                let input = row["input"].as_str().unwrap();
                let parsed = dispatcher.parse(input, &source);
                assert_eq!(parsed.reader.cursor(), row["cursor"].as_u64().unwrap() as usize, "{row}");
                assert_eq!(parsed.reader.can_read(), row["remaining"].as_bool().unwrap(), "{row}");
                assert_eq!(parsed.context.build(input).command().is_some(), row["executable"].as_bool().unwrap(), "{row}");
                let suggestions = suggestions(input, permission).unwrap();
                assert_eq!(suggestions.range().start(), row["suggestions"]["start"].as_u64().unwrap() as usize, "{row}");
                assert_eq!(suggestions.range().end(), row["suggestions"]["end"].as_u64().unwrap() as usize, "{row}");
                let texts: Vec<_> = suggestions.list().iter().map(|value| value.text()).collect();
                let expected: Vec<_> = row["suggestions"]["texts"].as_array().unwrap().iter().map(|value| value.as_str().unwrap()).collect();
                assert_eq!(texts, expected, "{row}");
            }
        }
        for row in vectors["execution"].as_array().unwrap() {
            let mut weather = WeatherState { rain_level: 0.5, old_rain_level: 0.4, thunder_level: 0.3, old_thunder_level: 0.2, ..Default::default() };
            let mut random = LegacyRandomSource::new(row["seed"].as_i64().unwrap());
            let result = execute(row["input"].as_str().unwrap(), row["permission"].as_u64().unwrap() as u8, &mut weather, &mut random);
            assert_eq!(result.is_ok(), row["ok"].as_bool().unwrap(), "{row}");
            if let Ok(result) = result { assert_eq!(result.result, row["result"].as_i64().unwrap() as i32, "{row}"); }
            assert_eq!(serde_json::json!([weather.clear_weather_time, weather.rain_time, weather.thunder_time, weather.raining, weather.thundering]), row["state"], "{row}");
            assert_eq!(random.next_i32(), row["next_int"].as_i64().unwrap() as i32, "{row}");
            assert_eq!((weather.rain_level, weather.old_rain_level, weather.thunder_level, weather.old_thunder_level), (0.5, 0.4, 0.3, 0.2));
        }
    }
}
