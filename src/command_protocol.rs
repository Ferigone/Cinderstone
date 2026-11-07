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
    NonnegativeInteger,
    SignedInteger,
    Time(i32),
    BorderDouble,
    NonnegativeFloat,
    Vec2,
    BlockPos,
    Angle,
    Objective,
    ResourceLocation,
    GreedyString,
    Word,
    Players,
    SinglePlayer,
    Component,
    Profiles,
    Message,
}

enum NodeKind {
    Root,
    Literal(&'static str),
    Argument {
        name: &'static str,
        parser: Parser,
        suggestion: Option<&'static str>,
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
                suggestion: ask_server.then_some("minecraft:ask_server"),
            },
            true,
        )
    }

    fn argument_named(&mut self,parent:usize,name:&'static str,parser:Parser,suggestion:&'static str)->usize{
        self.add(parent,NodeKind::Argument{name,parser,suggestion:Some(suggestion)},true)
    }

    fn packet(self) -> Result<Packet> {
        let mut output = Writer::new();
        output.varint(i32::try_from(self.nodes.len())?);
        for node in &self.nodes {
            let (kind, custom) = match &node.kind {
                NodeKind::Root => (0, false),
                NodeKind::Literal(_) => (1, false),
                NodeKind::Argument { suggestion, .. } => (2, suggestion.is_some()),
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
                    suggestion,
                } => {
                    output.string(name);
                    match parser {
                        Parser::NonnegativeInteger => { output.varint(3).u8(1).i32(0); },
                        Parser::SignedInteger => {output.varint(3).u8(0);},
                        Parser::Time(minimum) => { output.varint(42).i32(*minimum); },
                        Parser::BorderDouble => {output.varint(2).u8(3).f64(-crate::border::MAX_SIZE).f64(crate::border::MAX_SIZE);},
                        Parser::NonnegativeFloat => {output.varint(1).u8(1).f32(0.);},
                        Parser::Vec2 => {output.varint(11);},
                        Parser::BlockPos => {output.varint(8);},
                        Parser::Angle => {output.varint(27);},
                        Parser::Objective => {output.varint(23);},
                        Parser::ResourceLocation => {output.varint(35);},
                        Parser::GreedyString => { output.varint(5).varint(2); },
                        Parser::Word => {
                            output.varint(5).varint(0);
                        }
                        // EntityArgument.players(): not single, players only.
                        Parser::SinglePlayer => {output.varint(6).u8(3);},
                        Parser::Players => {
                            output.varint(6).u8(2);
                        }
                        Parser::Profiles => {
                            output.varint(7);
                        }
                        Parser::Component => {output.varint(17);},
                        Parser::Message => {
                            output.varint(19);
                        }
                    }
                    if let Some(name)=suggestion {output.string(name);}
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
    if permission_level >= 4 {
        let save = tree.literal(0, "save-all", true);
        tree.literal(save, "flush", true);
    }
    if permission_level >= 3 {
        let idle = tree.literal(0, "setidletimeout", false);
        tree.argument(idle, "minutes", Parser::NonnegativeInteger, false);
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
    if permission_level>=4 {tree.literal(0,"stop",true);}
    let msg=tree.literal(0,"msg",false);let targets=tree.argument(msg,"targets",Parser::Players,false);tree.nodes[targets].executable=false;tree.argument(targets,"message",Parser::Message,false);
    for name in ["tell","w"]{let alias=tree.literal(0,name,false);tree.nodes[alias].redirect=Some(msg);}
    let emote=tree.literal(0,"me",false);tree.argument(emote,"action",Parser::Message,false);
    let teammsg=tree.literal(0,"teammsg",false);
    tree.argument(teammsg,"message",Parser::Message,false);
    let alias=tree.literal(0,"tm",false);
    tree.nodes[alias].redirect=Some(teammsg);
    let trigger=tree.literal(0,"trigger",false);
    let objective=tree.argument(trigger,"objective",Parser::Objective,true);
    for name in ["add","set"] {let branch=tree.literal(objective,name,false);tree.argument(branch,"value",Parser::SignedInteger,false);}
    if permission_level>=2 {
        let title=tree.literal(0,"title",false);let targets=tree.argument(title,"targets",Parser::Players,false);tree.nodes[targets].executable=false;
        for name in ["clear","reset"]{tree.literal(targets,name,true);}
        for name in ["title","subtitle","actionbar"]{let branch=tree.literal(targets,name,false);tree.argument(branch,"title",Parser::Component,false);}
        let times=tree.literal(targets,"times",false);let fade=tree.argument(times,"fadeIn",Parser::Time(0),false);tree.nodes[fade].executable=false;let stay=tree.argument(fade,"stay",Parser::Time(0),false);tree.nodes[stay].executable=false;tree.argument(stay,"fadeOut",Parser::Time(0),false);
        let tellraw=tree.literal(0,"tellraw",false);let target=tree.argument(tellraw,"targets",Parser::Players,false);tree.nodes[target].executable=false;tree.argument(target,"message",Parser::Component,false);
        let experience=tree.literal(0,"experience",false);
        for (name,parser) in [("add",Parser::SignedInteger),("set",Parser::NonnegativeInteger)]{let branch=tree.literal(experience,name,false);let target=tree.argument(branch,"targets",Parser::Players,false);tree.nodes[target].executable=false;let amount=tree.argument(target,"amount",parser,false);for unit in ["points","levels"]{tree.literal(amount,unit,true);}}
        let query=tree.literal(experience,"query",false);let target=tree.argument(query,"targets",Parser::SinglePlayer,false);tree.nodes[target].executable=false;for unit in ["points","levels"]{tree.literal(target,unit,true);}
        let xp=tree.literal(0,"xp",false);tree.nodes[xp].redirect=Some(experience);
        let recipe=tree.literal(0,"recipe",false);for name in ["give","take"]{let branch=tree.literal(recipe,name,false);let target=tree.argument(branch,"targets",Parser::Players,false);tree.nodes[target].executable=false;tree.argument_named(target,"recipe",Parser::ResourceLocation,"minecraft:all_recipes");tree.literal(target,"*",true);}
        let say=tree.literal(0,"say",false);tree.argument(say,"message",Parser::Message,false);
        let spawn=tree.literal(0,"setworldspawn",true);
        let pos=tree.argument(spawn,"pos",Parser::BlockPos,false);
        tree.argument(pos,"angle",Parser::Angle,false);
        tree.literal(0, "seed", true);
        let border=tree.literal(0,"worldborder",false);
        for name in ["add","set"] {
            let branch=tree.literal(border,name,false);
            let distance=tree.argument(branch,"distance",Parser::BorderDouble,false);
            tree.argument(distance,"time",Parser::NonnegativeInteger,false);
        }
        let center=tree.literal(border,"center",false);
        tree.argument(center,"pos",Parser::Vec2,false);
        let damage=tree.literal(border,"damage",false);
        for (name,arg) in [("amount","damagePerBlock"),("buffer","distance")] {
            let branch=tree.literal(damage,name,false);tree.argument(branch,arg,Parser::NonnegativeFloat,false);
        }
        tree.literal(border,"get",true);
        let warning=tree.literal(border,"warning",false);
        for name in ["distance","time"] {let branch=tree.literal(warning,name,false);tree.argument(branch,name,Parser::NonnegativeInteger,false);}
        let difficulty = tree.literal(0,"difficulty",true);
        for name in crate::commands::difficulty_command::NAMES { tree.literal(difficulty,name,true); }
        let weather = tree.literal(0, "weather", false);
        for kind in ["clear", "rain", "thunder"] {
            let branch = tree.literal(weather, kind, true);
            tree.argument(branch, "duration", Parser::Time(1), false);
        }
        let time = tree.literal(0, "time", false);
        let set = tree.literal(time, "set", false);
        for name in ["day", "noon", "night", "midnight"] { tree.literal(set, name, true); }
        tree.argument(set, "time", Parser::Time(0), false);
        let add = tree.literal(time, "add", false);
        tree.argument(add, "time", Parser::Time(0), false);
        let query = tree.literal(time, "query", false);
        for name in ["daytime", "gametime", "day"] { tree.literal(query, name, true); }
        for root in ["scoreboard","team"] {
            let root=tree.literal(0,root,false);
            tree.argument(root,"arguments",Parser::GreedyString,false);
        }
    }
    tree.packet()
}

pub fn experience_suggestions(request:&SuggestionRequest,permission:u8,names:&[String])->Result<Packet>{
    let leading=usize::from(request.command.starts_with('/'));
    let s=crate::commands::experience_command::suggestions(&request.command[leading..],permission,names).map_err(|e|anyhow::anyhow!("XP suggestions: {e:?}"))?;
    let matches=s.list().iter().map(|v|v.text().to_owned()).collect::<Vec<_>>();if matches.is_empty(){return encode_suggestions(request.id,0,0,&[]);}
    let r=s.range();encode_suggestions(request.id,r.start()+leading,r.end()-r.start(),&matches)
}

/// Recipe names come from the retained captured catalogue; target names are live.
pub fn recipe_suggestions(request:&SuggestionRequest,permission:u8,names:&[String])->Result<Packet>{
    let ids=crate::server_recipe_book::VanillaRecipeCatalogue::get().iter().map(|r|r.id.as_str().to_owned()).collect::<Vec<_>>();
    let leading=usize::from(request.command.starts_with('/'));
    let s=crate::commands::recipe_command::suggestions(&request.command[leading..],permission,names,&ids).map_err(|e|anyhow::anyhow!("recipe suggestions: {e:?}"))?;
    let matches=s.list().iter().map(|v|v.text().to_owned()).collect::<Vec<_>>();if matches.is_empty(){return encode_suggestions(request.id,0,0,&[]);}
    let r=s.range();encode_suggestions(request.id,r.start()+leading,r.end()-r.start(),&matches)
}

/// Completion uses the actual current admitted roster supplied by Main.
pub fn msg_suggestions(request:&SuggestionRequest,names:&[String])->Result<Packet>{
    let leading=usize::from(request.command.starts_with('/'));
    let s=crate::commands::msg_command::suggestions(&request.command[leading..],names).map_err(|e|anyhow::anyhow!("whisper suggestions: {e:?}"))?;
    let matches=s.list().iter().map(|v|v.text().to_owned()).collect::<Vec<_>>();if matches.is_empty(){return encode_suggestions(request.id,0,0,&[]);}
    let r=s.range();encode_suggestions(request.id,r.start()+leading,r.end()-r.start(),&matches)
}

/// The caller supplies trigger objectives enabled by the actual player's score owner.
pub fn trigger_suggestions(request:&SuggestionRequest,objectives:&[String])->Result<Packet>{
    let leading=usize::from(request.command.starts_with('/'));
    let s=crate::commands::trigger_command::suggestions(&request.command[leading..],objectives)
        .map_err(|e|anyhow::anyhow!("trigger suggestions: {e:?}"))?;
    let matches=s.list().iter().map(|v|v.text().to_owned()).collect::<Vec<_>>();
    if matches.is_empty(){return encode_suggestions(request.id,0,0,&[]);}
    let r=s.range();encode_suggestions(request.id,r.start()+leading,r.end()-r.start(),&matches)
}

/// Native branches use their dispatchers; root suggestions retain administration.
pub fn native_command_suggestions(
    request: &SuggestionRequest, permission_level: u8, context: &SuggestionContext<'_>,
) -> Result<Packet> {
    let leading = usize::from(request.command.starts_with('/'));
    let input = &request.command[leading..];
    if input == "setworldspawn" || input.starts_with("setworldspawn ") {
        let suggestions=crate::commands::setworldspawn_command::suggestions(input,permission_level)
            .map_err(|error|anyhow::anyhow!("setworldspawn suggestions: {error:?}"))?;
        let matches=suggestions.list().iter().map(|s|s.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty(){return encode_suggestions(request.id,0,0,&[]);}
        let range=suggestions.range();
        return encode_suggestions(request.id,range.start()+leading,range.end()-range.start(),&matches);
    }
    if input == "stop" || input.starts_with("stop ") {
        let suggestions=crate::commands::stop_command::suggestions(input,permission_level)
            .map_err(|error|anyhow::anyhow!("stop suggestions: {error:?}"))?;
        let matches=suggestions.list().iter().map(|s|s.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty(){return encode_suggestions(request.id,0,0,&[]);}
        let range=suggestions.range();
        return encode_suggestions(request.id,range.start()+leading,range.end()-range.start(),&matches);
    }
    if input == "pardon-ip" || input.starts_with("pardon-ip ") {
        let suggestions = crate::commands::pardon_ip_command::suggestions(input, permission_level, context.banned_ip_addresses)
            .map_err(|error| anyhow::anyhow!("pardon-ip suggestions: {error:?}"))?;
        let matches = suggestions.list().iter().map(|value| value.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty() { return encode_suggestions(request.id, 0, 0, &[]); }
        let range = suggestions.range();
        return encode_suggestions(request.id, range.start() + leading, range.end() - range.start(), &matches);
    }
    if input == "save-all" || input.starts_with("save-all ") {
        let suggestions = crate::commands::save_all_command::suggestions(input, permission_level)
            .map_err(|error| anyhow::anyhow!("save suggestions: {error:?}"))?;
        let matches = suggestions.list().iter().map(|value| value.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty() { return encode_suggestions(request.id, 0, 0, &[]); }
        let range = suggestions.range();
        return encode_suggestions(request.id, range.start() + leading, range.end() - range.start(), &matches);
    }
    if input == "setidletimeout" || input.starts_with("setidletimeout ") {
        let suggestions = crate::commands::idle_command::suggestions(input, permission_level)
            .map_err(|error| anyhow::anyhow!("idle suggestions: {error:?}"))?;
        let matches = suggestions.list().iter().map(|value| value.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty() { return encode_suggestions(request.id, 0, 0, &[]); }
        let range = suggestions.range();
        return encode_suggestions(request.id, range.start() + leading, range.end() - range.start(), &matches);
    }
    if input == "list" || input.starts_with("list ") {
        let suggestions = crate::commands::list_command::suggestions(input)
            .map_err(|error| anyhow::anyhow!("list suggestions: {error:?}"))?;
        let matches = suggestions.list().iter().map(|value| value.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty() { return encode_suggestions(request.id, 0, 0, &[]); }
        let range = suggestions.range();
        return encode_suggestions(request.id, range.start() + leading, range.end() - range.start(), &matches);
    }
    if input == "seed" || input.starts_with("seed ") {
        if permission_level < 2 { return encode_suggestions(request.id, 0, 0, &[]); }
        let suggestions = crate::commands::seed_command::suggestions(input, permission_level)
            .map_err(|error| anyhow::anyhow!("seed suggestions: {error:?}"))?;
        let matches = suggestions.list().iter().map(|value| value.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty() { return encode_suggestions(request.id, 0, 0, &[]); }
        let range = suggestions.range();
        return encode_suggestions(request.id, range.start() + leading, range.end() - range.start(), &matches);
    }
    if input == "banlist" || input.starts_with("banlist ") {
        let suggestions=crate::commands::banlist_command::suggestions(input,permission_level)
            .map_err(|error|anyhow::anyhow!("banlist suggestions: {error:?}"))?;
        let matches=suggestions.list().iter().map(|s|s.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty(){return encode_suggestions(request.id,0,0,&[]);}
        let range=suggestions.range();
        return encode_suggestions(request.id,range.start()+leading,range.end()-range.start(),&matches);
    }
    if input == "worldborder" || input.starts_with("worldborder ") {
        let suggestions=crate::commands::worldborder_command::suggestions(input,permission_level)
            .map_err(|error|anyhow::anyhow!("worldborder suggestions: {error:?}"))?;
        let matches=suggestions.list().iter().map(|value|value.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty(){return encode_suggestions(request.id,0,0,&[]);}
        let range=suggestions.range();
        return encode_suggestions(request.id,range.start()+leading,range.end()-range.start(),&matches);
    }
    if input == "difficulty" || input.starts_with("difficulty ") {
        let suggestions = crate::commands::difficulty_command::suggestions(input,permission_level)
            .map_err(|error|anyhow::anyhow!("difficulty suggestions: {error:?}"))?;
        let matches = suggestions.list().iter().map(|value|value.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty() { return encode_suggestions(request.id,0,0,&[]); }
        let range = suggestions.range();
        return encode_suggestions(request.id,range.start()+leading,range.end()-range.start(),&matches);
    }
    if input == "weather" || input.starts_with("weather ") {
        if permission_level < 2 { return encode_suggestions(request.id, 0, 0, &[]); }
        let suggestions = crate::commands::weather_command::suggestions(input, permission_level)
            .map_err(|error| anyhow::anyhow!("weather suggestions: {error:?}"))?;
        let matches = suggestions.list().iter().map(|value| value.text().to_owned()).collect::<Vec<_>>();
        if matches.is_empty() { return encode_suggestions(request.id, 0, 0, &[]); }
        let range = suggestions.range();
        return encode_suggestions(request.id, range.start() + leading, range.end() - range.start(), &matches);
    }
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
                "setidletimeout",
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
    if include_time && permission_level >= 2 && last_separator.is_none()
        && "weather".starts_with(&lowered) && remaining != "weather" {
        matches.push("weather".to_owned());
    }
    if include_time && permission_level >= 2 && last_separator.is_none()
        && "seed".starts_with(&lowered) && remaining != "seed" {
        matches.push("seed".to_owned());
    }
    if include_time && permission_level >= 2 && last_separator.is_none()
        && "difficulty".starts_with(&lowered) && remaining != "difficulty" {
        matches.push("difficulty".to_owned());
    }
    if include_time && permission_level>=2 && last_separator.is_none()
        && "worldborder".starts_with(&lowered) && remaining!="worldborder" {matches.push("worldborder".to_owned());}
    if include_time && permission_level>=2 && last_separator.is_none()
        && "setworldspawn".starts_with(&lowered) && remaining!="setworldspawn" {matches.push("setworldspawn".to_owned());}
    if include_time && permission_level>=2 && last_separator.is_none(){for name in ["experience","xp","tellraw","title"]{if name.starts_with(&lowered)&&remaining!=name{matches.push(name.to_owned());}}}
    if include_time && permission_level>=2 && last_separator.is_none() && "recipe".starts_with(&lowered) && remaining!="recipe" {matches.push("recipe".to_owned());}
    if include_time && permission_level>=2 && last_separator.is_none() && "say".starts_with(&lowered) && remaining!="say" {matches.push("say".to_owned());}
    if include_time && last_separator.is_none() {for name in ["msg","tell","w"]{if name.starts_with(&lowered)&&remaining!=name{matches.push(name.to_owned());}}}
    if include_time && last_separator.is_none() && "me".starts_with(&lowered) && remaining!="me" {matches.push("me".to_owned());}
    if include_time && last_separator.is_none() && "trigger".starts_with(&lowered) && remaining!="trigger" {matches.push("trigger".to_owned());}
    if include_time && permission_level >= 4 && last_separator.is_none() && "save-all".starts_with(&lowered) && remaining != "save-all" {
        matches.push("save-all".to_owned());
    }
    if include_time && permission_level>=4 && last_separator.is_none() && "stop".starts_with(&lowered) && remaining!="stop" {
        matches.push("stop".to_owned());
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

#[cfg(test)]
mod native_weather_tests {
    use super::*;

    #[test]
    fn difficulty_prefix_preserves_deop_and_permission_ranges() {
        for prefix in ["d", "/d"] {
            for (permission, expected) in [(0,vec![]),(1,vec![]),(2,vec!["difficulty"]),
                (3,vec!["deop","difficulty"]),(4,vec!["deop","difficulty"])] {
                let packet=native_command_suggestions(&SuggestionRequest {id:10,command:prefix.into()},
                    permission,&SuggestionContext::default()).unwrap();
                let mut reader=Reader::new(&packet.payload);
                assert_eq!(reader.varint().unwrap(),10);
                let range=(reader.varint().unwrap(),reader.varint().unwrap());
                let count=reader.varint().unwrap();
                let mut texts=Vec::new();
                for _ in 0..count {texts.push(reader.string(32767).unwrap());assert!(!reader.bool().unwrap());}
                reader.finish().unwrap();
                assert_eq!(texts,expected,"permission {permission}, {prefix}");
                assert_eq!(range,if expected.is_empty(){(0,0)}else{(i32::from(prefix.starts_with('/')),1)});
            }
        }
    }

    #[test]
    fn weather_root_completion_keeps_existing_whitelist_and_permissions() {
        for (permission, expected) in [
            (1, vec![]), (2, vec!["weather", "worldborder"]), (3, vec!["weather", "whitelist", "worldborder"]),
        ] {
            let packet = native_command_suggestions(&SuggestionRequest {
                id: 9, command: "/w".into(),
            }, permission, &SuggestionContext::default()).unwrap();
            let mut reader = Reader::new(&packet.payload);
            assert_eq!(reader.varint().unwrap(), 9);
            let start = reader.varint().unwrap();
            let length = reader.varint().unwrap();
            let count = reader.varint().unwrap();
            let mut texts = Vec::new();
            for _ in 0..count {
                texts.push(reader.string(32767).unwrap());
                assert!(!reader.bool().unwrap());
            }
            reader.finish().unwrap();
            assert_eq!(texts, expected);
            assert_eq!((start, length), if expected.is_empty() { (0, 0) } else { (1, 1) });
        }
    }
}

pub fn tellraw_suggestions(request:&SuggestionRequest,permission:u8,names:&[String])->Result<Packet>{
    let leading=usize::from(request.command.starts_with('/'));
    let s=crate::commands::tellraw_command::suggestions(&request.command[leading..],permission,names).map_err(|e|anyhow::anyhow!("command suggestions: {e:?}"))?;
    let matches=s.list().iter().map(|v|v.text().to_owned()).collect::<Vec<_>>();if matches.is_empty(){return encode_suggestions(request.id,0,0,&[]);}
    let r=s.range();encode_suggestions(request.id,r.start()+leading,r.end()-r.start(),&matches)
}

pub fn title_suggestions(request:&SuggestionRequest,permission:u8,names:&[String])->Result<Packet>{
    let leading=usize::from(request.command.starts_with('/'));
    let s=crate::commands::title_command::suggestions(&request.command[leading..],permission,names).map_err(|e|anyhow::anyhow!("command suggestions: {e:?}"))?;
    let matches=s.list().iter().map(|v|v.text().to_owned()).collect::<Vec<_>>();if matches.is_empty(){return encode_suggestions(request.id,0,0,&[]);}
    let r=s.range();encode_suggestions(request.id,r.start()+leading,r.end()-r.start(),&matches)
}
