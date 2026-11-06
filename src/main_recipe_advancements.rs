//! Vanilla recipe advancements in Main (WP-E6 #27): award matched criteria and
//! grant their recipe rewards through the player's recipe book.
//!
//! `MainItemGameplay` evaluates the listeners at their trigger points
//! (InventoryMenu slotChanged, onInsideBlock) and queues the matches. Once per
//! server tick, after the connection phase, this owner runs `TICK` for every
//! active player, fires RecipeUnlockedTrigger for recipes other owners added,
//! then awards the queue in order: PlayerAdvancements.award →
//! AdvancementRewards.grant → awardRecipesByKey → ServerRecipeBook.addRecipes,
//! whose RecipeUnlockedTrigger runs nested, before its ADD packet.
//!
//! The ADD packets reach the client at the end of the tick that matched them,
//! after that tick's slot packets; the original sends them from inside
//! broadcastChanges, before the slot's own ContainerSetSlot. Recipe
//! advancements have no display, so no ClientboundUpdateAdvancements follows.
use super::*;
use mc121_rust::{
    advancements::player::{Listener, PlayerAdvancements, RecipeAdvancements},
    player_data::PlayerLifecycle,
    server_recipe_book::{RecipeBookHost, RecipeBookPacket, RecipeDescriptor, RecipeId},
};

/// PlayerAdvancements.load for an admitted player: the saved document, or a
/// new player's empty progress.
pub(super) fn load(lifecycle: &PlayerLifecycle) -> Result<PlayerAdvancements> {
    let catalogue = RecipeAdvancements::vanilla();
    match &lifecycle.advancements_json {
        Some(text) => PlayerAdvancements::load(catalogue, text)
            .map_err(|error| anyhow!("saved advancements cannot be loaded: {error:?}")),
        None => Ok(PlayerAdvancements::new(catalogue)),
    }
}

pub(super) fn save(advancements: &PlayerAdvancements) -> Result<String> {
    advancements
        .save(RecipeAdvancements::vanilla())
        .map_err(|error| anyhow!("advancements cannot be saved: {error:?}"))
}

struct AwardHost<'a> {
    advancements: &'a mut PlayerAdvancements,
    now: i64,
    packets: Vec<Packet>,
}

impl AwardHost<'_> {
    fn award(&mut self, book: &mut ServerRecipeBook, listener: Listener) -> Result<()> {
        let catalogue = RecipeAdvancements::vanilla();
        let Some(recipes) = self.advancements.award(catalogue, listener, self.now)? else {
            return Ok(());
        };
        // awardRecipesByKey: ids the recipe manager does not hold are skipped.
        let recipes: Vec<RecipeDescriptor> = recipes
            .iter()
            .filter_map(|id| RecipeId::parse(id).ok())
            .filter_map(|id| VanillaRecipeCatalogue::get().descriptor(&id))
            .collect();
        book.add_recipes(&recipes, self)?;
        Ok(())
    }
}

impl RecipeBookHost for AwardHost<'_> {
    fn recipe_unlocked(&mut self, book: &mut ServerRecipeBook, recipe: &RecipeDescriptor) -> Result<()> {
        let matched = self.advancements.recipe_unlocked(RecipeAdvancements::vanilla(), recipe.id.as_str());
        for listener in matched {
            self.award(book, listener)?;
        }
        Ok(())
    }
    fn send_packet(&mut self, _book: &mut ServerRecipeBook, packet: RecipeBookPacket) -> Result<()> {
        self.packets.push(packet.encode()?);
        Ok(())
    }
}

impl Server {
    pub(super) fn flush_recipe_advancements(&self) -> Result<()> {
        let mut guard = self.players.write().map_err(|_| anyhow!("player registry lock poisoned"))?;
        self.award_recipe_advancements_locked(&mut guard, None)
    }

    /// One award pass. With `departing`, only that player's queued matches and
    /// recipe-book additions are awarded (no `TICK`), so its removal save holds
    /// them; otherwise every player whose recipe-book INIT is built.
    pub(super) fn award_recipe_advancements_locked(&self, players: &mut LivePlayers, departing: Option<[u8; 16]>) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;
        let catalogue = RecipeAdvancements::vanilla();
        let selected = |gameplay: &main_item_gameplay::MainItemGameplay, uuid: [u8; 16]| match departing {
            Some(player) => uuid == player,
            None => gameplay.recipe_book_ready(uuid),
        };
        let players_in_pass: Vec<(i32, [u8; 16])> = players
            .gameplay
            .active_player_ids()
            .into_iter()
            .filter(|(_, uuid)| selected(&players.gameplay, *uuid))
            .collect();
        // Matches of players outside this pass (INIT not built yet) wait.
        let (mut queue, waiting): (Vec<_>, Vec<_>) = players
            .gameplay
            .take_pending_awards()
            .into_iter()
            .partition(|(uuid, _)| selected(&players.gameplay, *uuid));
        players.gameplay.requeue_pending_awards(waiting);
        let mut unlocked: Vec<([u8; 16], String)> = Vec::new();
        for (_, uuid) in &players_in_pass {
            let Some(advancements) = players.gameplay.recipe_advancements(*uuid) else { continue };
            if departing.is_none() {
                queue.extend(advancements.tick(catalogue).into_iter().map(|listener| (*uuid, listener)));
            }
            if let Some(book) = players.recipe_books.get(uuid) {
                let baseline = players.gameplay.recipe_trigger_baseline_mut(*uuid);
                for id in book.known() {
                    if !baseline.contains(id.as_str()) {
                        unlocked.push((*uuid, id.as_str().to_owned()));
                    }
                }
            }
        }
        if queue.is_empty() && unlocked.is_empty() {
            return Ok(());
        }
        let mut outbox: BTreeMap<[u8; 16], Vec<Packet>> = BTreeMap::new();
        let events = unlocked
            .into_iter()
            .map(|(uuid, recipe)| (uuid, Err(recipe)))
            .chain(queue.into_iter().map(|(uuid, listener)| (uuid, Ok(listener))));
        for (uuid, event) in events {
            let Some(book) = players.recipe_books.get_mut(&uuid) else { continue };
            let Some(advancements) = players.gameplay.recipe_advancements_mut(uuid) else { continue };
            let mut host = AwardHost { advancements, now, packets: Vec::new() };
            match event {
                Ok(listener) => host.award(book, listener)?,
                Err(recipe) => {
                    for listener in host.advancements.recipe_unlocked(catalogue, &recipe) {
                        host.award(book, listener)?;
                    }
                }
            }
            outbox.entry(uuid).or_default().extend(host.packets);
        }
        for (id, uuid) in &players_in_pass {
            if let Some(book) = players.recipe_books.get(uuid) {
                let known = book.known().iter().map(|id| id.as_str().to_owned()).collect();
                *players.gameplay.recipe_trigger_baseline_mut(*uuid) = known;
            }
            if let Some(packets) = outbox.remove(uuid) {
                if !packets.is_empty() {
                    let _ = self.entity_events.send(PlayerEvent::WorldAudience {
                        recipients: Arc::new(vec![*id]),
                        packets: Arc::new(packets),
                    });
                }
            }
        }
        Ok(())
    }
}
