//! `CommandDispatcher` core (Brigadier 1.2.9): registration, `parse`, `execute`, usage
//! strings and completion suggestions.
//!
//! Stages 2-4 of WP-E3 (issue #24). Custom `LiteralMessage` exception types, Minecraft
//! argument types and the command catalog are later stages. Derived from Mojang's Brigadier
//! (MIT, see `licenses/brigadier-MIT.txt`).

pub mod chain;
pub mod context;
pub mod tree;

pub use chain::ContextChain;
pub use context::{
    CommandContext, CommandContextBuilder, ParsedArgument, ParsedCommandNode, StringRange, SuggestionContext,
};
pub use tree::{
    ArgumentBuilder, Command, CommandNode, CommandTree, NodeId, NodeKind, RedirectModifier, Requirement, ASK_SERVER,
};

use super::exception::{built_in, CommandSyntaxException};
use super::java_text::java_to_lower_case_root;
use super::string_reader::StringReader;
use super::suggestion::{SuggestionError, Suggestions, SuggestionsBuilder};
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

    fn redirect_usage(&self, redirect: NodeId) -> String {
        if redirect == self.root {
            "...".to_string()
        } else {
            format!("-> {}", self.tree.usage_text(redirect))
        }
    }

    /// `getAllUsage(node, source, restricted)`: one line per executable path below `node`.
    pub fn all_usage(&self, node: NodeId, source: &S, restricted: bool) -> Vec<String> {
        let mut result = Vec::new();
        self.collect_all_usage(node, source, &mut result, "", restricted);
        result
    }

    fn collect_all_usage(&self, node: NodeId, source: &S, result: &mut Vec<String>, prefix: &str, restricted: bool) {
        if restricted && !self.tree.can_use(node, source) {
            return;
        }
        if self.tree.command(node).is_some() {
            result.push(prefix.to_string());
        }
        if let Some(redirect) = self.tree.redirect(node) {
            let redirect = self.redirect_usage(redirect);
            result.push(if prefix.is_empty() {
                format!("{} {redirect}", self.tree.usage_text(node))
            } else {
                format!("{prefix} {redirect}")
            });
        } else {
            for child in self.tree.children(node) {
                let usage = self.tree.usage_text(child);
                let next = if prefix.is_empty() { usage } else { format!("{prefix} {usage}") };
                self.collect_all_usage(child, source, result, &next, restricted);
            }
        }
    }

    /// `getSmartUsage(node, source)`: a compact usage per usable child of `node`.
    pub fn smart_usage(&self, node: NodeId, source: &S) -> Vec<(NodeId, String)> {
        let optional = self.tree.command(node).is_some();
        let mut result = Vec::new();
        for child in self.tree.children(node) {
            if let Some(usage) = self.smart_usage_of(child, source, optional, false) {
                result.push((child, usage));
            }
        }
        result
    }

    fn smart_usage_of(&self, node: NodeId, source: &S, optional: bool, deep: bool) -> Option<String> {
        if !self.tree.can_use(node, source) {
            return None;
        }
        let text = self.tree.usage_text(node);
        let this = if optional { format!("[{text}]") } else { text };
        let child_optional = self.tree.command(node).is_some();
        let (open, close) = if child_optional { ("[", "]") } else { ("(", ")") };
        if !deep {
            if let Some(redirect) = self.tree.redirect(node) {
                return Some(format!("{this} {}", self.redirect_usage(redirect)));
            }
            let children: Vec<NodeId> = self
                .tree
                .children(node)
                .into_iter()
                .filter(|child| self.tree.can_use(*child, source))
                .collect();
            if children.len() == 1 {
                if let Some(usage) = self.smart_usage_of(children[0], source, child_optional, child_optional) {
                    return Some(format!("{this} {usage}"));
                }
            } else if children.len() > 1 {
                let mut child_usage: Vec<String> = Vec::new();
                for child in &children {
                    if let Some(usage) = self.smart_usage_of(*child, source, child_optional, true) {
                        if !child_usage.contains(&usage) {
                            child_usage.push(usage);
                        }
                    }
                }
                if child_usage.len() == 1 {
                    let usage = &child_usage[0];
                    let usage = if child_optional { format!("[{usage}]") } else { usage.clone() };
                    return Some(format!("{this} {usage}"));
                } else if child_usage.len() > 1 {
                    let names: Vec<String> = children.iter().map(|child| self.tree.usage_text(*child)).collect();
                    return Some(format!("{this} {open}{}{close}", names.join("|")));
                }
            }
        }
        Some(this)
    }

    /// `getCompletionSuggestions(parse)`: complete at the end of the input.
    pub fn completion_suggestions_at_end(&self, parse: &ParseResults<S>) -> Result<Suggestions, SuggestionError> {
        self.completion_suggestions(parse, parse.reader.total_length())
    }

    /// `getCompletionSuggestions(parse, cursor)`: the children of the node before `cursor`
    /// each list their suggestions for the text up to `cursor`, and the lists are merged.
    /// A child whose provider fails with a `CommandSyntaxException` contributes nothing.
    pub fn completion_suggestions(&self, parse: &ParseResults<S>, cursor: usize) -> Result<Suggestions, SuggestionError> {
        let found = parse.context.find_suggestion_context(cursor)?;
        let start = found.start_pos.min(cursor);
        let full_input = parse.reader.string();
        let units = parse.reader.units();
        if cursor > units.len() {
            return Err(SuggestionError::StringIndexOutOfBounds);
        }
        let truncated = String::from_utf16_lossy(&units[..cursor]);
        let truncated_lower_case = java_to_lower_case_root(&truncated);
        let context = parse.context.build(&truncated);
        let mut collected = Vec::new();
        for child in self.tree.children(found.parent) {
            let mut builder = SuggestionsBuilder::with_lower_case(&truncated, &truncated_lower_case, start)?;
            collected.push(
                self.tree
                    .list_suggestions(child, &context, &mut builder)
                    .unwrap_or_else(|_| Suggestions::empty()),
            );
        }
        Suggestions::merge(&full_input, &collected)
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
