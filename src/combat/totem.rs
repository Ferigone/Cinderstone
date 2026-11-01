//! Java 1.21 `LivingEntity.checkTotemDeathProtection`, source lines 1290–1319.
//!
//! This runner does not own an actor, inventory, health, effect map or RNG.
//! Every operation below must use the actual actor's owner synchronously. The
//! source consumes the totem before its later callbacks; a later error retains
//! earlier mutations and does not return or recreate the consumed item.

/// The original InteractionHand enum encounter order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotemHand {
    MainHand,
    OffHand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TotemEffectKind {
    Regeneration,
    Absorption,
    FireResistance,
}

/// Arguments of the actual three-argument MobEffectInstance constructors.
///
/// The effect holder must resolve to the corresponding actual MobEffects
/// holder. This value describes a requested construction, not a replacement
/// active effect instance or an owned map entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TotemEffectSpec {
    pub kind: TotemEffectKind,
    pub duration: i32,
    pub amplifier: i32,
    pub ambient: bool,
    pub visible: bool,
    pub show_icon: bool,
}

impl TotemEffectSpec {
    const fn new(kind: TotemEffectKind, duration: i32, amplifier: i32) -> Self {
        Self {
            kind,
            duration,
            amplifier,
            ambient: false,
            visible: true,
            show_icon: true,
        }
    }
}

/// Mandatory callbacks into the real victim, inventory, effects and level.
///
/// HandStack is an opaque handle to the canonical stack returned by
/// LivingEntity.getItemInHand. CopiedStack is the detached ItemStack.copy made
/// before shrink; it must preserve that stack's actual components and count.
/// No callback may substitute a temporary inventory image or a second health
/// owner. Missing stat, advancement, game-event, attribute/effect or packet
/// owners must be reported as errors, not treated as successful callbacks.
pub trait TotemHost<S> {
    type Error;
    type HandStack;
    type CopiedStack;

    /// Resolve BYPASSES_INVULNERABILITY against the actual damage type holder.
    fn source_bypasses_invulnerability(&mut self, source: &S) -> Result<bool, Self::Error>;
    fn item_in_hand(&mut self, hand: TotemHand) -> Result<Self::HandStack, Self::Error>;
    /// Exact Items.TOTEM_OF_UNDYING identity, not a display name or component.
    fn stack_is_totem(&mut self, stack: &Self::HandStack) -> Result<bool, Self::Error>;
    fn copy_stack(&mut self, stack: &Self::HandStack) -> Result<Self::CopiedStack, Self::Error>;
    fn shrink_stack(&mut self, stack: &Self::HandStack, amount: i32) -> Result<(), Self::Error>;

    fn victim_is_server_player(&mut self) -> Result<bool, Self::Error>;
    /// ServerPlayer.awardStat(Stats.ITEM_USED.get(Items.TOTEM_OF_UNDYING)).
    fn award_totem_used_stat(&mut self) -> Result<(), Self::Error>;
    /// CriteriaTriggers.USED_TOTEM.trigger with the pre-shrink copied stack.
    fn used_totem_criterion(&mut self, stack: &Self::CopiedStack) -> Result<(), Self::Error>;
    /// Actual GameEvent.ITEM_INTERACT_FINISH callback, including its listeners.
    fn item_interact_finish(&mut self) -> Result<(), Self::Error>;

    /// Actual LivingEntity.setHealth, including its max-health clamp/metadata.
    fn set_health(&mut self, health: f32) -> Result<(), Self::Error>;
    /// Actual removeAllEffects, including each onEffectRemoved callback.
    fn remove_all_effects(&mut self) -> Result<bool, Self::Error>;
    /// Construct/add the real instance through LivingEntity.addEffect. Apply
    /// its admission, hidden-instance, attribute and notification callbacks.
    fn add_effect(&mut self, effect: TotemEffectSpec) -> Result<bool, Self::Error>;
    /// Actual Level.broadcastEntityEvent on this actor, in the shared outbox.
    fn broadcast_entity_event(&mut self, event: u8) -> Result<(), Self::Error>;
}

/// The source's real totem protection callback sequence.
///
/// The caller invokes this only at the actual lethal branch of LivingEntity
/// hurt. A true result does not establish a complete death/respawn lifecycle.
pub fn check_totem_death_protection<S, H: TotemHost<S>>(
    source: &S,
    host: &mut H,
) -> Result<bool, H::Error> {
    if host.source_bypasses_invulnerability(source)? {
        return Ok(false);
    }

    let mut copied_totem = None;
    for hand in [TotemHand::MainHand, TotemHand::OffHand] {
        let stack = host.item_in_hand(hand)?;
        if host.stack_is_totem(&stack)? {
            let copy = host.copy_stack(&stack)?;
            host.shrink_stack(&stack, 1)?;
            copied_totem = Some(copy);
            break;
        }
    }
    let Some(copied_totem) = copied_totem else {
        return Ok(false);
    };

    if host.victim_is_server_player()? {
        host.award_totem_used_stat()?;
        host.used_totem_criterion(&copied_totem)?;
        host.item_interact_finish()?;
    }

    host.set_health(1.0)?;
    let _ = host.remove_all_effects()?;
    let _ = host.add_effect(TotemEffectSpec::new(TotemEffectKind::Regeneration, 900, 1))?;
    let _ = host.add_effect(TotemEffectSpec::new(TotemEffectKind::Absorption, 100, 1))?;
    let _ = host.add_effect(TotemEffectSpec::new(
        TotemEffectKind::FireResistance,
        800,
        0,
    ))?;
    host.broadcast_entity_event(35)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Stack {
        totem: bool,
        count: i32,
        component_marker: u32,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Call {
        Bypasses,
        Hand(TotemHand),
        IsTotem(usize),
        Copy(usize),
        Shrink(usize, i32),
        ServerPlayer,
        Stat,
        Criterion(Stack),
        GameEvent,
        Health(u32),
        ClearEffects,
        Effect(TotemEffectSpec),
        EntityEvent(u8),
    }

    struct Host {
        hands: [Stack; 2],
        calls: Vec<Call>,
        bypasses: bool,
        server_player: bool,
        fail_criterion: bool,
    }

    impl Host {
        fn new() -> Self {
            Self {
                hands: [
                    Stack {
                        totem: false,
                        count: 1,
                        component_marker: 10,
                    },
                    Stack {
                        totem: true,
                        count: 2,
                        component_marker: 73,
                    },
                ],
                calls: Vec::new(),
                bypasses: false,
                server_player: true,
                fail_criterion: false,
            }
        }
    }

    impl TotemHost<()> for Host {
        type Error = &'static str;
        type HandStack = usize;
        type CopiedStack = Stack;

        fn source_bypasses_invulnerability(&mut self, _: &()) -> Result<bool, Self::Error> {
            self.calls.push(Call::Bypasses);
            Ok(self.bypasses)
        }
        fn item_in_hand(&mut self, hand: TotemHand) -> Result<usize, Self::Error> {
            self.calls.push(Call::Hand(hand));
            Ok(match hand {
                TotemHand::MainHand => 0,
                TotemHand::OffHand => 1,
            })
        }
        fn stack_is_totem(&mut self, stack: &usize) -> Result<bool, Self::Error> {
            self.calls.push(Call::IsTotem(*stack));
            Ok(self.hands[*stack].totem)
        }
        fn copy_stack(&mut self, stack: &usize) -> Result<Stack, Self::Error> {
            self.calls.push(Call::Copy(*stack));
            Ok(self.hands[*stack].clone())
        }
        fn shrink_stack(&mut self, stack: &usize, amount: i32) -> Result<(), Self::Error> {
            self.calls.push(Call::Shrink(*stack, amount));
            self.hands[*stack].count -= amount;
            Ok(())
        }
        fn victim_is_server_player(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::ServerPlayer);
            Ok(self.server_player)
        }
        fn award_totem_used_stat(&mut self) -> Result<(), Self::Error> {
            self.calls.push(Call::Stat);
            Ok(())
        }
        fn used_totem_criterion(&mut self, stack: &Stack) -> Result<(), Self::Error> {
            self.calls.push(Call::Criterion(stack.clone()));
            if self.fail_criterion {
                Err("criterion owner unavailable")
            } else {
                Ok(())
            }
        }
        fn item_interact_finish(&mut self) -> Result<(), Self::Error> {
            self.calls.push(Call::GameEvent);
            Ok(())
        }
        fn set_health(&mut self, health: f32) -> Result<(), Self::Error> {
            self.calls.push(Call::Health(health.to_bits()));
            Ok(())
        }
        fn remove_all_effects(&mut self) -> Result<bool, Self::Error> {
            self.calls.push(Call::ClearEffects);
            Ok(false)
        }
        fn add_effect(&mut self, effect: TotemEffectSpec) -> Result<bool, Self::Error> {
            self.calls.push(Call::Effect(effect));
            Ok(false)
        }
        fn broadcast_entity_event(&mut self, event: u8) -> Result<(), Self::Error> {
            self.calls.push(Call::EntityEvent(event));
            Ok(())
        }
    }

    #[test]
    fn canonical_offhand_consumption_copy_callback_order_and_failure_boundary() {
        let mut host = Host::new();
        let copy = host.hands[1].clone();
        assert_eq!(check_totem_death_protection(&(), &mut host), Ok(true));
        assert_eq!(host.hands[1].count, 1);
        let prefix = vec![
            Call::Bypasses,
            Call::Hand(TotemHand::MainHand),
            Call::IsTotem(0),
            Call::Hand(TotemHand::OffHand),
            Call::IsTotem(1),
            Call::Copy(1),
            Call::Shrink(1, 1),
            Call::ServerPlayer,
            Call::Stat,
            Call::Criterion(copy),
        ];
        let mut expected = prefix;
        expected.extend([
            Call::GameEvent,
            Call::Health(1.0f32.to_bits()),
            Call::ClearEffects,
            Call::Effect(TotemEffectSpec::new(TotemEffectKind::Regeneration, 900, 1)),
            Call::Effect(TotemEffectSpec::new(TotemEffectKind::Absorption, 100, 1)),
            Call::Effect(TotemEffectSpec::new(
                TotemEffectKind::FireResistance,
                800,
                0,
            )),
            Call::EntityEvent(35),
        ]);
        assert_eq!(host.calls, expected);

        let mut failing = Host::new();
        failing.fail_criterion = true;
        assert_eq!(
            check_totem_death_protection(&(), &mut failing),
            Err("criterion owner unavailable")
        );
        assert_eq!(failing.hands[1].count, 1);
        assert_eq!(failing.calls, expected[..10]);

        let mut bypass = Host::new();
        bypass.bypasses = true;
        assert_eq!(check_totem_death_protection(&(), &mut bypass), Ok(false));
        assert_eq!(bypass.calls, [Call::Bypasses]);
        assert_eq!(bypass.hands[1].count, 2);

        let mut both_hands = Host::new();
        both_hands.server_player = false;
        both_hands.hands[0].totem = true;
        assert_eq!(check_totem_death_protection(&(), &mut both_hands), Ok(true));
        assert_eq!(both_hands.hands[0].count, 0);
        assert_eq!(both_hands.hands[1].count, 2);
        assert!(!both_hands
            .calls
            .iter()
            .any(|c| matches!(c, Call::Stat | Call::Criterion(_) | Call::GameEvent)));
    }
}
