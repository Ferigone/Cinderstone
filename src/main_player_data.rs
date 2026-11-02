//! Native player-save helpers used by the main lifecycle integration.
use anyhow::{ensure, Context, Result};
use mc121_rust::{
    nbt::Tag,
    player_enchantment_seed::{read_xp_seed_value, PlayerEnchantmentSeed},
    player_data::{uuid_string, PlayerDataStore, PlayerLifecycle},
    player_nbt::JavaPlayerSource,
    random::RandomSource,
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

/// Data origin determines whether a real saved-data load occurs. Retained
/// snapshots follow restoreFrom, not a second load of their serialized fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EnchantmentSeedAdmission {
    NewPlayer,
    SavedData,
    RestoredPlayer,
}

/// Preserve only available original XpSeed provenance for an old native record.
/// A present canonical field always wins, including a known zero. A native
/// record lacking both field and actual source entry remains explicitly unknown.
fn project_legacy_enchantment_seed(lifecycle: &mut PlayerLifecycle) -> Result<()> {
    if lifecycle.enchantment_seed.is_some() { return Ok(()); }
    let Some(bytes) = &lifecycle.java_source_nbt else { return Ok(()); };
    let source = JavaPlayerSource::from_gzip(bytes)?;
    if source.root().get("XpSeed").is_some() {
        lifecycle.enchantment_seed = Some(PlayerEnchantmentSeed::from_saved_value(
            read_xp_seed_value(source.root())?,
        ));
    }
    Ok(())
}

pub(super) fn admit_enchantment_seed<R: RandomSource + ?Sized>(
    lifecycle: &mut PlayerLifecycle,
    origin: EnchantmentSeedAdmission,
    player_random: &mut R,
) -> Result<()> {
    match origin {
        EnchantmentSeedAdmission::NewPlayer => {
            ensure!(lifecycle.require_enchantment_seed()?.get() == 0,
                "new Player did not retain its actual constructor seed");
        }
        EnchantmentSeedAdmission::SavedData => {
            if let Err(error) = project_legacy_enchantment_seed(lifecycle) {
                // Broken retained provenance does not invent a seed or block
                // unrelated legacy play. Enchanting will require a known owner.
                eprintln!("{}", json!({"event":"native_player_enchantment_seed_unknown",
                    "reason":"unavailable_preserved_java_seed", "detail":format!("{error:#}")}));
            }
            if let Some(seed) = &mut lifecycle.enchantment_seed {
                let source_value = seed.get();
                seed.load_xp_seed(source_value, player_random);
            }
        }
        EnchantmentSeedAdmission::RestoredPlayer => {
            // The snapshot contains the previous player's actual field. Copy
            // it unconditionally, including zero/unknown, without load redraw.
        }
    }
    if lifecycle.enchantment_seed.is_none() {
        eprintln!("{}", json!({"event":"native_player_enchantment_seed_unknown",
            "reason":"legacy_field_without_source_provenance",
            "enchanting_available":false}));
    }
    Ok(())
}
