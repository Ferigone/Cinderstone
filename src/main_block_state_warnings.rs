//! Real native diagnostic sink for the source NbtUtils property warning.
//!
//! The envelope is native JSON, not a claim of Log4j console-layout parity.
//! All source arguments and the interpolated message retain UTF-16 units.
//! Compact SNBT uses the actual source formatter; absent floating-number
//! support is an explicit unsupported capability, never approximate text.

use anyhow::{ensure, Context, Result};
use mc121_rust::{
    falling_block::{
        block_state_codec::INVALID_PROPERTY_WARNING,
        block_state_registry::BlockStateWarningSink,
    },
    nbt::JavaString,
    nbt_snbt::{self, SnbtNumberFormatter},
    source_nbt_decode::SourceCompoundView,
};
use serde_json::json;
use std::io::Write;

pub(super) struct MainBlockStateWarnings<'a> {
    numbers: Option<&'a mut dyn SnbtNumberFormatter>,
}
impl<'a> MainBlockStateWarnings<'a> {
    /// The caller lends a genuine Java Float/Double formatting owner. No
    /// alternative Rust-formatting or zero/default provider is installed.
    pub(super) fn with_numbers(numbers: &'a mut dyn SnbtNumberFormatter) -> Self {
        Self { numbers: Some(numbers) }
    }

    /// Explicitly bounded profile. The exact source formatter rejects an
    /// encountered Float/Double when its genuine number capability is absent.
    pub(super) fn without_floating() -> Self { Self { numbers: None } }
}

impl BlockStateWarningSink for MainBlockStateWarnings<'_> {
    fn warn_invalid_property(&mut self, message: &'static str,
        property: &JavaString, value: &JavaString, original: SourceCompoundView<'_>,
    ) -> Result<()> {
        ensure!(message == INVALID_PROPERTY_WARNING, "foreign BlockState property warning template");
        let snbt = match self.numbers.as_mut() {
            Some(numbers) => nbt_snbt::to_snbt(original.tag(), &mut **numbers)?,
            None => nbt_snbt::to_snbt_without_floating(original.tag())?,
        };
        let property_units = property.units();
        let value_units = value.units();
        let snbt_units = snbt.units();
        // Source SLF4J substitutes these three arguments once. Argument text
        // containing braces/backslashes is not interpreted as another format.
        let mut rendered = Vec::new();
        rendered.extend("Unable to read property: ".encode_utf16());
        rendered.extend_from_slice(property_units.as_ref());
        rendered.extend(" with value: ".encode_utf16());
        rendered.extend_from_slice(value_units.as_ref());
        rendered.extend(" for blockstate: ".encode_utf16());
        rendered.extend_from_slice(snbt_units.as_ref());
        let event = json!({
            "event": "block_state_property_warning",
            "level": "WARN",
            "envelope": "native-json",
            "message_template": message,
            "property_utf16": property_units.as_ref(),
            "value_utf16": value_units.as_ref(),
            "original_snbt_utf16": snbt_units.as_ref(),
            "message_utf16": rendered,
        });
        let mut encoded = serde_json::to_vec(&event)?;
        encoded.push(b'\n');
        std::io::stderr().lock().write_all(&encoded)
            .context("writing actual BlockState property warning to stderr")
    }
}
