//! Bounded Minecraft Java 1.21 administration-command wire interface.
//!
//! This is an explicitly selected command graph, not a general Brigadier engine.
//! Signed-command decoding preserves the unverified envelope; it never grants
//! permission to execute it or claims to validate secure chat.

use anyhow::{ensure, Result};

use crate::protocol::{Packet, Reader, Writer};

pub const UNSIGNED_COMMAND_PACKET_ID: i32 = 0x04;
pub const SIGNED_COMMAND_PACKET_ID: i32 = 0x05;
pub const SUGGESTION_REQUEST_PACKET_ID: i32 = 0x0b;
pub const SUGGESTIONS_PACKET_ID: i32 = 0x10;
pub const COMMAND_TREE_PACKET_ID: i32 = 0x11;

/// Packet decoding does not trim, remove a slash, or execute the command.
pub fn decode_unsigned_command(payload: &[u8]) -> Result<String> {
    let mut input = Reader::new(payload);
    let command = input.string(32767)?;
    input.finish()?;
    Ok(command)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgumentSignature {
    pub name: String,
    pub signature: [u8; 256],
}

/// Unverified fields from ServerboundChatCommandSignedPacket. The caller must
/// supply the real last-seen validator and command signing context before
/// accepting a signed command. Neither this type nor its decoder validates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedCommand {
    pub command: String,
    pub timestamp_millis: i64,
    pub salt: i64,
    pub argument_signatures: Vec<ArgumentSignature>,
    pub last_seen_offset: i32,
    pub acknowledged: [u8; 3],
}

pub fn decode_signed_command(payload: &[u8]) -> Result<SignedCommand> {
    let mut input = Reader::new(payload);
    let command = input.string(32767)?;
    let timestamp_millis = input.i64()?;
    let salt = input.i64()?;
    let count = input.varint()?;
    ensure!((0..=8).contains(&count), "invalid command signature count");
    let mut argument_signatures = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let name = input.string(16)?;
        let mut signature = [0; 256];
        signature.copy_from_slice(input.bytes(256)?);
        argument_signatures.push(ArgumentSignature { name, signature });
    }
    let last_seen_offset = input.varint()?;
    let mut acknowledged = [0; 3];
    acknowledged.copy_from_slice(input.bytes(3)?);
    input.finish()?;
    Ok(SignedCommand {
        command,
        timestamp_millis,
        salt,
        argument_signatures,
        last_seen_offset,
        acknowledged,
    })
}

/// StringUtil.isAllowedChatCharacter, evaluated on Java UTF-16 code units.
pub fn command_contains_illegal_characters(command: &str) -> bool {
    command
        .encode_utf16()
        .any(|c| c == 0x00a7 || c < 0x20 || c == 0x7f)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionRequest {
    pub id: i32,
    pub command: String,
}

pub fn decode_suggestion_request(payload: &[u8]) -> Result<SuggestionRequest> {
    let mut input = Reader::new(payload);
    let id = input.varint()?;
    let command = input.string(32500)?;
    input.finish()?;
    Ok(SuggestionRequest { id, command })
}

#[derive(Clone, Copy)]
enum Parser {
    Time,
    GreedyString,
    Word,
    Players,
    Profiles,
    Message,
}

enum NodeKind {
    Root,
    Literal(&'static str),
    Argument {
        name: &'static str,
        parser: Parser,
        ask_server: bool,
    },
}

struct Node {
    kind: NodeKind,
    executable: bool,
    children: Vec<usize>,
    redirect: Option<usize>,
}

struct Tree {
    nodes: Vec<Node>,
}

impl Tree {
    fn new() -> Self {
        Self {
            nodes: vec![Node {
                kind: NodeKind::Root,
                executable: false,
                children: Vec::new(),
                redirect: None,
            }],
        }
    }

    fn add(&mut self, parent: usize, kind: NodeKind, executable: bool) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node {
            kind,
            executable,
            children: Vec::new(),
            redirect: None,
        });
        self.nodes[parent].children.push(index);
        index
    }

    fn literal(&mut self, parent: usize, name: &'static str, executable: bool) -> usize {
        self.add(parent, NodeKind::Literal(name), executable)
    }

    fn argument(
        &mut self,
        parent: usize,
        name: &'static str,
        parser: Parser,
        ask_server: bool,
    ) -> usize {
        self.add(
            parent,
            NodeKind::Argument {
                name,
                parser,
                ask_server,
            },
            true,
        )
    }

    fn packet(self) -> Result<Packet> {
        let mut output = Writer::new();
        output.varint(i32::try_from(self.nodes.len())?);
        for node in &self.nodes {
            let (kind, custom) = match &node.kind {
                NodeKind::Root => (0, false),
                NodeKind::Literal(_) => (1, false),
                NodeKind::Argument { ask_server, .. } => (2, *ask_server),
            };
            let flags = kind | if node.executable { 4 } else { 0 } | if custom { 16 } else { 0 }
                | if node.redirect.is_some() { 8 } else { 0 };
            output.u8(flags).varint(i32::try_from(node.children.len())?);
            for child in &node.children {
                output.varint(i32::try_from(*child)?);
            }
            if let Some(redirect) = node.redirect {
                output.varint(i32::try_from(redirect)?);
            }
            match &node.kind {
                NodeKind::Root => {}
                NodeKind::Literal(name) => {
                    output.string(name);
                }
                NodeKind::Argument {
                    name,
                    parser,
                    ask_server,
                } => {
                    output.string(name);
                    match parser {
                        Parser::Time => { output.varint(42).i32(0); },
                        Parser::GreedyString => { output.varint(5).varint(2); },
                        Parser::Word => {
                            output.varint(5).varint(0);
                        }
                        // EntityArgument.players(): not single, players only.
                        Parser::Players => {
                            output.varint(6).u8(2);
                        }
                        Parser::Profiles => {
                            output.varint(7);
                        }
                        Parser::Message => {
                            output.varint(19);
                        }
                    }
                    if *ask_server {
                        output.string("minecraft:ask_server");
                    }
                }
            }
        }
        output.varint(0);
        Ok(Packet {
            id: COMMAND_TREE_PACKET_ID,
            payload: output.into_inner(),
        })
    }
}

/// The selected vanilla administration graph filtered by the real permission
/// level. All selected administration roots require level3; `list` is public.
/// MessageArgument nodes remain the actual signable parser19. Advertising them
/// does not make their signed transport executable without a signing owner.
fn administration_tree(permission_level: u8) -> Tree {
    let mut tree = Tree::new();
    let list = tree.literal(0, "list", true);
    tree.literal(list, "uuids", true);
    if permission_level >= 3 {
        let ban = tree.literal(0, "ban", false);
        let targets = tree.argument(ban, "targets", Parser::Profiles, false);
        tree.argument(targets, "reason", Parser::Message, false);

        let ban_ip = tree.literal(0, "ban-ip", false);
        let target = tree.argument(ban_ip, "target", Parser::Word, false);
        tree.argument(target, "reason", Parser::Message, false);

        let banlist = tree.literal(0, "banlist", true);
        tree.literal(banlist, "ips", true);
        tree.literal(banlist, "players", true);

        let deop = tree.literal(0, "deop", false);
        tree.argument(deop, "targets", Parser::Profiles, true);

        let kick = tree.literal(0, "kick", false);
        let targets = tree.argument(kick, "targets", Parser::Players, false);
        tree.argument(targets, "reason", Parser::Message, false);

        let op = tree.literal(0, "op", false);
        tree.argument(op, "targets", Parser::Profiles, true);

        let pardon = tree.literal(0, "pardon", false);
        tree.argument(pardon, "targets", Parser::Profiles, true);

        let pardon_ip = tree.literal(0, "pardon-ip", false);
        tree.argument(pardon_ip, "target", Parser::Word, true);

        let whitelist = tree.literal(0, "whitelist", false);
        for literal in ["on", "off", "list", "reload"] {
            tree.literal(whitelist, literal, true);
        }
        for literal in ["add", "remove"] {
            let branch = tree.literal(whitelist, literal, false);
            tree.argument(branch, "targets", Parser::Profiles, true);
        }
    }
    tree
}

pub fn administration_command_tree(permission_level: u8) -> Result<Packet> {
    administration_tree(permission_level).packet()
}

/// Scoreboard roots are usable in Main. Their full suggestion grammar belongs to WP-E3.
pub fn native_command_tree(permission_level: u8) -> Result<Packet> {
    let mut tree=administration_tree(permission_level);
    let teammsg=tree.literal(0,"teammsg",false);
    tree.argument(teammsg,"message",Parser::Message,false);
    let alias=tree.literal(0,"tm",false);
    tree.nodes[alias].redirect=Some(teammsg);
    let trigger=tree.literal(0,"trigger",false);
    tree.argument(trigger,"arguments",Parser::GreedyString,false);
    if permission_level>=2 {
        let time = tree.literal(0, "time", false);
        let set = tree.literal(time, "set", false);
        for name in ["day", "noon", "night", "midnight"] { tree.literal(set, name, true); }
        tree.argument(set, "time", Parser::Time, false);
        let add = tree.literal(time, "add", false);
        tree.argument(add, "time", Parser::Time, false);
        let query = tree.literal(time, "query", false);
        for name in ["daytime", "gametime", "day"] { tree.literal(query, name, true); }
        for root in ["scoreboard","team"] {
            let root=tree.literal(0,root,false);
            tree.argument(root,"arguments",Parser::GreedyString,false);
        }
    }
    tree.packet()
}

/// The time branch uses the actual dispatcher; administration keeps its existing route.
pub fn native_command_suggestions(
    request: &SuggestionRequest, permission_level: u8, context: &SuggestionContext<'_>,
) -> Result<Packet> {
    let leading = usize::from(request.command.starts_with('/'));
    let input = &request.command[leading..];
    if input == "time" || input.starts_with("time ") ||
        (!input.is_empty() && "time".starts_with(&input.to_lowercase())) {
        if permission_level < 2 { return encode_suggestions(request.id, 0, 0, &[]); }
        let suggestions = crate::commands::time_command::suggestions(input, permission_level)
            .map_err(|error| anyhow::anyhow!("time suggestions: {error:?}"))?;
        let matches = suggestions.list().iter().map(|value| value.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty() { return encode_suggestions(request.id, 0, 0, &[]); }
        let range = suggestions.range();
        return encode_suggestions(request.id, range.start() + leading, range.end() - range.start(), &matches);
    }
    administration_suggestions_with_time(request, permission_level, context, true)
}

/// Snapshot supplied by the real access-list/player owners. In particular,
/// `op_candidates` and `whitelist_add_candidates` must be computed by UUID-based
/// source predicates; stored names alone cannot prove that a profile is eligible.
#[derive(Debug, Clone, Copy, Default)]
pub struct SuggestionContext<'a> {
    pub online_player_names: &'a [String],
    pub operator_names: &'a [String],
    pub banned_player_names: &'a [String],
    pub banned_ip_addresses: &'a [String],
    pub whitelisted_names: &'a [String],
    pub op_candidates: &'a [String],
    pub whitelist_add_candidates: &'a [String],
}

/// Encode the range and tooltip-free matches used by the selected admin
/// commands. The listener's source limit is1000 results, applied after sorting.
pub fn encode_suggestions(
    id: i32,
    start: usize,
    length: usize,
    matches: &[String],
) -> Result<Packet> {
    ensure!(
        start <= 32500 && length <= 32500 && start + length <= 32500,
        "invalid suggestion range"
    );
    let count = matches.len().min(1000);
    let mut output = Writer::new();
    output
        .varint(id)
        .varint(i32::try_from(start)?)
        .varint(i32::try_from(length)?)
        .varint(count as i32);
    for value in matches.iter().take(count) {
        ensure!(
            value.encode_utf16().count() <= 32767,
            "suggestion exceeds Java string length"
        );
        output.string(value).bool(false);
    }
    Ok(Packet {
        id: SUGGESTIONS_PACKET_ID,
        payload: output.into_inner(),
    })
}

/// Bounded completions for the selected literal paths and ordinary profile/IP
/// names. This does not implement entity-selector option completion or a general
/// Brigadier parser. Unsupported argument contexts yield empty suggestions.
pub fn administration_suggestions(
    request: &SuggestionRequest,
    permission_level: u8,
    context: &SuggestionContext<'_>,
) -> Result<Packet> {
    administration_suggestions_with_time(request, permission_level, context, false)
}

fn administration_suggestions_with_time(
    request: &SuggestionRequest,
    permission_level: u8,
    context: &SuggestionContext<'_>,
    include_time: bool,
) -> Result<Packet> {
    ensure!(
        request.command.encode_utf16().count() <= 32500,
        "suggestion request exceeds Java string length"
    );
    let leading = usize::from(request.command.starts_with('/'));
    let command = &request.command[leading..];
    let last_separator = command.rfind(' ');
    let (completed, remaining, start_byte) = match last_separator {
        Some(index) => (
            &command[..index],
            &command[index + 1..],
            leading + index + 1,
        ),
        None => ("", command, leading),
    };
    let start = request.command[..start_byte].encode_utf16().count();
    let length = remaining.encode_utf16().count();
    let mut matches = Vec::new();
    let lowered = remaining.to_lowercase();
    let mut literals: &[&str] = &[];
    let mut names: &[String] = &[];
    if last_separator.is_none() {
        literals = if permission_level >= 3 {
            &[
                "ban",
                "ban-ip",
                "banlist",
                "deop",
                "kick",
                "list",
                "op",
                "pardon",
                "pardon-ip",
                "whitelist",
            ]
        } else {
            &["list"]
        };
    } else if completed == "list" {
        literals = &["uuids"];
    } else if permission_level >= 3 {
        match completed {
            "banlist" => literals = &["ips", "players"],
            "whitelist" => literals = &["add", "list", "off", "on", "reload", "remove"],
            "ban" | "kick" => names = context.online_player_names,
            "deop" => names = context.operator_names,
            "op" => names = context.op_candidates,
            "pardon" => names = context.banned_player_names,
            "pardon-ip" => names = context.banned_ip_addresses,
            "whitelist add" => names = context.whitelist_add_candidates,
            "whitelist remove" => names = context.whitelisted_names,
            _ => {}
        }
    }
    for literal in literals {
        if literal.starts_with(&lowered) && *literal != remaining {
            matches.push((*literal).to_owned());
        }
    }
    for name in names {
        if name != remaining && matches_substr(&lowered, &name.to_lowercase()) {
            matches.push(name.clone());
        }
    }
    if include_time && permission_level >= 2 && last_separator.is_none()
        && "time".starts_with(&lowered) && remaining != "time" {
        matches.push("time".to_owned());
    }
    // Actual Minecraft profile names, IPs and literals are ASCII. UTF-16 ordering
    // supplies Java String ordering for any explicitly supplied extended values.
    matches.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    matches.dedup();
    matches.sort_by(|a, b| {
        a.to_lowercase()
            .encode_utf16()
            .cmp(b.to_lowercase().encode_utf16())
    });
    if matches.is_empty() {
        // Brigadier Suggestions.empty() has StringRange.at(0), not the failing
        // argument's candidate start. No fabricated completion range is sent.
        encode_suggestions(request.id, 0, 0, &[])
    } else {
        encode_suggestions(request.id, start, length, &matches)
    }
}

/// SharedSuggestionProvider.matchesSubStr: starts at zero or after `.` / `_`.
fn matches_substr(needle: &str, candidate: &str) -> bool {
    if candidate.starts_with(needle) {
        return true;
    }
    candidate
        .char_indices()
        .any(|(i, c)| (c == '.' || c == '_') && candidate[i + c.len_utf8()..].starts_with(needle))
}
