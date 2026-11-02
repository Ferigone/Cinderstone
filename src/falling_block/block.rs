//! Pinned FallingBlock callbacks for the ordinary ColoredFallingBlock family.
//!
//! Reuse the established world/scheduler/entity capability surface and shared
//! onPlace/updateShape/isFree primitives. The ordinary falling hook is a
//! distinct required capability: it must not acquire Anvil's hurt2/40 behavior.
use crate::{placement::{self, BlockPosition}, workstations::anvil_block};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::sync::OnceLock;

pub use anvil_block::{AnvilBlockAction as FallingBlockAction,
    AnvilBlockHost as FallingBlockHost, AnvilTickOutcome as FallingTickOutcome, ShapeUpdate};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFallingClass { Anvil, Ordinary }

#[derive(Deserialize)]
struct Catalogue { minecraft: String, jar_sha256: String, blocks: Vec<Block> }
#[derive(Deserialize)]
struct Block { name: String, class: String, min: u32, max: u32,
    on_place_class: String, falling_delay: Option<i32> }
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
        Ok(facts)
    })().map_err(|error| format!("{error:#}")))
        .as_ref().map_err(|error| anyhow::anyhow!("{error}"))
}

/// Exact finite source classes implemented by the native scheduled/tick hosts.
/// None is an explicit unported family, not a guessed generic FallingBlock.
pub fn source_class(state: u32) -> Result<Option<NativeFallingClass>> {
    placement::placement_block_name(state)?;
    let block = catalogue()?.blocks.iter().find(|b| (b.min..=b.max).contains(&state))
        .context("falling callback state lacks its actual block owner")?;
    let class = match block.class.as_str() {
        "dey" => NativeFallingClass::Anvil,
        "dhg" if matches!(block.name.as_str(), "minecraft:sand" | "minecraft:red_sand" | "minecraft:gravel")
            => NativeFallingClass::Ordinary,
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

/// The same actual constructed object returned by fall, including an actual
/// addFreshEntity false result. Production must certify the literal inherited
/// FallingBlock.falling body; an unavailable override is not an empty callback.
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

/// FallingBlock.tick44–50. The only distinction from the Anvil path is the
/// actual virtual falling hook after construction/add, never an Anvil setter.
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
