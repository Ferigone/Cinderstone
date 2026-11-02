//! `CommandDispatcher` core (Brigadier 1.2.9): registration, `parse` and `execute`.
//!
//! Stage 2 of WP-E3 (issue #24). Usage strings, completion suggestions, custom
//! `LiteralMessage` exception types, Minecraft argument types and the command catalog are
//! later stages. Derived from Mojang's Brigadier (MIT, see `licenses/brigadier-MIT.txt`).

pub mod chain;
pub mod context;
pub mod tree;

pub use chain::ContextChain;
pub use context::{CommandContext, CommandContextBuilder, ParsedArgument, ParsedCommandNode, StringRange};
pub use tree::{ArgumentBuilder, Command, CommandNode, CommandTree, NodeId, NodeKind, RedirectModifier, Requirement};

use super::exception::{built_in, CommandSyntaxException};
use super::string_reader::StringReader;
use std::cmp::Ordering;
use std::rc::Rc;

/// `ResultConsumer`: told about every executed (or failed) command context.
pub type ResultConsumer<S> = Rc<dyn Fn(&CommandContext<S>, bool, i32)>;

const ARGUMENT_SEPARATOR: u16 = b' ' as u16;

/// `ParseResults`: the builder so far, the reader position and the per-node failures
/// (in the original's `LinkedHashMap` insertion order).
pub struct ParseResults<S> {
    pub context: CommandContextBuilder<S>,
    pub reader: StringReader,
    pub exceptions: Vec<(NodeId, CommandSyntaxException)>,
}

pub struct CommandDispatcher<S> {
    tree: CommandTree<S>,
    root: NodeId,
    consumer: Option<ResultConsumer<S>>,
}

impl<S: Clone + 'static> Default for CommandDispatcher<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S: Clone + 'static> CommandDispatcher<S> {
    pub fn new() -> Self {
        let mut tree = CommandTree::default();
        let root = tree.new_root();
        Self {
            tree,
            root,
            consumer: None,
        }
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn tree(&self) -> &CommandTree<S> {
        &self.tree
    }

    pub fn set_consumer(&mut self, consumer: ResultConsumer<S>) {
        self.consumer = Some(consumer);
    }

    /// `register(literal)`: build the literal and merge it under the root.
    pub fn register(&mut self, command: ArgumentBuilder<S>) -> NodeId {
        let built = self.tree.build(&command);
        self.tree.add_child(self.root, built);
        built
    }

    pub fn parse(&self, command: &str, source: &S) -> ParseResults<S> {
        self.parse_reader(StringReader::new(command), source)
    }

    pub fn parse_reader(&self, command: StringReader, source: &S) -> ParseResults<S> {
        let context = CommandContextBuilder::new(source.clone(), self.root, command.cursor());
        self.parse_nodes(self.root, &command, &context)
    }

    fn parse_nodes(
        &self,
        node: NodeId,
        original_reader: &StringReader,
        context_so_far: &CommandContextBuilder<S>,
    ) -> ParseResults<S> {
        let source = context_so_far.source();
        let mut errors: Vec<(NodeId, CommandSyntaxException)> = Vec::new();
        let mut potentials: Vec<ParseResults<S>> = Vec::new();
        let cursor = original_reader.cursor();

        let mut probe = original_reader.clone();
        for child in self.tree.relevant_nodes(node, &mut probe) {
            if !self.tree.can_use(child, source) {
                continue;
            }
            let mut context = context_so_far.copy();
            let mut reader = original_reader.clone();
            let outcome = self.tree.parse_node(child, &mut reader, &mut context).and_then(|()| {
                if reader.can_read() && reader.peek() != Some(ARGUMENT_SEPARATOR) {
                    return Err(built_in::simple(
                        "command.expected.separator",
                        reader.units(),
                        reader.cursor(),
                    ));
                }
                Ok(())
            });
            if let Err(exception) = outcome {
                errors.push((child, exception));
                reader.set_cursor(cursor);
                continue;
            }

            context.with_command(self.tree.command(child).cloned());
            let redirect = self.tree.redirect(child);
            if reader.can_read_n(if redirect.is_none() { 2 } else { 1 }) {
                reader.skip();
                if let Some(target) = redirect {
                    let child_context = CommandContextBuilder::new(source.clone(), target, reader.cursor());
                    let parse = self.parse_nodes(target, &reader, &child_context);
                    context.with_child(parse.context);
                    return ParseResults {
                        context,
                        reader: parse.reader,
                        exceptions: parse.exceptions,
                    };
                }
                potentials.push(self.parse_nodes(child, &reader, &context));
            } else {
                potentials.push(ParseResults {
                    context,
                    reader,
                    exceptions: Vec::new(),
                });
            }
        }

        if !potentials.is_empty() {
            if potentials.len() > 1 {
                potentials.sort_by(compare_potentials);
            }
            return potentials.remove(0);
        }

        ParseResults {
            context: context_so_far.clone(),
            reader: original_reader.clone(),
            exceptions: errors,
        }
    }

    pub fn execute_str(&self, input: &str, source: &S) -> Result<i32, CommandSyntaxException> {
        let parse = self.parse(input, source);
        self.execute(&parse)
    }

    pub fn execute(&self, parse: &ParseResults<S>) -> Result<i32, CommandSyntaxException> {
        if parse.reader.can_read() {
            if parse.exceptions.len() == 1 {
                return Err(parse.exceptions[0].1.clone());
            }
            let key = if parse.context.range().is_empty() {
                "command.unknown.command"
            } else {
                "command.unknown.argument"
            };
            return Err(built_in::simple(key, parse.reader.units(), parse.reader.cursor()));
        }

        let command = parse.reader.string();
        let original = parse.context.build(&command);
        let Some(chain) = ContextChain::try_flatten(&original) else {
            self.complete(&original, false, 0);
            return Err(built_in::simple(
                "command.unknown.command",
                parse.reader.units(),
                parse.reader.cursor(),
            ));
        };
        chain.execute_all(original.source().clone(), self.consumer.as_ref())
    }

    fn complete(&self, context: &CommandContext<S>, success: bool, result: i32) {
        if let Some(consumer) = &self.consumer {
            consumer(context, success, result);
        }
    }
}

/// The stable comparator `parseNodes` sorts competing parses with: complete reads first,
/// then parses without errors.
fn compare_potentials<S>(a: &ParseResults<S>, b: &ParseResults<S>) -> Ordering {
    let (a_read, b_read) = (a.reader.can_read(), b.reader.can_read());
    if !a_read && b_read {
        return Ordering::Less;
    }
    if a_read && !b_read {
        return Ordering::Greater;
    }
    let (a_err, b_err) = (!a.exceptions.is_empty(), !b.exceptions.is_empty());
    if !a_err && b_err {
        return Ordering::Less;
    }
    if a_err && !b_err {
        return Ordering::Greater;
    }
    Ordering::Equal
}
