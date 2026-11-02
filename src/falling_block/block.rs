//! Pinned finite FallingBlock classes and shared inherited callbacks.
//!
//! Reuse the established world/scheduler/entity capability surface and shared
//! onPlace/updateShape/isFree primitives. The ordinary falling hook is a
//! distinct required capability for ColoredFallingBlock and ConcretePowderBlock:
//! neither may acquire Anvil's hurt2/40 behavior. Source recognition alone does
//! not install a production powder owner, fluid/shape host or callback stage.
use crate::{inventory, placement::{self, BlockPosition}, workstations::anvil_block};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::OnceLock};

pub use anvil_block::{AnvilBlockAction as FallingBlockAction,
    AnvilBlockHost as FallingBlockHost, AnvilTickOutcome as FallingTickOutcome, ShapeUpdate};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFallingClass { Anvil, Ordinary, ConcretePowder }

#[derive(Deserialize)]
struct Catalogue { minecraft: String, jar_sha256: String, blocks: Vec<Block> }
#[derive(Deserialize)]
struct Block { name: String, class: String, min: u32, max: u32, default: u32,
    on_place_class: String, placement_class: String, falling_delay: Option<i32>,
    hardened_state: Option<u32> }
fn catalogue() -> Result<&'static Catalogue> {
    static OWNER: OnceLock<std::result::Result<Catalogue, String>> = OnceLock::new();
    OWNER.get_or_init(|| (|| -> Result<Catalogue> {
        let facts: Catalogue = serde_json::from_str(include_str!("../../fixtures/placement-1.21.json"))?;
        ensure!(facts.minecraft == "1.21" && facts.jar_sha256 ==
            "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
            "foreign falling callback catalogue");
        let ordinary = facts.blocks.iter().filter(|b| b.class == "dhg")
            .map(|b| b.name.as_str()).collect::<Vec<_>>();
        ensure!(ordinary.len() == 3 && ["minecraft:sand", "minecraft:red_sand", "minecraft:gravel"]
            .iter().all(|name| ordinary.contains(name)),
            "ordinary falling class domain differs from pinned source");
        // The same existing inventory owner validates the actual original
        // registry/Property capture against this placement input. Bind the
        // captured final dhk.b associations to those SAME canonical states;
        // neither color names nor an offset between state IDs is a binding.
        let canonical = inventory::block_state_catalog();
        let hash = openssl::sha::sha256(include_bytes!("../../fixtures/placement-1.21.json"))
            .iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        ensure!(canonical.original_owner_facts_metadata()["placement_catalog_sha256"].as_str()
            == Some(hash.as_str()), "falling catalogue differs from the actual canonical source binding");
        let powders = facts.blocks.iter().filter(|block| block.class == "dhk")
            .collect::<Vec<_>>();
        ensure!(powders.len() == 16, "pinned ConcretePowderBlock domain differs");
        let mut powder_keys = BTreeSet::new();
        let mut concrete_blocks = BTreeSet::new();
        for powder in powders {
            ensure!(powder_keys.insert(powder.name.as_str())
                && powder.min == powder.max && powder.default == powder.min
                && powder.on_place_class == "diu" && powder.placement_class == "dhk"
                && powder.falling_delay == Some(anvil_block::FALL_DELAY_TICKS),
                "powder singleton/inherited delay/source declaration differs");
            let actual = canonical.state_definition(powder.min)?
                .context("actual powder state is absent from the canonical registry")?;
            ensure!(powder.name.strip_prefix("minecraft:") == Some(actual.name)
                && (actual.minimum_state, actual.maximum_state, actual.default_state)
                    == (powder.min, powder.max, powder.default),
                "captured powder Block differs from its actual canonical owner");
            let hardened = powder.hardened_state
                .context("actual ConcretePowderBlock final concrete association is absent")?;
            let concrete = facts.blocks.iter().find(|block| (block.min..=block.max).contains(&hardened))
                .context("captured concrete default lacks its actual Block owner")?;
            ensure!(concrete.class == "dfy" && concrete.on_place_class == "dtb"
                && concrete.min == concrete.max && concrete.default == hardened
                && concrete.default == concrete.min && concrete.min != powder.min
                && concrete_blocks.insert(concrete.min),
                "captured powder association differs from the pinned concrete singleton domain");
            let actual = canonical.state_definition(hardened)?
                .context("associated concrete state is absent from the canonical registry")?;
            ensure!(concrete.name.strip_prefix("minecraft:") == Some(actual.name)
                && (actual.minimum_state, actual.maximum_state, actual.default_state)
                    == (concrete.min, concrete.max, hardened),
                "captured concrete association differs from its actual canonical owner");
        }
        Ok(facts)
    })().map_err(|error| format!("{error:#}")))
        .as_ref().map_err(|error| anyhow::anyhow!("{error}"))
}

/// Exact finite source classes with certified pinned library algorithms.
/// None is an explicit unported family, not a guessed generic FallingBlock.
/// Some(ConcretePowder) does NOT certify any Main callback/runtime capability:
/// production must require its installed retained owner and the SAME captured
/// association before scheduling, construction, movement or either landing edit.
pub fn source_class(state: u32) -> Result<Option<NativeFallingClass>> {
    placement::placement_block_name(state)?;
    let block = catalogue()?.blocks.iter().find(|b| (b.min..=b.max).contains(&state))
        .context("falling callback state lacks its actual block owner")?;
    let class = match block.class.as_str() {
        "dey" => NativeFallingClass::Anvil,
        "dhg" if matches!(block.name.as_str(), "minecraft:sand" | "minecraft:red_sand" | "minecraft:gravel")
            => NativeFallingClass::Ordinary,
        "dhk" => NativeFallingClass::ConcretePowder,
        _ => return Ok(None),
    };
    ensure!(block.on_place_class == "diu" && block.falling_delay == Some(2),
        "native falling source inheritance/delay differs from the pinned class");
    Ok(Some(class))
}
pub fn source_delay(state: u32) -> Result<Option<i32>> {
    Ok(source_class(state)?.map(|_| anvil_block::FALL_DELAY_TICKS))
}
pub fn ordinary_delay(state: u32) -> Result<Option<i32>> {
    Ok((source_class(state)? == Some(NativeFallingClass::Ordinary))
        .then_some(anvil_block::FALL_DELAY_TICKS))
}

/// The actual inherited delay2 for certified dhk only. Wet dhk.updateShape
/// returns concrete BEFORE reaching the scheduler; this helper does not turn
/// that wet return into a scheduling obligation or install its Main owner.
pub fn concrete_powder_delay(state: u32) -> Result<Option<i32>> {
    Ok((source_class(state)? == Some(NativeFallingClass::ConcretePowder))
        .then_some(anvil_block::FALL_DELAY_TICKS))
}

/// Actual default state of the final concrete Block captured from dhk.b.
/// The result is a source fact for checking an installed Main association;
/// it does not construct/replace that retained Block or authorize an edit.
pub fn concrete_powder_hardened_state(state: u32) -> Result<Option<u32>> {
    if source_class(state)? != Some(NativeFallingClass::ConcretePowder) { return Ok(None); }
    let block = catalogue()?.blocks.iter().find(|block| (block.min..=block.max).contains(&state))
        .context("certified powder state lacks its captured Block")?;
    Ok(Some(block.hardened_state.context("certified powder lost its concrete association")?))
}

/// Only these certified finite classes inherit literal FallingBlock.falling53.
/// Anvil overrides it, and unported subclasses receive no empty-hook claim.
/// An actual returned constructor/admission object remains required by the host.
pub fn inherits_base_falling_hook(state: u32) -> Result<bool> {
    Ok(matches!(source_class(state)?,
        Some(NativeFallingClass::Ordinary | NativeFallingClass::ConcretePowder)))
}

/// The same actual constructed object returned by fall, including an actual
/// addFreshEntity false result. Production must certify the literal inherited
/// FallingBlock.falling body; an unavailable override is not an empty callback.
/// Both certified dhg and dhk inherit that body. Main must still resolve the
/// same returned object and, for dhk, its installed canonical powder owner.
pub trait OrdinaryFallingBlockHost: FallingBlockHost {
    fn inherited_falling(&mut self, block: &Self::Block, state: &Self::BlockState,
        entity: &mut Self::FallingEntity) -> Result<()>;
}
pub fn on_place<H: FallingBlockHost>(host: &mut H, block: &H::Block,
    state: &H::BlockState, position: BlockPosition) -> Result<()> {
    anvil_block::on_place(host, block, state, position)
}
pub fn update_shape<H: FallingBlockHost>(host: &mut H, block: &H::Block,
    update: ShapeUpdate<'_, H::BlockState>) -> Result<H::BlockState> {
    anvil_block::update_shape(host, block, update)
}
pub fn is_free<H: FallingBlockHost>(host: &mut H, state: &H::BlockState) -> Result<bool> {
    anvil_block::is_free(host, state)
}

/// FallingBlock.tick44–50 for an admitted base-hook class. The distinction
/// from Anvil is its actual virtual falling hook AFTER construction/add, never
/// an Anvil setter. Stable/nonfalling branches do not acquire entity entropy.
pub fn scheduled_tick<H: OrdinaryFallingBlockHost>(host: &mut H, block: &H::Block,
    state: &H::BlockState, position: BlockPosition) -> Result<FallingTickOutcome> {
    host.preflight(FallingBlockAction::ScheduledTick, block, state, position)?;
    let below = host.block_state((position.0, position.1.wrapping_sub(1), position.2))?;
    if !is_free(host, &below)? { return Ok(FallingTickOutcome::NotFreeBelow); }
    if position.1 < host.minimum_build_height()? {
        return Ok(FallingTickOutcome::BelowMinimumBuildHeight);
    }
    host.preflight(FallingBlockAction::FallingEntity, block, state, position)?;
    let mut entity = host.fall_block(position, state)?;
    host.inherited_falling(block, state, &mut entity)?;
    Ok(FallingTickOutcome::Falling)
}
