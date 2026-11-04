//! Brigadier's command tree: nodes, builders and `addChild` merging (Brigadier 1.2.9).
//!
//! Derived from Mojang's Brigadier (MIT, see `licenses/brigadier-MIT.txt`).
//!
//! The original links node objects through shared references, including redirects back to
//! earlier nodes. Here nodes live in an arena (`CommandTree`) and refer to each other by
//! `NodeId`. As in 1.2.9, `addChild` merges onto a same-named child and does **not** re-sort
//! the children; `literals` and `arguments` keep insertion order, which decides the order in
//! which argument nodes are tried.

use super::context::{CommandContext, CommandContextBuilder, ParsedArgument, StringRange};
use crate::commands::arguments::ErasedArgumentType;
use crate::commands::exception::{built_in, CommandSyntaxException};
use crate::commands::java_text::java_to_lower_case_root;
use crate::commands::string_reader::StringReader;
use crate::commands::suggestion::{SuggestionProvider, Suggestions, SuggestionsBuilder};
use std::rc::Rc;

pub type NodeId = usize;
pub type Command<S> = Rc<dyn Fn(&CommandContext<S>) -> Result<i32, CommandSyntaxException>>;
pub type Requirement<S> = Rc<dyn Fn(&S) -> bool>;
pub type RedirectModifier<S> = Rc<dyn Fn(&CommandContext<S>) -> Result<Vec<S>, CommandSyntaxException>>;

pub enum NodeKind<S> {
    Root,
    /// `units` and `lower_case_units` are the UTF-16 forms of the literal and of
    /// `literal.toLowerCase(Locale.ROOT)`.
    Literal { literal: String, units: Vec<u16>, lower_case_units: Vec<u16> },
    Argument { name: String, argument_type: Rc<dyn ErasedArgumentType<S>> },
}

pub struct CommandNode<S> {
    kind: NodeKind<S>,
    children: Vec<(String, NodeId)>,
    literals: Vec<(String, NodeId)>,
    arguments: Vec<(String, NodeId)>,
    requirement: Requirement<S>,
    redirect: Option<NodeId>,
    modifier: Option<RedirectModifier<S>>,
    forks: bool,
    command: Option<Command<S>>,
    suggestions: Option<SuggestionProvider<S>>,
    suggestion_name: Option<String>,
}

impl<S> CommandNode<S> {
    fn new(kind: NodeKind<S>) -> Self {
        Self {
            kind,
            children: Vec::new(),
            literals: Vec::new(),
            arguments: Vec::new(),
            requirement: Rc::new(|_| true),
            redirect: None,
            modifier: None,
            forks: false,
            command: None,
            suggestions: None,
            suggestion_name: None,
        }
    }
}

/// `SuggestionProviders.ASK_SERVER`'s name: what the client is told for any provider that is
/// not registered under a name of its own.
pub const ASK_SERVER: &str = "minecraft:ask_server";

/// The arena owning every node of one dispatcher (and the temporary roots builders use).
pub struct CommandTree<S> {
    nodes: Vec<CommandNode<S>>,
}

impl<S> Default for CommandTree<S> {
    fn default() -> Self {
        Self { nodes: Vec::new() }
    }
}

impl<S: Clone> CommandTree<S> {
    pub fn new_root(&mut self) -> NodeId {
        self.push(CommandNode::new(NodeKind::Root))
    }

    fn push(&mut self, node: CommandNode<S>) -> NodeId {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// `getName()`: `""` for the root, the literal text, or the argument name.
    pub fn name(&self, id: NodeId) -> &str {
        match &self.nodes[id].kind {
            NodeKind::Root => "",
            NodeKind::Literal { literal, .. } => literal,
            NodeKind::Argument { name, .. } => name,
        }
    }

    /// `getUsageText()`: `""` for the root, the literal, or `<name>` for an argument.
    pub fn usage_text(&self, id: NodeId) -> String {
        match &self.nodes[id].kind {
            NodeKind::Root => String::new(),
            NodeKind::Literal { literal, .. } => literal.clone(),
            NodeKind::Argument { name, .. } => format!("<{name}>"),
        }
    }

    pub fn kind(&self, id: NodeId) -> &NodeKind<S> {
        &self.nodes[id].kind
    }

    pub fn command(&self, id: NodeId) -> Option<&Command<S>> {
        self.nodes[id].command.as_ref()
    }

    pub fn redirect(&self, id: NodeId) -> Option<NodeId> {
        self.nodes[id].redirect
    }

    pub fn redirect_modifier(&self, id: NodeId) -> Option<&RedirectModifier<S>> {
        self.nodes[id].modifier.as_ref()
    }

    pub fn is_fork(&self, id: NodeId) -> bool {
        self.nodes[id].forks
    }

    /// `getChildren()` in the order the original's `LinkedHashMap` holds them.
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        self.nodes[id].children.iter().map(|(_, child)| *child).collect()
    }

    /// `getChild(name)`.
    pub fn child(&self, id: NodeId, name: &str) -> Option<NodeId> {
        self.nodes[id].children.iter().find(|(key, _)| key == name).map(|(_, child)| *child)
    }

    /// `ArgumentCommandNode.getCustomSuggestions() != null`.
    pub fn has_custom_suggestions(&self, id: NodeId) -> bool {
        self.nodes[id].suggestions.is_some()
    }

    /// `SuggestionProviders.getName(provider)`: the registered name of the node's custom
    /// provider, `minecraft:ask_server` for an unregistered one, `None` without a provider.
    pub fn suggestion_provider_name(&self, id: NodeId) -> Option<&str> {
        let node = &self.nodes[id];
        node.suggestions.as_ref()?;
        Some(node.suggestion_name.as_deref().unwrap_or(ASK_SERVER))
    }

    pub fn can_use(&self, id: NodeId, source: &S) -> bool {
        (self.nodes[id].requirement)(source)
    }

    /// `addChild`: merge onto a same-named child (taking over a command) or append.
    pub fn add_child(&mut self, parent: NodeId, node: NodeId) {
        if matches!(self.nodes[node].kind, NodeKind::Root) {
            panic!("Cannot add a RootCommandNode as a child to any other CommandNode");
        }
        let name = self.name(node).to_string();
        if let Some(existing) = self.child(parent, &name) {
            if let Some(command) = self.nodes[node].command.clone() {
                self.nodes[existing].command = Some(command);
            }
            for grandchild in self.children(node) {
                self.add_child(existing, grandchild);
            }
        } else {
            self.nodes[parent].children.push((name.clone(), node));
            match self.nodes[node].kind {
                NodeKind::Literal { .. } => self.nodes[parent].literals.push((name, node)),
                NodeKind::Argument { .. } => self.nodes[parent].arguments.push((name, node)),
                NodeKind::Root => {}
            }
        }
    }

    /// `getRelevantNodes(reader)`: the literal matching the next token, else every argument.
    pub fn relevant_nodes(&self, parent: NodeId, reader: &mut StringReader) -> Vec<NodeId> {
        let node = &self.nodes[parent];
        if !node.literals.is_empty() {
            let cursor = reader.cursor();
            while reader.can_read() && reader.peek() != Some(b' ' as u16) {
                reader.skip();
            }
            let text: Vec<u16> = reader.units()[cursor..reader.cursor()].to_vec();
            reader.set_cursor(cursor);
            let matching = node.literals.iter().find(|(name, _)| name.encode_utf16().eq(text.iter().copied()));
            if let Some((_, literal)) = matching {
                return vec![*literal];
            }
        }
        node.arguments.iter().map(|(_, id)| *id).collect()
    }

    /// `CommandNode.parse(reader, contextBuilder)` for each node kind.
    pub fn parse_node(
        &self,
        id: NodeId,
        reader: &mut StringReader,
        context: &mut CommandContextBuilder<S>,
    ) -> Result<(), CommandSyntaxException> {
        match &self.nodes[id].kind {
            NodeKind::Root => Ok(()),
            NodeKind::Literal { literal, units, .. } => {
                let start = reader.cursor();
                if reader.can_read_n(units.len()) {
                    let end = start + units.len();
                    if reader.units()[start..end] == units[..] {
                        reader.set_cursor(end);
                        if !reader.can_read() || reader.peek() == Some(b' ' as u16) {
                            context.with_node(self, id, StringRange::between(start, end));
                            return Ok(());
                        }
                        reader.set_cursor(start);
                    }
                }
                Err(built_in::string_arg(
                    "argument.literal.incorrect",
                    literal.clone(),
                    reader.units(),
                    reader.cursor(),
                ))
            }
            NodeKind::Argument { name, argument_type } => {
                let start = reader.cursor();
                let result = argument_type.parse_value(reader)?;
                let parsed = ParsedArgument {
                    range: StringRange::between(start, reader.cursor()),
                    result,
                };
                let range = parsed.range;
                context.with_argument(name, parsed);
                context.with_node(self, id, range);
                Ok(())
            }
        }
    }

    /// `listSuggestions(context, builder)` for each node kind: a literal offers itself when it
    /// starts with the typed text (case-insensitively), an argument asks its custom provider
    /// or else its type, and the root offers nothing.
    pub fn list_suggestions(
        &self,
        id: NodeId,
        context: &CommandContext<S>,
        builder: &mut SuggestionsBuilder,
    ) -> Result<Suggestions, CommandSyntaxException> {
        match &self.nodes[id].kind {
            NodeKind::Root => Ok(Suggestions::empty()),
            NodeKind::Literal { literal, lower_case_units, .. } => {
                if lower_case_units.starts_with(builder.remaining_lower_case_units()) {
                    builder.suggest(literal);
                    Ok(builder.build())
                } else {
                    Ok(Suggestions::empty())
                }
            }
            NodeKind::Argument { argument_type, .. } => match &self.nodes[id].suggestions {
                Some(provider) => provider(context, builder),
                None => argument_type.list_suggestions(context, builder),
            },
        }
    }

    /// Build a detached subtree from a builder and return its node. Children go through a
    /// temporary root so duplicate names merge exactly as `ArgumentBuilder.then` does.
    pub fn build(&mut self, builder: &ArgumentBuilder<S>) -> NodeId {
        let temporary_root = self.new_root();
        for child in &builder.children {
            let built = self.build(child);
            self.add_child(temporary_root, built);
        }
        let kind = match &builder.kind {
            BuilderKind::Literal(literal) => NodeKind::Literal {
                literal: literal.clone(),
                units: literal.encode_utf16().collect(),
                lower_case_units: java_to_lower_case_root(literal).encode_utf16().collect(),
            },
            BuilderKind::Argument(name, argument_type) => NodeKind::Argument {
                name: name.clone(),
                argument_type: Rc::clone(argument_type),
            },
        };
        let mut node = CommandNode::new(kind);
        node.command = builder.command.clone();
        node.requirement = Rc::clone(&builder.requirement);
        node.redirect = builder.target;
        node.modifier = builder.modifier.clone();
        node.forks = builder.forks;
        node.suggestions = builder.suggestions.clone();
        node.suggestion_name = builder.suggestion_name.clone();
        let id = self.push(node);
        for child in self.children(temporary_root) {
            self.add_child(id, child);
        }
        id
    }
}

impl<S: Clone + 'static> CommandTree<S> {
    /// `createBuilder()`: a builder with the node's own name or type, requirement, command,
    /// suggestion provider and redirect, but none of its children. A redirect target is a node of
    /// *this* tree; the caller retargets it with [`ArgumentBuilder::with_redirect_target`] when the
    /// builder is built into another tree.
    pub fn create_builder(&self, id: NodeId) -> ArgumentBuilder<S> {
        let node = &self.nodes[id];
        let mut builder = match &node.kind {
            NodeKind::Root => panic!("a RootCommandNode has no builder"),
            NodeKind::Literal { literal, .. } => ArgumentBuilder::literal(literal),
            NodeKind::Argument { name, argument_type } => ArgumentBuilder::argument(name, Rc::clone(argument_type)),
        };
        builder.requirement = Rc::clone(&node.requirement);
        builder.command = node.command.clone();
        builder.target = node.redirect;
        builder.modifier = node.modifier.clone();
        builder.forks = node.forks;
        builder.suggestions = node.suggestions.clone();
        builder.suggestion_name = node.suggestion_name.clone();
        builder
    }
}

enum BuilderKind<S> {
    Literal(String),
    Argument(String, Rc<dyn ErasedArgumentType<S>>),
}

/// `LiteralArgumentBuilder` / `RequiredArgumentBuilder` in one type.
pub struct ArgumentBuilder<S> {
    kind: BuilderKind<S>,
    children: Vec<ArgumentBuilder<S>>,
    command: Option<Command<S>>,
    requirement: Requirement<S>,
    target: Option<NodeId>,
    modifier: Option<RedirectModifier<S>>,
    forks: bool,
    suggestions: Option<SuggestionProvider<S>>,
    suggestion_name: Option<String>,
}

impl<S: Clone + 'static> ArgumentBuilder<S> {
    fn with_kind(kind: BuilderKind<S>) -> Self {
        Self {
            kind,
            children: Vec::new(),
            command: None,
            requirement: Rc::new(|_| true),
            target: None,
            modifier: None,
            forks: false,
            suggestions: None,
            suggestion_name: None,
        }
    }

    /// `LiteralArgumentBuilder.literal(name)`.
    pub fn literal(name: &str) -> Self {
        Self::with_kind(BuilderKind::Literal(name.to_string()))
    }

    /// `RequiredArgumentBuilder.argument(name, type)`.
    pub fn argument(name: &str, argument_type: Rc<dyn ErasedArgumentType<S>>) -> Self {
        Self::with_kind(BuilderKind::Argument(name.to_string(), argument_type))
    }

    /// `then(argument)`; the original throws `IllegalStateException` on a redirected node.
    pub fn then(mut self, child: ArgumentBuilder<S>) -> Self {
        if self.target.is_some() {
            panic!("Cannot add children to a redirected node");
        }
        self.children.push(child);
        self
    }

    pub fn executes(mut self, command: Command<S>) -> Self {
        self.command = Some(command);
        self
    }

    pub fn requires(mut self, requirement: Requirement<S>) -> Self {
        self.requirement = requirement;
        self
    }

    /// `RequiredArgumentBuilder.suggests(provider)`; literal nodes have no such method.
    pub fn suggests(mut self, provider: SuggestionProvider<S>) -> Self {
        if matches!(self.kind, BuilderKind::Literal(_)) {
            panic!("Only argument nodes take a suggestion provider");
        }
        self.suggestions = Some(provider);
        self.suggestion_name = None;
        self
    }

    /// `suggests(provider)` with a provider registered under `name` (`SuggestionProviders.register`);
    /// the command-tree packet tells the client the name instead of `minecraft:ask_server`.
    pub fn suggests_named(mut self, name: &str, provider: SuggestionProvider<S>) -> Self {
        self = self.suggests(provider);
        self.suggestion_name = Some(name.to_string());
        self
    }

    /// `redirect(target)`.
    pub fn redirect(self, target: NodeId) -> Self {
        self.forward(target, None, false)
    }

    /// `redirect(target, SingleRedirectModifier)`: one new source per context.
    pub fn redirect_single(self, target: NodeId, modifier: Rc<dyn Fn(&CommandContext<S>) -> Result<S, CommandSyntaxException>>) -> Self {
        let modifier: RedirectModifier<S> = Rc::new(move |context| modifier(context).map(|source| vec![source]));
        self.forward(target, Some(modifier), false)
    }

    /// `fork(target, modifier)`.
    pub fn fork(self, target: NodeId, modifier: RedirectModifier<S>) -> Self {
        self.forward(target, Some(modifier), true)
    }

    /// `redirect(target)` where `target` may be missing: the original's `redirect(null)` clears the
    /// redirect, the modifier and the fork flag.
    pub fn with_redirect_target(mut self, target: Option<NodeId>) -> Self {
        self.target = target;
        self.modifier = None;
        self.forks = false;
        self
    }

    /// Replace the requirement.
    pub fn with_requirement(mut self, requirement: Requirement<S>) -> Self {
        self.requirement = requirement;
        self
    }

    /// `SuggestionProviders.safelySwap(provider)`: a provider registered under a name is kept,
    /// any other becomes `provider` (the ask-the-server provider). Nothing happens without a provider.
    pub fn with_swapped_suggestions(mut self, unregistered_replacement: SuggestionProvider<S>) -> Self {
        if self.suggestions.is_some() && self.suggestion_name.is_none() {
            self.suggestions = Some(unregistered_replacement);
        }
        self
    }

    /// Whether the builder carries a command (`getCommand() != null`).
    pub fn has_command(&self) -> bool {
        self.command.is_some()
    }

    /// `forward(target, modifier, fork)`; throws if the node already has children.
    pub fn forward(mut self, target: NodeId, modifier: Option<RedirectModifier<S>>, fork: bool) -> Self {
        if !self.children.is_empty() {
            panic!("Cannot forward a node with children");
        }
        self.target = Some(target);
        self.modifier = modifier;
        self.forks = fork;
        self
    }
}
