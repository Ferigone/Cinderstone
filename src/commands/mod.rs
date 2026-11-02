//! Command system, stages 1-4: the reader, syntax exceptions, primitive argument types and
//! the `CommandDispatcher` (tree, parse, execute, usage strings, completion suggestions) of
//! Mojang's Brigadier 1.2.9 with the dedicated server's built-in exception messages.
//!
//! This is `commands_data.1` partial work (WP-E3, issue #24). Nothing here is wired to
//! Main: Minecraft argument types, selectors and the command catalog are later stages, and
//! the existing bounded administration commands keep their own parser.
//!
//! Brigadier is MIT-licensed (`licenses/brigadier-MIT.txt`). See
//! `docs/reference/commands-brigadier.md` for the source identity and evidence.

pub mod arguments;
pub mod dispatcher;
pub mod exception;
pub mod java_text;
pub mod string_reader;
pub mod suggestion;

pub use arguments::{
    ArgumentType, ArgumentValue, ErasedArgumentType, BoolArgumentType, DoubleArgumentType, FloatArgumentType, IntegerArgumentType,
    LongArgumentType, StringArgumentType, StringType,
};
pub use dispatcher::{ArgumentBuilder, CommandContext, CommandDispatcher, ParseResults};
pub use exception::{CommandArg, CommandMessage, CommandSyntaxException};
pub use string_reader::StringReader;
pub use suggestion::{Suggestion, SuggestionError, SuggestionProvider, Suggestions, SuggestionsBuilder};

#[cfg(test)]
mod dispatcher_tests;
#[cfg(test)]
mod suggestion_tests;
#[cfg(test)]
mod tests;
