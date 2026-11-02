//! Java 1.21 LecternMenu using the captured world's actual book/page owner.
//!
//! This menu has one book slot and one page DataSlot, with no appended player
//! inventory slots. It owns neither a book copy nor a second page counter.
//! Mutating callbacks operate on a host-staged transaction; publish it only
//! after success, preserving the actual block entity, player and world owners.

use crate::{crafting::RecipeBook, inventory::ItemStack};
use anyhow::{anyhow, ensure, Result};

pub const MENU_TYPE_ID: i32 = 17;
pub const BOOK_SLOT: usize = 0;
pub const PAGE_DATA: usize = 0;
pub const SLOT_COUNT: usize = 1;
pub const DATA_COUNT: usize = 1;
pub const BUTTON_PREVIOUS: i32 = 1;
pub const BUTTON_NEXT: i32 = 2;
pub const BUTTON_TAKE_BOOK: i32 = 3;
pub const BUTTON_PAGE_JUMP_START: i32 = 100;

/// A capability/preflight request, not an owned book or result preview.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LecternAction {
    SetPage {
        requested: i32,
        clamped: i32,
        changed: bool,
    },
    TakeBook,
    /// Ordinary Slot.remove, independent of the button's mayBuild guard.
    SplitBook {
        amount: i32,
    },
    EndPagePulse,
}

/// Implement on a staged borrow of the real lectern/world/player transaction.
/// No callback has a permissive default. Preflight must establish all requested
/// owner/callback capabilities before mutation; an unsupported receiver, drop,
/// scheduler or synchronization path must fail there. A later error discards
/// the entire host transaction, not just a menu-local copy.
pub trait LecternHost {
    type BlockState;
    fn book(&self) -> &ItemStack;
    fn page(&self) -> i32;
    fn page_count(&self) -> i32;
    /// Container.stillValidBlockEntity: actual world exists, same block-entity
    /// identity at the captured position, then player reach with +4.0 range.
    fn still_valid_block_entity(&self, additional_range: f64) -> Result<bool>;
    /// Actual Player.mayBuild(), independent of creative or spectator guesses.
    fn player_may_build(&self) -> Result<bool>;
    fn preflight(&mut self, action: LecternAction) -> Result<()>;

    /// The captured block entity's actual stored BlockState.
    fn block_entity_state(&self) -> Result<Self::BlockState>;
    /// Raw canonical ownership move only. The helper below invokes the source
    /// onBookItemRemove callback; do not invoke that callback a second time.
    fn take_book_raw(&mut self) -> Result<ItemStack>;
    /// Raw owner fields only; None preserves the actual stored pageCount.
    fn write_page_data(&mut self, page: i32, page_count: Option<i32>) -> Result<()>;
    fn set_changed(&mut self) -> Result<()>;
    /// Preserve every other property of the actual state. None preserves
    /// HAS_BOOK. Return the actual new state used by the BLOCK_CHANGE event.
    fn set_block_properties(
        &mut self,
        state: &Self::BlockState,
        powered: bool,
        has_book: Option<bool>,
        flags: i32,
    ) -> Result<Self::BlockState>;
    fn block_change_without_source(&mut self, new_state: &Self::BlockState) -> Result<()>;
    fn update_neighbors_below(&mut self, old_state: &Self::BlockState) -> Result<()>;
    fn schedule_block_tick(&mut self, old_state: &Self::BlockState, delay: i32) -> Result<()>;
    fn level_event(&mut self, event: i32, data: i32) -> Result<()>;
    /// Actual Inventory.add mutates this owned stack to its remainder. Preserve
    /// the source Boolean return, including creative inventory semantics.
    fn add_to_player_inventory(&mut self, book: &mut ItemStack) -> Result<bool>;
    /// Actual Player.drop(stack, false), through native Item/RNG/entity owners.
    fn drop_book(&mut self, remainder: ItemStack, throw_randomly: bool) -> Result<()>;
    /// Actual LecternMenu.setData calls broadcastChanges even on a no-op page.
    fn broadcast_changes(&mut self) -> Result<()>;
}

/// Actual lectern book ItemStack split. This only mutates its stored stack;
/// the source removal callback below owns page/block updates, and no implicit
/// container dirty/broadcast callback is added by removeItem.
pub trait LecternContainerHost: LecternHost {
    fn split_book_raw(&mut self, amount: i32) -> Result<ItemStack>;
}

pub fn on_book_item_remove(host: &mut impl LecternHost) -> Result<()> {
    host.write_page_data(0, Some(0))?;
    let old_state = host.block_entity_state()?;
    let new_state = host.set_block_properties(&old_state, false, Some(false), 3)?;
    host.block_change_without_source(&new_state)?;
    host.update_neighbors_below(&old_state)
}

pub fn remove_book_item(host: &mut impl LecternContainerHost, amount: i32) -> Result<ItemStack> {
    host.preflight(LecternAction::SplitBook { amount })?;
    let taken = host.split_book_raw(amount)?;
    if host.book().is_empty() {
        on_book_item_remove(host)?;
    }
    Ok(taken)
}

/// removeItemNoUpdate still invokes onBookItemRemove in the real bookAccess.
pub fn remove_book_no_update(host: &mut impl LecternHost) -> Result<ItemStack> {
    let taken = host.take_book_raw()?;
    on_book_item_remove(host)?;
    Ok(taken)
}

/// Source ItemStack.is checks canonical items, not arbitrary book components.
pub fn has_lectern_book(book: &ItemStack) -> bool {
    !book.is_empty()
        && ["writable_book", "written_book"]
            .iter()
            .any(|name| RecipeBook::vanilla().item_id(name) == Some(book.item_id))
}

/// LecternBlockEntity.getPageCount: written content takes precedence when both
/// components exist. This helper is for the actual owner's book load/set path;
/// menu navigation reads its already-owned pageCount instead of recomputing it.
pub fn book_page_count(book: &ItemStack) -> Result<i32> {
    for id in [34, 33] {
        if let Some(content) = book.component_value(id)? {
            let pages = content
                .get("pages")
                .and_then(serde_json::Value::as_array)
                .ok_or_else(|| anyhow!("invalid lectern book pages component"))?;
            return Ok(i32::try_from(pages.len())?);
        }
    }
    Ok(0)
}

/// Java Mth.clamp(value, 0, pageCount-1) is min(max(value,0),max), including
/// the source's -1 result for a zero-page book; Rust clamp would panic there.
pub fn clamp_page(requested: i32, page_count: i32) -> Result<i32> {
    ensure!(page_count >= 0, "negative actual lectern page count");
    Ok(requested.max(0).min(page_count.wrapping_sub(1)))
}

/// Block-entity comparator calculation uses Java float at every operation.
/// The LecternBlock wrapper must first verify actual HAS_BOOK and actual owner;
/// it returns zero otherwise, rather than calling this helper on an empty BE.
pub fn book_redstone_signal(page: i32, page_count: i32, has_book: bool) -> i32 {
    let fraction = if page_count > 1 {
        page as f32 / (page_count as f32 - 1.0_f32)
    } else {
        1.0_f32
    };
    let value = fraction * 14.0_f32;
    let integer = value as i32;
    let floor = if value < integer as f32 {
        integer.wrapping_sub(1)
    } else {
        integer
    };
    floor.wrapping_add(i32::from(has_book))
}

pub fn still_valid(host: &impl LecternHost) -> Result<bool> {
    Ok(host.still_valid_block_entity(4.0)? && has_lectern_book(host.book()))
}

/// LecternMenu never implements shift-transfer into/out of another inventory.
pub fn quick_move_stack() -> ItemStack {
    ItemStack::empty()
}

/// Source LecternMenu.clickMenuButton. Caller must perform its normal packet
/// listener/menu identity and validity checks before entering the transaction.
/// This helper leaves the book at the actual lectern on close/disconnect; base
/// menu cursor removal remains with the shared menu lifecycle owner.
pub fn click_menu_button(host: &mut impl LecternHost, button: i32) -> Result<bool> {
    if button >= BUTTON_PAGE_JUMP_START {
        set_page(host, button - BUTTON_PAGE_JUMP_START)?;
        return Ok(true);
    }
    match button {
        BUTTON_PREVIOUS => {
            let requested = host.page().wrapping_sub(1);
            set_page(host, requested)?;
            Ok(true)
        }
        BUTTON_NEXT => {
            let requested = host.page().wrapping_add(1);
            set_page(host, requested)?;
            Ok(true)
        }
        BUTTON_TAKE_BOOK => {
            if !host.player_may_build()? {
                return Ok(false);
            }
            host.preflight(LecternAction::TakeBook)?;
            let mut book = remove_book_no_update(host)?;
            // LecternBlockEntity.onBookItemRemove precedes the explicit
            // Container.setChanged and player inventory insertion.
            host.set_changed()?;
            if !host.add_to_player_inventory(&mut book)? {
                host.drop_book(book, false)?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// LecternMenu.setData -> actual ContainerData -> LecternBlockEntity.setPage.
pub fn set_page(host: &mut impl LecternHost, requested: i32) -> Result<()> {
    let clamped = clamp_page(requested, host.page_count())?;
    let changed = clamped != host.page();
    host.preflight(LecternAction::SetPage {
        requested,
        clamped,
        changed,
    })?;
    if changed {
        host.write_page_data(clamped, None)?;
        host.set_changed()?;
        let old_state = host.block_entity_state()?;
        host.set_block_properties(&old_state, true, None, 3)?;
        host.update_neighbors_below(&old_state)?;
        host.schedule_block_tick(&old_state, 2)?;
        host.level_event(1043, 0)?;
    }
    host.broadcast_changes()
}

/// Source scheduled LecternBlock.tick clears POWERED after the two-tick pulse.
/// Called by the actual block scheduler, never a menu-owned timer.
pub fn scheduled_page_pulse_end<H: LecternHost>(
    host: &mut H,
    scheduler_state: &H::BlockState,
) -> Result<()> {
    host.preflight(LecternAction::EndPagePulse)?;
    host.set_block_properties(scheduler_state, false, None, 3)?;
    host.update_neighbors_below(scheduler_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{inventory::Component, protocol::Writer};
    #[derive(Clone)]
    struct State {
        powered: bool,
        has_book: bool,
    }
    struct Host {
        book: ItemStack,
        page: i32,
        count: i32,
        may_build: bool,
        same_owner: bool,
        inventory_full: bool,
        state: State,
        calls: Vec<String>,
        dropped: Vec<ItemStack>,
    }
    impl Host {
        fn new() -> Self {
            let mut book =
                ItemStack::new(RecipeBook::vanilla().item_id("writable_book").unwrap(), 1).unwrap();
            let mut content = Writer::new();
            content.varint(3);
            for text in ["First", "Second", "Third"] {
                content.string(text).bool(false);
            }
            book.components.push(Component {
                type_id: 33,
                data: content.into_inner(),
            });
            Self {
                book,
                page: 0,
                count: 3,
                may_build: true,
                same_owner: true,
                inventory_full: false,
                state: State {
                    powered: false,
                    has_book: true,
                },
                calls: vec![],
                dropped: vec![],
            }
        }
    }
    impl LecternHost for Host {
        type BlockState = State;
        fn book(&self) -> &ItemStack {
            &self.book
        }
        fn page(&self) -> i32 {
            self.page
        }
        fn page_count(&self) -> i32 {
            self.count
        }
        fn still_valid_block_entity(&self, range: f64) -> Result<bool> {
            assert_eq!(range, 4.0);
            Ok(self.same_owner)
        }
        fn player_may_build(&self) -> Result<bool> {
            Ok(self.may_build)
        }
        fn preflight(&mut self, _: LecternAction) -> Result<()> {
            Ok(())
        }
        fn block_entity_state(&self) -> Result<State> {
            Ok(self.state.clone())
        }
        fn take_book_raw(&mut self) -> Result<ItemStack> {
            self.calls.push("remove".into());
            Ok(std::mem::take(&mut self.book))
        }
        fn write_page_data(&mut self, page: i32, count: Option<i32>) -> Result<()> {
            self.calls.push(format!("page:{page}"));
            self.page = page;
            if let Some(count) = count {
                self.count = count;
            }
            Ok(())
        }
        fn set_changed(&mut self) -> Result<()> {
            self.calls.push("changed".into());
            Ok(())
        }
        fn set_block_properties(
            &mut self,
            state: &State,
            powered: bool,
            has_book: Option<bool>,
            flags: i32,
        ) -> Result<State> {
            assert_eq!(flags, 3);
            self.calls.push(format!("powered:{powered}"));
            self.state = State {
                powered,
                has_book: has_book.unwrap_or(state.has_book),
            };
            Ok(self.state.clone())
        }
        fn block_change_without_source(&mut self, _: &State) -> Result<()> {
            self.calls.push("block_change:null".into());
            Ok(())
        }
        fn update_neighbors_below(&mut self, _: &State) -> Result<()> {
            self.calls.push("below".into());
            Ok(())
        }
        fn schedule_block_tick(&mut self, _: &State, delay: i32) -> Result<()> {
            self.calls.push(format!("schedule:{delay}"));
            Ok(())
        }
        fn level_event(&mut self, event: i32, data: i32) -> Result<()> {
            assert_eq!(data, 0);
            self.calls.push(format!("event:{event}"));
            Ok(())
        }
        fn add_to_player_inventory(&mut self, book: &mut ItemStack) -> Result<bool> {
            self.calls.push("inventory".into());
            if self.inventory_full {
                return Ok(false);
            }
            *book = ItemStack::empty();
            Ok(true)
        }
        fn drop_book(&mut self, book: ItemStack, random: bool) -> Result<()> {
            assert!(!random);
            self.calls.push("drop:false".into());
            self.dropped.push(book);
            Ok(())
        }
        fn broadcast_changes(&mut self) -> Result<()> {
            self.calls.push("broadcast".into());
            Ok(())
        }
    }
    #[test]
    fn borrowed_page_navigation_clamps_and_pulses_only_for_changes() -> Result<()> {
        let mut host = Host::new();
        assert_eq!(book_page_count(host.book())?, 3);
        assert!(click_menu_button(&mut host, 2)?);
        assert_eq!(host.page, 1);
        assert_eq!(
            host.calls,
            [
                "page:1",
                "changed",
                "powered:true",
                "below",
                "schedule:2",
                "event:1043",
                "broadcast"
            ]
        );
        host.calls.clear();
        assert!(click_menu_button(&mut host, i32::MAX)?);
        assert_eq!(host.page, 2);
        host.calls.clear();
        assert!(click_menu_button(&mut host, 2)?);
        assert_eq!(host.calls, ["broadcast"]);
        assert_eq!(clamp_page(100, 0)?, -1);
        assert_eq!(book_redstone_signal(1, 3, true), 8);
        assert!(host.state.powered);
        let scheduled_state = host.state.clone();
        scheduled_page_pulse_end(&mut host, &scheduled_state)?;
        assert!(!host.state.powered);
        Ok(())
    }
    #[test]
    fn take_book_requires_permission_and_drops_only_actual_remainder() -> Result<()> {
        let mut host = Host::new();
        host.may_build = false;
        assert!(!click_menu_button(&mut host, 3)?);
        assert!(host.calls.is_empty());
        host.may_build = true;
        host.inventory_full = true;
        assert!(still_valid(&host)?);
        let expected = host.book.clone();
        assert!(click_menu_button(&mut host, 3)?);
        assert_eq!(
            host.calls,
            [
                "remove",
                "page:0",
                "powered:false",
                "block_change:null",
                "below",
                "changed",
                "inventory",
                "drop:false"
            ]
        );
        assert_eq!(host.dropped, [expected]);
        assert!(host.book.is_empty());
        assert_eq!((host.page, host.count), (0, 0));
        assert!(!still_valid(&host)?);
        assert!(quick_move_stack().is_empty());
        Ok(())
    }
}
