//! `ClientboundCommandsPacket`: the command tree as the client receives it (1.21 body, without
//! the packet id).
//!
//! Derived from the behaviour of the dedicated server's `ClientboundCommandsPacket`, read with
//! direct `javap` inspection of the pinned jar (remapped with the official mappings, kept outside
//! git) and compared with the executed original in
//! `fixtures/commands-brigadier-tree-packet-vectors.json`.
//!
//! What the original does and this encoder reproduces:
//!
//! * nodes are numbered by a **breadth-first walk** from the root that also queues redirect
//!   targets, and the numbering map is keyed by `CommandNode.equals`, a *structural* equality:
//!   kind, literal or argument name, the argument type's `equals`, the `Command` instance and the
//!   children map (not the requirement, the redirect or the suggestion provider). Equal nodes
//!   therefore share one index, and the first one reached supplies the entry;
//! * an entry is a flags byte (`1` literal, `2` argument, `4` executable, `8` redirect, `16`
//!   custom suggestions), the children as a VarInt array, the redirect index when flagged, then
//!   the name (a literal) or the name, the `ArgumentTypeInfos` registry id, the serialized template
//!   and, when flagged, the suggestion provider's name (an argument);
//! * the packet is the VarInt entry count, the entries, and the VarInt index of the root.
//!
//! [`usable_tree`] is the step before the packet: the server does not send its registered tree but
//! `Commands.fillUsableCommands`' copy of the nodes the player may use.
//!
//! Argument types without a [`network_info`](super::arguments::ArgumentType::network_info) cannot
//! be encoded (the original throws for an unregistered type); Minecraft's own argument types are a
//! later stage. Permission filtering belongs to the caller: the original builds the tree sent to a
//! player by copying only the nodes the player may use.

use super::dispatcher::{Command, CommandTree, NodeId, NodeKind};
use super::suggestion::{SuggestionProvider, Suggestions};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

const FLAG_LITERAL: u8 = 1;
const FLAG_ARGUMENT: u8 = 2;
const FLAG_EXECUTABLE: u8 = 4;
const FLAG_REDIRECT: u8 = 8;
const FLAG_CUSTOM_SUGGESTIONS: u8 = 16;

/// `FriendlyByteBuf.writeUtf` accepts at most 32767 UTF-16 units.
const MAX_STRING_UNITS: usize = 32767;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommandTreePacketError {
    /// The argument node's type has no network form (`ArgumentTypeInfos.unpack` would throw).
    UnsupportedArgumentType { node: String },
    /// A name longer than `FriendlyByteBuf.writeUtf` allows.
    StringTooLong { units: usize },
    /// More nodes than a VarInt count can hold.
    TooManyNodes,
}

/// The identity of an argument type under `Object.equals`.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum TypeIdentity {
    /// Types whose `equals` compares their fields.
    Value(String),
    /// Types that keep `Object.equals`: only the same instance is equal.
    Instance(usize),
}

/// What `CommandNode.equals` compares, with the children already reduced to equivalence classes.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ClassKey {
    kind: u8,
    name: String,
    argument_type: Option<TypeIdentity>,
    command: usize,
    children: Vec<(String, usize)>,
}

pub(super) struct Classifier<'a, S> {
    tree: &'a CommandTree<S>,
    interned: HashMap<ClassKey, usize>,
    class_of: HashMap<NodeId, usize>,
}

impl<'a, S: Clone> Classifier<'a, S> {
    pub(super) fn new(tree: &'a CommandTree<S>) -> Self {
        Self {
            tree,
            interned: HashMap::new(),
            class_of: HashMap::new(),
        }
    }

    /// The equivalence class of `node` under `CommandNode.equals`.
    pub(super) fn class(&mut self, node: NodeId) -> usize {
        if let Some(class) = self.class_of.get(&node) {
            return *class;
        }
        let mut children: Vec<(String, usize)> = self
            .tree
            .children(node)
            .into_iter()
            .map(|child| (self.tree.name(child).to_string(), self.class(child)))
            .collect();
        // Map.equals ignores the order of the children.
        children.sort();
        let (kind, argument_type) = match self.tree.kind(node) {
            NodeKind::Root => (0, None),
            NodeKind::Literal { .. } => (1, None),
            NodeKind::Argument { argument_type, .. } => (
                2,
                Some(match argument_type.equality_key() {
                    Some(key) => TypeIdentity::Value(key),
                    None => TypeIdentity::Instance(Rc::as_ptr(argument_type) as *const () as usize),
                }),
            ),
        };
        let key = ClassKey {
            kind,
            name: self.tree.name(node).to_string(),
            argument_type,
            command: self
                .tree
                .command(node)
                .map_or(0, |command| Rc::as_ptr(command) as *const () as usize),
            children,
        };
        let next = self.interned.len();
        let class = *self.interned.entry(key).or_insert(next);
        self.class_of.insert(node, class);
        class
    }
}

fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        if value & !0x7F == 0 {
            out.push(value as u8);
            return;
        }
        out.push((value & 0x7F) as u8 | 0x80);
        value >>= 7;
    }
}

fn write_utf(out: &mut Vec<u8>, text: &str) -> Result<(), CommandTreePacketError> {
    let units = text.encode_utf16().count();
    if units > MAX_STRING_UNITS {
        return Err(CommandTreePacketError::StringTooLong { units });
    }
    write_varint(out, text.len() as i32);
    out.extend_from_slice(text.as_bytes());
    Ok(())
}

fn index_i32(value: usize) -> Result<i32, CommandTreePacketError> {
    i32::try_from(value).map_err(|_| CommandTreePacketError::TooManyNodes)
}

/// The body of `ClientboundCommandsPacket` for the tree below `root`.
pub fn encode_commands_packet<S: Clone>(tree: &CommandTree<S>, root: NodeId) -> Result<Vec<u8>, CommandTreePacketError> {
    let mut classifier = Classifier::new(tree);
    // enumerateNodes: breadth first, redirect targets queued after the children; a node whose
    // equivalence class already has an index is skipped.
    let mut index_of_class: HashMap<usize, usize> = HashMap::new();
    let mut representatives: Vec<NodeId> = Vec::new();
    let mut queue: VecDeque<NodeId> = VecDeque::from([root]);
    while let Some(node) = queue.pop_front() {
        let class = classifier.class(node);
        if index_of_class.contains_key(&class) {
            continue;
        }
        index_of_class.insert(class, representatives.len());
        representatives.push(node);
        queue.extend(tree.children(node));
        if let Some(redirect) = tree.redirect(node) {
            queue.push_back(redirect);
        }
    }

    let mut out = Vec::new();
    write_varint(&mut out, index_i32(representatives.len())?);
    for node in &representatives {
        let node = *node;
        let mut flags = 0u8;
        let redirect = tree.redirect(node).map(|target| index_of_class[&classifier.class(target)]);
        if redirect.is_some() {
            flags |= FLAG_REDIRECT;
        }
        if tree.command(node).is_some() {
            flags |= FLAG_EXECUTABLE;
        }
        match tree.kind(node) {
            NodeKind::Root => {}
            NodeKind::Argument { .. } => {
                flags |= FLAG_ARGUMENT;
                if tree.has_custom_suggestions(node) {
                    flags |= FLAG_CUSTOM_SUGGESTIONS;
                }
            }
            NodeKind::Literal { .. } => flags |= FLAG_LITERAL,
        }
        out.push(flags);
        let children = tree.children(node);
        write_varint(&mut out, index_i32(children.len())?);
        for child in children {
            write_varint(&mut out, index_i32(index_of_class[&classifier.class(child)])?);
        }
        if let Some(redirect) = redirect {
            write_varint(&mut out, index_i32(redirect)?);
        }
        match tree.kind(node) {
            NodeKind::Root => {}
            NodeKind::Literal { literal, .. } => write_utf(&mut out, literal)?,
            NodeKind::Argument { name, argument_type } => {
                write_utf(&mut out, name)?;
                let info = argument_type
                    .network_info()
                    .ok_or_else(|| CommandTreePacketError::UnsupportedArgumentType { node: name.clone() })?;
                write_varint(&mut out, info.registry_id);
                out.extend_from_slice(&info.template);
                if let Some(provider) = tree.suggestion_provider_name(node) {
                    write_utf(&mut out, provider)?;
                }
            }
        }
    }
    write_varint(&mut out, index_i32(index_of_class[&classifier.class(root)])?);
    Ok(out)
}

/// `Commands.fillUsableCommands`: the copy of `tree` a player (or other source) is sent.
///
/// Only the children the source may use are copied, depth first. Each copy has no requirement,
/// every executable node shares one no-op command, unregistered suggestion providers are replaced
/// by the ask-the-server provider, and a redirect points at the copy of its target. The original
/// finds that copy in a `HashMap` keyed by `CommandNode.equals` (not by identity) that
/// `sendCommands` seeds with the root, and `put` overwrites an equal key: so a target resolves to
/// the *latest* copied node equal to it, and a target not copied yet drops the redirect.
pub fn usable_tree<S: Clone + 'static>(tree: &CommandTree<S>, root: NodeId, source: &S) -> (CommandTree<S>, NodeId) {
    let mut copy = CommandTree::default();
    let copy_root = copy.new_root();
    let no_op: Command<S> = Rc::new(|_| Ok(0));
    let ask_server: SuggestionProvider<S> = Rc::new(|_, _| Ok(Suggestions::empty()));
    let mut classifier = Classifier::new(tree);
    let mut copies: HashMap<usize, NodeId> = HashMap::new();
    copies.insert(classifier.class(root), copy_root);
    fill_usable(tree, root, copy_root, source, &mut copy, &no_op, &ask_server, &mut classifier, &mut copies);
    (copy, copy_root)
}

#[allow(clippy::too_many_arguments)]
fn fill_usable<S: Clone + 'static>(
    tree: &CommandTree<S>,
    from: NodeId,
    into: NodeId,
    source: &S,
    copy: &mut CommandTree<S>,
    no_op: &Command<S>,
    ask_server: &SuggestionProvider<S>,
    classifier: &mut Classifier<'_, S>,
    copies: &mut HashMap<usize, NodeId>,
) {
    for child in tree.children(from) {
        if !tree.can_use(child, source) {
            continue;
        }
        let mut builder = tree
            .create_builder(child)
            .with_requirement(Rc::new(|_| true))
            .with_swapped_suggestions(Rc::clone(ask_server));
        if builder.has_command() {
            builder = builder.executes(Rc::clone(no_op));
        }
        if let Some(target) = tree.redirect(child) {
            builder = builder.with_redirect_target(copies.get(&classifier.class(target)).copied());
        }
        let built = copy.build(&builder);
        copies.insert(classifier.class(child), built);
        copy.add_child(into, built);
        if !tree.children(child).is_empty() {
            fill_usable(tree, child, built, source, copy, no_op, ask_server, classifier, copies);
        }
    }
}
