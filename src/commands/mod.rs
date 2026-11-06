//! Command system, stages 1-4: the reader, syntax exceptions, primitive argument types and
//! the `CommandDispatcher` (tree, parse, execute, usage strings, completion suggestions) of
//! Mojang's Brigadier 1.2.9 with the dedicated server's built-in exception messages.
//!
//! This is `commands_data.1` partial work (WP-E3, issue #24). Nothing here is wired to
//! Main except the bounded `/time` and `/weather` paths: remaining argument types, selectors and the command catalog are later stages, and
//! the existing bounded administration commands keep their own parser.
//!
//! Brigadier is MIT-licensed (`licenses/brigadier-MIT.txt`). See
//! `docs/reference/commands-brigadier.md` for the source identity and evidence.

pub mod arguments;
pub mod coordinates;
pub mod dispatcher;
pub mod exception;
pub mod java_text;
pub mod operation;
pub mod ranges;
pub mod simple_arguments;
pub mod string_reader;
pub mod suggestion;
pub mod time_command;
pub mod weather_command;
pub mod tree_packet;

pub use arguments::{
    ArgumentType, ArgumentValue, ErasedArgumentType, BoolArgumentType, DoubleArgumentType, FloatArgumentType, IntegerArgumentType,
    LongArgumentType, StringArgumentType, StringType,
};
pub use coordinates::{
    BlockPosArgumentType, ColumnPosArgumentType, CoordinateSource, Coordinates, LocalCoordinates, Vec2ArgumentType,
    Vec3ArgumentType, WorldCoordinate, WorldCoordinates,
};
pub use dispatcher::{ArgumentBuilder, CommandContext, CommandDispatcher, ParseResults};
pub use exception::{CommandArg, CommandMessage, CommandSyntaxException};
pub use string_reader::StringReader;
pub use operation::{OperationArgumentType, ScoreOperation};
pub use ranges::{FloatRangeArgumentType, IntRangeArgumentType, MinMaxDoubles, MinMaxInts};
pub use simple_arguments::{
    AngleArgumentType, EntityAnchor, EntityAnchorArgumentType, JavaUuid, SingleAngle, Swizzle, SwizzleArgumentType,
    TimeArgumentType, UuidArgumentType,
};
pub use suggestion::{Suggestion, SuggestionError, SuggestionProvider, Suggestions, SuggestionsBuilder};
pub use tree_packet::{encode_commands_packet, usable_tree, CommandTreePacketError};

#[cfg(test)]
mod dispatcher_tests;
#[cfg(test)]
mod mc_arguments_tests;
#[cfg(test)]
mod suggestion_tests;
#[cfg(test)]
mod tree_packet_tests;
#[cfg(test)]
mod tests;
