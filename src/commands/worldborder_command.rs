// SPDX-License-Identifier: GPL-2.0-only
//! Java 1.21 WorldBorderCommand over an actual retained border and monotonic clock.
use super::{
    arguments::{
        ArgumentNetworkInfo, ArgumentType, ArgumentValue, DoubleArgumentType, FloatArgumentType,
        IntegerArgumentType,
    },
    coordinates::{CoordinateSource, Coordinates, Vec2ArgumentType},
    dispatcher::{ArgumentBuilder, CommandContext, CommandDispatcher},
    exception::{CommandArg, CommandMessage as SyntaxMessage, CommandSyntaxException},
    java_text::java_double_to_string,
    string_reader::StringReader,
    suggestion::{SuggestionError, Suggestions, SuggestionsBuilder},
};
use crate::{
    admission_commands::{CommandMessage, MessageArg},
    border::{WorldBorder, MAX_CENTER_COORDINATE, MAX_SIZE},
    geometry::Vec3,
    protocol::{Packet, Writer},
};
use std::{cell::RefCell, rc::Rc};
#[derive(Clone, Copy)]
enum Action {
    Get,
    Size {
        add: bool,
        distance: f64,
        time: Option<i32>,
    },
    Center(f64, f64),
    Warning(bool, i32),
    Damage(bool, f32),
}
#[derive(Clone)]
struct Source {
    permission: u8,
    position: Vec3,
    action: Rc<RefCell<Option<Action>>>,
}
fn source(permission: u8, position: Vec3) -> Source {
    Source {
        permission,
        position,
        action: Rc::new(RefCell::new(None)),
    }
}
fn record(ctx: &CommandContext<Source>, action: Action) -> Result<i32, CommandSyntaxException> {
    *ctx.source().action.borrow_mut() = Some(action);
    Ok(0)
}
fn integer(ctx: &CommandContext<Source>, name: &str) -> i32 {
    match ctx.argument(name) {
        Some(ArgumentValue::Integer(v)) => *v,
        _ => unreachable!(),
    }
}
// CommandSourceStack uses DEFAULT_GLOBAL relevant coordinates for this argument.
struct BorderPosition;
impl ArgumentType for BorderPosition {
    type Value = Coordinates;
    fn parse(&self, r: &mut StringReader) -> Result<Coordinates, CommandSyntaxException> {
        Vec2ArgumentType::new().parse(r)
    }
    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        Vec2ArgumentType::new().network_info()
    }
    fn list_suggestions<S>(
        &self,
        _: &CommandContext<S>,
        b: &mut SuggestionsBuilder,
    ) -> Result<Suggestions, CommandSyntaxException> {
        let remaining = b.remaining();
        if remaining.starts_with('^') {
            return Ok(Suggestions::empty());
        }
        let parts: Vec<_> = remaining.trim_end_matches(' ').split(' ').collect();
        let candidates = if remaining.is_empty() {
            vec!["~".to_owned(), "~ ~".to_owned()]
        } else if parts.len() == 1 {
            vec![format!("{} ~", parts[0])]
        } else {
            vec![]
        };
        for value in candidates {
            let full = if value == "~" { "~ ~" } else { &value };
            if self.parse(&mut StringReader::new(full)).is_ok() && value.starts_with(&remaining) {
                b.suggest(&value);
            }
        }
        Ok(b.build())
    }
}
fn dispatcher() -> CommandDispatcher<Source> {
    let mut d = CommandDispatcher::new();
    let mut root =
        ArgumentBuilder::literal("worldborder").requires(Rc::new(|s: &Source| s.permission >= 2));
    for (name, add) in [("add", true), ("set", false)] {
        root = root.then(
            ArgumentBuilder::literal(name).then(
                ArgumentBuilder::argument(
                    "distance",
                    Rc::new(DoubleArgumentType::new(-MAX_SIZE, MAX_SIZE)),
                )
                .executes(Rc::new(move |c: &CommandContext<Source>| {
                    let distance = match c.argument("distance") {
                        Some(ArgumentValue::Double(v)) => *v,
                        _ => unreachable!(),
                    };
                    record(
                        c,
                        Action::Size {
                            add,
                            distance,
                            time: None,
                        },
                    )
                }))
                .then(
                    ArgumentBuilder::argument(
                        "time",
                        Rc::new(IntegerArgumentType::new(0, i32::MAX)),
                    )
                    .executes(Rc::new(move |c: &CommandContext<Source>| {
                        let distance = match c.argument("distance") {
                            Some(ArgumentValue::Double(v)) => *v,
                            _ => unreachable!(),
                        };
                        record(
                            c,
                            Action::Size {
                                add,
                                distance,
                                time: Some(integer(c, "time")),
                            },
                        )
                    })),
                ),
            ),
        );
    }
    root = root.then(ArgumentBuilder::literal("center").then(
        ArgumentBuilder::argument("pos", Rc::new(BorderPosition)).executes(Rc::new(
            |c: &CommandContext<Source>| {
                let coords = match c.argument("pos") {
                    Some(ArgumentValue::Coordinates(v)) => v,
                    _ => unreachable!(),
                };
                let p = c.source().position;
                let at = coords.position(&CoordinateSource {
                    position: p,
                    anchored_position: p,
                    rotation: (0., 0.),
                });
                record(
                    c,
                    Action::Center(f64::from(at.x as f32), f64::from(at.z as f32)),
                )
            },
        )),
    ));
    let mut damage = ArgumentBuilder::literal("damage");
    for (name, arg, amount) in [
        ("amount", "damagePerBlock", true),
        ("buffer", "distance", false),
    ] {
        damage = damage.then(ArgumentBuilder::literal(name).then(
            ArgumentBuilder::argument(arg, Rc::new(FloatArgumentType::new(0., f32::MAX))).executes(
                Rc::new(move |c: &CommandContext<Source>| {
                    let value = match c.argument(arg) {
                        Some(ArgumentValue::Float(v)) => *v,
                        _ => unreachable!(),
                    };
                    record(c, Action::Damage(amount, value))
                }),
            ),
        ));
    }
    root = root.then(damage).then(
        ArgumentBuilder::literal("get")
            .executes(Rc::new(|c: &CommandContext<Source>| record(c, Action::Get))),
    );
    let mut warning = ArgumentBuilder::literal("warning");
    for (name, delay) in [("distance", false), ("time", true)] {
        warning = warning.then(
            ArgumentBuilder::literal(name).then(
                ArgumentBuilder::argument(name, Rc::new(IntegerArgumentType::new(0, i32::MAX)))
                    .executes(Rc::new(move |c: &CommandContext<Source>| {
                        record(c, Action::Warning(delay, integer(c, name)))
                    })),
            ),
        );
    }
    d.register(root.then(warning));
    d
}

/// Decimal half-up formatting of Java's binary-to-decimal digits. These commands
/// use Locale.ROOT fixed precision (0, 1 or 2); no locale/grouping formats occur.
fn fixed(value: f64, places: usize) -> String {
    let raw = java_double_to_string(value);
    if !value.is_finite() {
        return raw;
    }
    let negative = raw.starts_with('-');
    let raw = raw.trim_start_matches('-');
    let (mantissa, exponent) = raw
        .split_once('E')
        .map(|(m, e)| (m, e.parse::<i32>().unwrap()))
        .unwrap_or((raw, 0));
    let dot = mantissa.find('.').unwrap_or(mantissa.len()) as i32 + exponent;
    let mut digits: Vec<u8> = mantissa.bytes().filter(|&b| b != b'.').collect();
    let keep = dot + places as i32;
    let mut out = if keep < 0 {
        vec![b'0']
    } else {
        let k = keep as usize;
        let round = digits.get(k).is_some_and(|&v| v >= b'5');
        digits.resize(digits.len().max(k), b'0');
        digits.truncate(k);
        if round {
            let mut i = digits.len();
            loop {
                if i == 0 {
                    digits.insert(0, b'1');
                    break;
                }
                i -= 1;
                if digits[i] != b'9' {
                    digits[i] += 1;
                    break;
                }
                digits[i] = b'0';
            }
        }
        if digits.is_empty() {
            digits.push(b'0');
        }
        digits
    };
    while out.len() <= places {
        out.insert(0, b'0');
    }
    if places > 0 {
        out.insert(out.len() - places, b'.');
    }
    let text = String::from_utf8(out).unwrap();
    if negative {
        format!("-{text}")
    } else {
        text
    }
}
fn fail(key: &'static str, arg: Option<f64>) -> CommandSyntaxException {
    CommandSyntaxException::without_context(SyntaxMessage::new(
        key,
        arg.map(|v| vec![CommandArg::Double(v)]).unwrap_or_default(),
    ))
}
fn packet(id: i32, fill: impl FnOnce(&mut Writer)) -> Packet {
    let mut w = Writer::new();
    fill(&mut w);
    Packet {
        id,
        payload: w.into_inner(),
    }
}
fn varlong(w: &mut Writer, value: i64) {
    let mut v = value as u64;
    loop {
        let b = (v & 127) as u8;
        v >>= 7;
        w.u8(if v == 0 { b } else { b | 128 });
        if v == 0 {
            break;
        }
    }
}
pub struct BorderResponse {
    pub result: i32,
    pub message: CommandMessage,
    pub broadcast: bool,
    pub packets: Vec<Packet>,
}
pub fn execute(
    input: &str,
    permission: u8,
    position: Vec3,
    border: &mut WorldBorder,
    now: i64,
) -> Result<BorderResponse, CommandSyntaxException> {
    let s = source(permission, position);
    dispatcher().execute_str(input, &s)?;
    let action = s
        .action
        .borrow_mut()
        .take()
        .expect("registered border action");
    let old = border.settings(now);
    let (result, key, args, packets) = match action {
        Action::Get => (
            crate::entity_state::java_floor(old.size + 0.5),
            "commands.worldborder.get",
            vec![MessageArg::Text(fixed(old.size, 0))],
            vec![],
        ),
        Action::Center(x, z) => {
            if (x, z) == border.center() {
                return Err(fail("commands.worldborder.center.failed", None));
            }
            if x.abs() > MAX_CENTER_COORDINATE || z.abs() > MAX_CENTER_COORDINATE {
                return Err(fail(
                    "commands.worldborder.set.failed.far",
                    Some(MAX_CENTER_COORDINATE),
                ));
            }
            border.set_center(x, z);
            (
                0,
                "commands.worldborder.center.success",
                vec![MessageArg::Text(fixed(x, 2)), MessageArg::Text(fixed(z, 2))],
                vec![packet(0x4d, |w| {
                    w.f64(x).f64(z);
                })],
            )
        }
        Action::Warning(delay, value) => {
            let previous = if delay {
                old.warning_time
            } else {
                old.warning_blocks
            };
            if value == previous {
                return Err(fail(
                    if delay {
                        "commands.worldborder.warning.time.failed"
                    } else {
                        "commands.worldborder.warning.distance.failed"
                    },
                    None,
                ));
            }
            if delay {
                border.set_warning_time(value);
            } else {
                border.set_warning_blocks(value);
            }
            (
                value,
                if delay {
                    "commands.worldborder.warning.time.success"
                } else {
                    "commands.worldborder.warning.distance.success"
                },
                vec![MessageArg::Integer(value)],
                vec![packet(if delay { 0x50 } else { 0x51 }, |w| {
                    w.varint(value);
                })],
            )
        }
        Action::Damage(amount, value) => {
            let v = f64::from(value);
            let previous = if amount {
                old.damage_per_block
            } else {
                old.safe_zone
            };
            if v == previous {
                return Err(fail(
                    if amount {
                        "commands.worldborder.damage.amount.failed"
                    } else {
                        "commands.worldborder.damage.buffer.failed"
                    },
                    None,
                ));
            }
            if amount {
                border.set_damage_per_block(v);
            } else {
                border.set_safe_zone(v);
            }
            (
                value as i32,
                if amount {
                    "commands.worldborder.damage.amount.success"
                } else {
                    "commands.worldborder.damage.buffer.success"
                },
                vec![MessageArg::Text(fixed(v, 2))],
                vec![],
            )
        }
        Action::Size {
            add,
            distance,
            time,
        } => {
            let value = if add { old.size + distance } else { distance };
            let duration = time
                .map(|s| {
                    (i64::from(s) * 1000).wrapping_add(if add { old.size_lerp_time } else { 0 })
                })
                .unwrap_or(0);
            if value == old.size {
                return Err(fail("commands.worldborder.set.failed.nochange", None));
            }
            if value < 1. {
                return Err(fail("commands.worldborder.set.failed.small", None));
            }
            if value > MAX_SIZE {
                return Err(fail("commands.worldborder.set.failed.big", Some(MAX_SIZE)));
            }
            let mut args = vec![MessageArg::Text(fixed(value, 1))];
            let (key, p) = if duration > 0 {
                border.lerp_size_between(old.size, value, duration, now);
                args.push(MessageArg::Text((duration / 1000).to_string()));
                (
                    if value > old.size {
                        "commands.worldborder.set.grow"
                    } else {
                        "commands.worldborder.set.shrink"
                    },
                    packet(0x4e, |w| {
                        w.f64(old.size).f64(value);
                        varlong(w, duration);
                    }),
                )
            } else {
                border.set_size(value);
                (
                    "commands.worldborder.set.immediate",
                    packet(0x4f, |w| {
                        w.f64(value);
                    }),
                )
            };
            ((value - old.size) as i32, key, args, vec![p])
        }
    };
    Ok(BorderResponse {
        result,
        message: CommandMessage::new(key, args),
        broadcast: !matches!(action, Action::Get),
        packets,
    })
}
pub fn suggestions(input: &str, permission: u8) -> Result<Suggestions, SuggestionError> {
    if permission < 2 {
        return Ok(Suggestions::empty());
    }
    let d = dispatcher();
    d.completion_suggestions_at_end(&d.parse(input, &source(permission, Vec3::new(0., 0., 0.))))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|v| format!("{v:02x}")).collect()
    }
    #[test]
    fn executed_original_vectors() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../fixtures/worldborder-command-vectors.json"
        ))
        .unwrap();
        for level in fixture["grammar"].as_array().unwrap() {
            let permission = level["permission"].as_u64().unwrap() as u8;
            let d = dispatcher();
            let s = source(permission, Vec3::new(10.25, 65., -3.5));
            let (tree, root) = super::super::tree_packet::usable_tree(d.tree(), d.root(), &s);
            assert_eq!(
                hex(&super::super::tree_packet::encode_commands_packet(&tree, root).unwrap()),
                level["packet_hex"].as_str().unwrap()
            );
            for row in level["cases"].as_array().unwrap() {
                let v = suggestions(row["input"].as_str().unwrap(), permission).unwrap();
                assert_eq!(
                    serde_json::json!({"start":v.range().start(),"end":v.range().end(),"texts":v.list().iter().map(|v|v.text()).collect::<Vec<_>>()}),
                    row["suggestions"],
                    "{row}"
                );
            }
        }
        let mut border = WorldBorder::default();
        for row in fixture["execution"].as_array().unwrap() {
            let now = row["now"].as_i64().unwrap();
            border.tick(now);
            let response = execute(
                row["input"].as_str().unwrap(),
                row["permission"].as_u64().unwrap() as u8,
                Vec3::new(10.25, 65., -3.5),
                &mut border,
                now,
            );
            assert_eq!(response.is_ok(), row["ok"].as_bool().unwrap(), "{row}");
            match response {
                Ok(v) => {
                    assert_eq!(v.result, row["result"].as_i64().unwrap() as i32, "{row}");
                    let tag =
                        crate::nbt::Tag::decode_anonymous(&v.message.network_component().unwrap())
                            .unwrap();
                    let c = crate::components::text::Component::from_nbt(&tag)
                        .unwrap()
                        .to_network_nbt()
                        .unwrap();
                    assert_eq!(hex(&c), row["feedback_hex"].as_str().unwrap(), "{row}");
                    assert_eq!(
                        serde_json::Value::Array(
                            v.packets
                                .iter()
                                .map(|p| serde_json::json!({"id":p.id,"payload":hex(&p.payload)}))
                                .collect()
                        ),
                        row["packets"],
                        "{row}"
                    );
                }
                Err(e) => {
                    if let Some(key) = row["error_key"].as_str() {
                        let raw = e.raw_message();
                        assert_eq!(raw.key, key, "{row}");
                        let message = CommandMessage::new(
                            raw.key,
                            raw.args
                                .iter()
                                .map(|v| match v {
                                    CommandArg::Double(v) => MessageArg::Double(*v),
                                    CommandArg::Integer(v) => MessageArg::Integer(*v),
                                    _ => MessageArg::Text(v.java_to_string()),
                                })
                                .collect(),
                        );
                        let tag = crate::nbt::Tag::decode_anonymous(
                            &message.network_component().unwrap(),
                        )
                        .unwrap();
                        let c = crate::components::text::Component::from_nbt(&tag)
                            .unwrap()
                            .to_network_nbt()
                            .unwrap();
                        assert_eq!(hex(&c), row["error_hex"].as_str().unwrap(), "{row}");
                    }
                }
            }
            let s = border.settings(now);
            assert_eq!(
                serde_json::json!([
                    s.center_x,
                    s.center_z,
                    s.damage_per_block,
                    s.safe_zone,
                    s.warning_blocks as f64,
                    s.warning_time as f64,
                    s.size,
                    s.size_lerp_time as f64,
                    s.size_lerp_target
                ]),
                row["state"],
                "{row}"
            );
        }
    }
}
