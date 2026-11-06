//! Initial client border snapshot from the live, restored world-border owner.
use mc121_rust::{border::WorldBorder, protocol::{Packet, Writer}};

pub(super) fn initialize(border: &WorldBorder, now_ms: i64) -> Packet {
    let settings = border.settings(now_ms);
    let mut writer = Writer::new();
    writer.f64(settings.center_x).f64(settings.center_z)
        .f64(settings.size).f64(settings.size_lerp_target);
    // FriendlyByteBuf.writeVarLong uses unsigned shifts, including negatives.
    let mut remaining = settings.size_lerp_time as u64;
    loop {
        let byte = (remaining & 0x7f) as u8;
        remaining >>= 7;
        writer.u8(if remaining == 0 { byte } else { byte | 0x80 });
        if remaining == 0 { break; }
    }
    writer.varint(border.absolute_max_size())
        .varint(settings.warning_blocks).varint(settings.warning_time);
    Packet { id: 0x25, payload: writer.into_inner() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mc121_rust::border::BorderSettings;

    #[test]
    fn original_initialize_border_packets() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../fixtures/border-login-vectors.json"
        )).unwrap();
        for case in vectors.as_array().unwrap() {
            let mut border = WorldBorder::from_settings(BorderSettings::default(), 0);
            border.set_center(case["x"].as_f64().unwrap(), case["z"].as_f64().unwrap());
            border.set_size(case["from"].as_f64().unwrap());
            border.set_absolute_max_size(case["absolute"].as_i64().unwrap() as i32);
            border.set_warning_blocks(case["warning_blocks"].as_i64().unwrap() as i32);
            border.set_warning_time(case["warning_time"].as_i64().unwrap() as i32);
            let duration = case["duration"].as_i64().unwrap();
            if duration != 0 {
                border.lerp_size_between(case["from"].as_f64().unwrap(),
                    case["to"].as_f64().unwrap(), duration, 0);
            }
            let packet = initialize(&border, case["now"].as_i64().unwrap());
            let actual: String = packet.payload.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(actual, case["payload_hex"].as_str().unwrap(), "{case}");
        }
    }
}
