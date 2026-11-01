//! Native player-save helpers used by the main lifecycle integration.
use anyhow::{ensure, Context, Result};
use mc121_rust::{
    nbt::Tag,
    player_data::{uuid_string, PlayerDataStore, PlayerLifecycle},
    server_recipe_book::{
        RecipeLoadIssue, RecipeLoadLog, ServerRecipeBook, VanillaRecipeCatalogue,
    },
};
use serde_json::json;

/// Only UUIDs backed by complete, validated native records enter the known
/// player registry. This neither activates a player nor creates a stat owner.
pub(super) fn load_known_native_players(store: &PlayerDataStore) -> Result<Vec<[u8; 16]>> {
    let directory = match std::fs::read_dir(store.directory()) {
        Ok(directory) => directory,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).context("read native player-save directory"),
    };
    let mut candidates = Vec::new();
    for entry in directory {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            eprintln!(
                "{}",
                json!({"event":"native_player_registry_record_rejected","path":format!("{:?}",entry.path()),"reason":"non-UTF8 player-save filename"})
            );
            continue;
        };
        if !name.ends_with(".native.json") {
            continue;
        }
        candidates.push((name.to_owned(), entry.path()));
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    let mut known = Vec::with_capacity(candidates.len());
    for (name, path) in candidates {
        let result = (|| -> Result<[u8; 16]> {
            let uuid = parse_filename(&name)?;
            store
                .load(uuid)?
                .context("player-save record disappeared while loading native registry")?;
            Ok(uuid)
        })();
        match result {
            Ok(uuid) => known.push(uuid),
            Err(error) => eprintln!(
                "{}",
                json!({"event":"native_player_registry_record_rejected","path":path,"reason":format!("{error:#}")})
            ),
        }
    }
    Ok(known)
}

fn parse_filename(name: &str) -> Result<[u8; 16]> {
    let text = name
        .strip_suffix(".native.json")
        .context("invalid native player-save suffix")?;
    ensure!(text.len() == 36, "invalid native UUID filename length");
    let bytes = text.as_bytes();
    ensure!(
        [8, 13, 18, 23]
            .into_iter()
            .all(|index| bytes[index] == b'-'),
        "invalid native UUID separators"
    );
    let hex = bytes
        .iter()
        .copied()
        .filter(|byte| *byte != b'-')
        .collect::<Vec<_>>();
    ensure!(hex.len() == 32, "invalid native UUID filename");
    let mut uuid = [0; 16];
    for (target, pair) in uuid.iter_mut().zip(hex.chunks_exact(2)) {
        let pair = std::str::from_utf8(pair)?;
        *target = u8::from_str_radix(pair, 16).context("invalid native UUID hex digit")?;
    }
    ensure!(
        uuid_string(uuid) == text,
        "noncanonical native UUID filename"
    );
    Ok(uuid)
}

struct NativeRecipeLoadLog;
impl RecipeLoadLog for NativeRecipeLoadLog {
    fn issue(&mut self, _: &mut ServerRecipeBook, issue: RecipeLoadIssue) -> Result<()> {
        let kind = match &issue {
            RecipeLoadIssue::Unrecognized(_) => "unrecognized_recipe",
            RecipeLoadIssue::ImproperlyFormatted(_) => "improperly_formatted_recipe",
        };
        eprintln!(
            "{}",
            json!({"event":"native_recipe_book_load_issue","issue_kind":kind,"message_template":issue.message_template(),"argument_utf16":issue.argument().units().as_ref()})
        );
        Ok(())
    }
}

pub(super) fn load_recipe_book(lifecycle: &PlayerLifecycle) -> Result<ServerRecipeBook> {
    let mut book = ServerRecipeBook::default();
    if let Some(bytes) = &lifecycle.recipe_book_nbt {
        let tag = Tag::decode_anonymous(bytes).context("decode native recipe-book NBT")?;
        ensure!(
            matches!(tag, Tag::Compound(_)),
            "recipe-book NBT is not a compound"
        );
        let mut catalogue = VanillaRecipeCatalogue::get();
        book.load_nbt(&tag, &mut catalogue, &mut NativeRecipeLoadLog)?;
    }
    Ok(book)
}

pub(super) fn save_recipe_book(book: &ServerRecipeBook) -> Result<Vec<u8>> {
    book.to_nbt().encode_network_anonymous()
}
