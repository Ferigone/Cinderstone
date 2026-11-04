//! `CommandContextBuilder` / `CommandContext`, `StringRange`, `ParsedArgument` and
//! `ParsedCommandNode` (Brigadier 1.2.9).
//!
//! Derived from Mojang's Brigadier (MIT, see `licenses/brigadier-MIT.txt`). As in the
//! original, `CommandContextBuilder::copy` does not carry the redirect modifier: it is set
//! again by every `with_node`.

use super::tree::{Command, CommandTree, NodeId, RedirectModifier};
use crate::commands::arguments::ArgumentValue;
use crate::commands::suggestion::SuggestionError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StringRange {
    start: usize,
    end: usize,
}

impl StringRange {
    pub fn at(pos: usize) -> Self {
        Self { start: pos, end: pos }
    }

    pub fn between(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn encompassing(a: Self, b: Self) -> Self {
        Self {
            start: a.start.min(b.start),
            end: a.end.max(b.end),
        }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedArgument {
    pub range: StringRange,
    pub result: ArgumentValue,
}

#[derive(Clone, Debug)]
pub struct ParsedCommandNode {
    pub node: NodeId,
    pub name: String,
    pub range: StringRange,
}

/// `LinkedHashMap.put`: replacing a key keeps its original position.
fn put(map: &mut Vec<(String, ParsedArgument)>, name: String, argument: ParsedArgument) {
    match map.iter_mut().find(|(key, _)| *key == name) {
        Some(entry) => entry.1 = argument,
        None => map.push((name, argument)),
    }
}

/// `SuggestionContext`: the node whose children are completed and where the replaced text
/// starts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SuggestionContext {
    pub parent: NodeId,
    pub start_pos: usize,
}

pub struct CommandContextBuilder<S> {
    arguments: Vec<(String, ParsedArgument)>,
    root: NodeId,
    nodes: Vec<ParsedCommandNode>,
    source: S,
    command: Option<Command<S>>,
    child: Option<Box<CommandContextBuilder<S>>>,
    range: StringRange,
    modifier: Option<RedirectModifier<S>>,
    forks: bool,
}

impl<S: Clone> Clone for CommandContextBuilder<S> {
    fn clone(&self) -> Self {
        Self {
            arguments: self.arguments.clone(),
            root: self.root,
            nodes: self.nodes.clone(),
            source: self.source.clone(),
            command: self.command.clone(),
            child: self.child.clone(),
            range: self.range,
            modifier: self.modifier.clone(),
            forks: self.forks,
        }
    }
}

impl<S: Clone> CommandContextBuilder<S> {
    pub fn new(source: S, root: NodeId, start: usize) -> Self {
        Self {
            arguments: Vec::new(),
            root,
            nodes: Vec::new(),
            source,
            command: None,
            child: None,
            range: StringRange::at(start),
            modifier: None,
            forks: false,
        }
    }

    pub fn source(&self) -> &S {
        &self.source
    }

    pub fn range(&self) -> StringRange {
        self.range
    }

    pub fn with_argument(&mut self, name: &str, argument: ParsedArgument) -> &mut Self {
        put(&mut self.arguments, name.to_string(), argument);
        self
    }

    pub fn with_command(&mut self, command: Option<Command<S>>) -> &mut Self {
        self.command = command;
        self
    }

    pub fn with_node(&mut self, tree: &CommandTree<S>, node: NodeId, range: StringRange) -> &mut Self {
        self.nodes.push(ParsedCommandNode {
            node,
            name: tree.name(node).to_string(),
            range,
        });
        self.range = StringRange::encompassing(self.range, range);
        self.modifier = tree.redirect_modifier(node).cloned();
        self.forks = tree.is_fork(node);
        self
    }

    /// `copy()`: the redirect modifier is intentionally not copied (see module docs).
    pub fn copy(&self) -> Self {
        let mut copy = Self::new(self.source.clone(), self.root, self.range.start());
        copy.command = self.command.clone();
        for (name, argument) in &self.arguments {
            put(&mut copy.arguments, name.clone(), argument.clone());
        }
        copy.nodes.extend(self.nodes.iter().cloned());
        copy.child = self.child.clone();
        copy.range = self.range;
        copy.forks = self.forks;
        copy
    }

    pub fn with_child(&mut self, child: Self) -> &mut Self {
        self.child = Some(Box::new(child));
        self
    }

    /// `findSuggestionContext(cursor)`: which node's children are offered at `cursor`.
    pub fn find_suggestion_context(&self, cursor: usize) -> Result<SuggestionContext, SuggestionError> {
        if self.range.start() > cursor {
            return Err(SuggestionError::NoNodeBeforeCursor);
        }
        if self.range.end() < cursor {
            if let Some(child) = &self.child {
                return child.find_suggestion_context(cursor);
            }
            return Ok(match self.nodes.last() {
                Some(last) => SuggestionContext {
                    parent: last.node,
                    start_pos: last.range.end() + 1,
                },
                None => SuggestionContext {
                    parent: self.root,
                    start_pos: self.range.start(),
                },
            });
        }
        let mut previous = self.root;
        for node in &self.nodes {
            if node.range.start() <= cursor && cursor <= node.range.end() {
                return Ok(SuggestionContext {
                    parent: previous,
                    start_pos: node.range.start(),
                });
            }
            previous = node.node;
        }
        Ok(SuggestionContext {
            parent: previous,
            start_pos: self.range.start(),
        })
    }

    pub fn build(&self, input: &str) -> CommandContext<S> {
        CommandContext {
            source: self.source.clone(),
            input: input.to_string(),
            arguments: self.arguments.clone(),
            command: self.command.clone(),
            root: self.root,
            nodes: self.nodes.clone(),
            range: self.range,
            child: self.child.as_ref().map(|child| Box::new(child.build(input))),
            modifier: self.modifier.clone(),
            forks: self.forks,
        }
    }
}

pub struct CommandContext<S> {
    source: S,
    input: String,
    arguments: Vec<(String, ParsedArgument)>,
    command: Option<Command<S>>,
    root: NodeId,
    nodes: Vec<ParsedCommandNode>,
    range: StringRange,
    child: Option<Box<CommandContext<S>>>,
    modifier: Option<RedirectModifier<S>>,
    forks: bool,
}

impl<S: Clone> Clone for CommandContext<S> {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            input: self.input.clone(),
            arguments: self.arguments.clone(),
            command: self.command.clone(),
            root: self.root,
            nodes: self.nodes.clone(),
            range: self.range,
            child: self.child.clone(),
            modifier: self.modifier.clone(),
            forks: self.forks,
        }
    }
}

impl<S: Clone> CommandContext<S> {
    /// `copyFor(source)`. The original returns `this` for an identical source object; the
    /// result is observably the same, so a copy is always made here.
    pub fn copy_for(&self, source: S) -> Self {
        let mut copy = self.clone();
        copy.source = source;
        copy
    }

    pub fn source(&self) -> &S {
        &self.source
    }

    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn root_node(&self) -> NodeId {
        self.root
    }

    /// `getArgument(name, ..)`: the parsed value, or `None` where Brigadier throws
    /// `IllegalArgumentException("No such argument ...")`.
    pub fn argument(&self, name: &str) -> Option<&ArgumentValue> {
        self.arguments.iter().find(|(key, _)| key == name).map(|(_, parsed)| &parsed.result)
    }

    pub fn parsed_argument(&self, name: &str) -> Option<&ParsedArgument> {
        self.arguments.iter().find(|(key, _)| key == name).map(|(_, parsed)| parsed)
    }

    pub fn command(&self) -> Option<&Command<S>> {
        self.command.as_ref()
    }

    pub fn nodes(&self) -> &[ParsedCommandNode] {
        &self.nodes
    }

    pub fn has_nodes(&self) -> bool {
        !self.nodes.is_empty()
    }

    pub fn range(&self) -> StringRange {
        self.range
    }

    pub fn child(&self) -> Option<&CommandContext<S>> {
        self.child.as_deref()
    }

    pub fn redirect_modifier(&self) -> Option<&RedirectModifier<S>> {
        self.modifier.as_ref()
    }

    pub fn is_forked(&self) -> bool {
        self.forks
    }
}
