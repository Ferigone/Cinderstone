//! Command system, stage 1: the reader, syntax exceptions and primitive argument types of
//! Mojang's Brigadier 1.2.9 with the dedicated server's built-in exception messages.
//!
//! This is `commands_data.1` partial work (WP-E3, issue #24). Nothing here is wired to
//! Main: the dispatcher, Minecraft argument types, selectors and the command catalog are
//! later stages, and the existing bounded administration commands keep their own parser.
//!
//! Brigadier is MIT-licensed (`licenses/brigadier-MIT.txt`). See
//! `docs/reference/commands-brigadier.md` for the source identity and evidence.

pub mod arguments;
pub mod exception;
pub mod java_text;
pub mod string_reader;

pub use arguments::{
    ArgumentType, BoolArgumentType, DoubleArgumentType, FloatArgumentType, IntegerArgumentType,
    LongArgumentType, StringArgumentType, StringType,
};
pub use exception::{CommandArg, CommandMessage, CommandSyntaxException};
pub use string_reader::StringReader;

#[cfg(test)]
mod tests;
