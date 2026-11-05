//! `OperationArgument`: the scoreboard operators `=`, `+=`, `-=`, `*=`, `/=`, `%=`, `<`, `>` and `><`.
//!
//! Ported from the unchanged class of the pinned 1.21 server, read with direct `javap` inspection of
//! the jar (remapped with the official mappings, kept outside git) and compared with the executed
//! original in `fixtures/commands-mc-arguments-vectors.json`; its `ArgumentTypeInfos` registry id
//! (no template) is compared with the original's own `ClientboundCommandsPacket` in
//! `fixtures/commands-brigadier-tree-packet-vectors.json`.
//!
//! What the original does and this port reproduces:
//!
//! * an empty remainder fails with `arguments.operation.invalid` *with* the reader's context; any other
//!   text is read up to the next space (not up to whitespace) and looked up exactly, and an unknown
//!   operator fails with the same message *without* context (so no position, and the reader stays
//!   after the text it consumed);
//! * the parsed value is a function of the two scores of a `ScoreAccess` pair. [`ScoreOperation::apply`]
//!   returns the new pair of plain integers so this stage needs no scoreboard binding: `=` takes the
//!   second, `+=`, `-=` and `*=` wrap like Java `int`, `/=` is `Math.floorDiv` and `%=` is
//!   `Math.floorMod` (both fail with `arguments.operation.div0`, without context, when the divisor is
//!   zero), `<` and `>` are `Math.min` / `Math.max`, and `><` swaps the two scores;
//! * the suggestions are the nine operators, in that order, that start with the lower-cased text typed
//!   so far (`SharedSuggestionProvider.suggest`, whose `matchesSubStr` also accepts a match after a
//!   `.` or `_`, which no operator contains).

use super::arguments::{ArgumentNetworkInfo, ArgumentType};
use super::dispatcher::CommandContext;
use super::exception::{built_in, CommandMessage, CommandSyntaxException};
use super::string_reader::StringReader;
use super::suggestion::{Suggestions, SuggestionsBuilder};

/// `ArgumentTypeInfos` registry id (1.21 registration order).
pub mod registry_ids {
    pub const OPERATION: i32 = 25;
}

/// `OperationArgument.Operation`, one value per operator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreOperation {
    Assign,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Min,
    Max,
    Swap,
}

/// The operators in the order the original suggests them.
const OPERATORS: [(&str, ScoreOperation); 9] = [
    ("=", ScoreOperation::Assign),
    ("+=", ScoreOperation::Add),
    ("-=", ScoreOperation::Subtract),
    ("*=", ScoreOperation::Multiply),
    ("/=", ScoreOperation::Divide),
    ("%=", ScoreOperation::Modulo),
    ("<", ScoreOperation::Min),
    (">", ScoreOperation::Max),
    ("><", ScoreOperation::Swap),
];

impl ScoreOperation {
    /// The operator text, as `getOperation(String)` accepts it.
    pub fn symbol(self) -> &'static str {
        OPERATORS.iter().find(|(_, operation)| *operation == self).map_or("", |(symbol, _)| symbol)
    }

    /// `getOperation(String)`: an exact match of one of the nine operators.
    pub fn from_symbol(text: &str) -> Option<Self> {
        OPERATORS.iter().find(|(symbol, _)| *symbol == text).map(|(_, operation)| *operation)
    }

    /// `Operation.apply(target, source)` on the scores of the two `ScoreAccess`es: returns the new
    /// `(target, source)` pair. Only `><` changes the source.
    pub fn apply(self, target: i32, source: i32) -> Result<(i32, i32), CommandSyntaxException> {
        Ok(match self {
            Self::Assign => (source, source),
            Self::Add => (target.wrapping_add(source), source),
            Self::Subtract => (target.wrapping_sub(source), source),
            Self::Multiply => (target.wrapping_mul(source), source),
            Self::Divide => (floor_div(target, source)?, source),
            Self::Modulo => (floor_mod(target, source)?, source),
            Self::Min => (target.min(source), source),
            Self::Max => (target.max(source), source),
            Self::Swap => (source, target),
        })
    }
}

fn divide_by_zero() -> CommandSyntaxException {
    CommandSyntaxException::without_context(CommandMessage::new("arguments.operation.div0", Vec::new()))
}

/// `Math.floorDiv(int, int)`: `Integer.MIN_VALUE / -1` wraps instead of failing.
fn floor_div(x: i32, y: i32) -> Result<i32, CommandSyntaxException> {
    if y == 0 {
        return Err(divide_by_zero());
    }
    let quotient = x.wrapping_div(y);
    Ok(if (x ^ y) < 0 && quotient.wrapping_mul(y) != x { quotient - 1 } else { quotient })
}

/// `Math.floorMod(int, int)` (`Mth.positiveModulo`).
fn floor_mod(x: i32, y: i32) -> Result<i32, CommandSyntaxException> {
    if y == 0 {
        return Err(divide_by_zero());
    }
    let remainder = x.wrapping_rem(y);
    Ok(if (remainder ^ y) < 0 && remainder != 0 { remainder + y } else { remainder })
}

/// `OperationArgument.operation()`.
#[derive(Clone, Copy, Debug, Default)]
pub struct OperationArgumentType;

impl ArgumentType for OperationArgumentType {
    type Value = ScoreOperation;

    fn parse(&self, reader: &mut StringReader) -> Result<ScoreOperation, CommandSyntaxException> {
        if !reader.can_read() {
            return Err(built_in::simple("arguments.operation.invalid", reader.units(), reader.cursor()));
        }
        let start = reader.cursor();
        while reader.can_read() && reader.peek() != Some(b' ' as u16) {
            reader.skip();
        }
        let text = String::from_utf16_lossy(&reader.units()[start..reader.cursor()]);
        ScoreOperation::from_symbol(&text)
            .ok_or_else(|| CommandSyntaxException::without_context(CommandMessage::new("arguments.operation.invalid", Vec::new())))
    }

    fn network_info(&self) -> Option<ArgumentNetworkInfo> {
        Some(ArgumentNetworkInfo {
            registry_id: registry_ids::OPERATION,
            template: Vec::new(),
        })
    }

    /// `SharedSuggestionProvider.suggest(operators, builder)`.
    fn list_suggestions<S>(
        &self,
        _context: &CommandContext<S>,
        builder: &mut SuggestionsBuilder,
    ) -> Result<Suggestions, CommandSyntaxException> {
        for (symbol, _) in OPERATORS {
            if symbol.encode_utf16().collect::<Vec<_>>().starts_with(builder.remaining_lower_case_units()) {
                builder.suggest(symbol);
            }
        }
        Ok(builder.build())
    }
}
