//! `ContextChain` (Brigadier 1.2.9): a parsed context chain flattened into redirect modifiers
//! followed by one executable context, and the execution order over it.
//!
//! Derived from Mojang's Brigadier (MIT, see `licenses/brigadier-MIT.txt`); the algorithm
//! was read from the bytecode of the bundled `ContextChain` and `CommandDispatcher.execute`
//! (direct `javap` inspection) and is compared with the executed original. Brigadier 1.2.x
//! replaced the older recursive context loop with this model.
//!
//! In *forked mode* (any modifier in the chain is a fork) a modifier failure yields no
//! sources and an executable failure yields `0`, every successful executable counts `1`, and
//! nothing is thrown; otherwise the first failure is returned to the caller.

use super::context::CommandContext;
use super::ResultConsumer;
use crate::commands::exception::CommandSyntaxException;

pub struct ContextChain<S> {
    modifiers: Vec<CommandContext<S>>,
    executable: CommandContext<S>,
}

impl<S: Clone> ContextChain<S> {
    /// `tryFlatten`: `None` when the last context in the chain has no command.
    pub fn try_flatten(root: &CommandContext<S>) -> Option<Self> {
        let mut modifiers = Vec::new();
        let mut current = root;
        loop {
            match current.child() {
                None => {
                    current.command()?;
                    return Some(Self {
                        modifiers,
                        executable: current.clone(),
                    });
                }
                Some(child) => {
                    modifiers.push(current.clone());
                    current = child;
                }
            }
        }
    }

    fn notify(consumer: Option<&ResultConsumer<S>>, context: &CommandContext<S>, success: bool, result: i32) {
        if let Some(consumer) = consumer {
            consumer(context, success, result);
        }
    }

    /// `runModifier`: the sources the next stage runs with.
    pub fn run_modifier(
        modifier: &CommandContext<S>,
        source: &S,
        consumer: Option<&ResultConsumer<S>>,
        forked_mode: bool,
    ) -> Result<Vec<S>, CommandSyntaxException> {
        let Some(redirect) = modifier.redirect_modifier() else {
            return Ok(vec![source.clone()]);
        };
        let context = modifier.copy_for(source.clone());
        match redirect(&context) {
            Ok(sources) => Ok(sources),
            Err(exception) => {
                Self::notify(consumer, &context, false, 0);
                if forked_mode {
                    Ok(Vec::new())
                } else {
                    Err(exception)
                }
            }
        }
    }

    /// `runExecutable`: run the final command for one source.
    pub fn run_executable(
        executable: &CommandContext<S>,
        source: &S,
        consumer: Option<&ResultConsumer<S>>,
        forked_mode: bool,
    ) -> Result<i32, CommandSyntaxException> {
        let context = executable.copy_for(source.clone());
        let command = executable.command().expect("a chain's last context is executable");
        match command(&context) {
            Ok(result) => {
                Self::notify(consumer, &context, true, result);
                Ok(if forked_mode { 1 } else { result })
            }
            Err(exception) => {
                Self::notify(consumer, &context, false, 0);
                if forked_mode {
                    Ok(0)
                } else {
                    Err(exception)
                }
            }
        }
    }

    /// `executeAll(source, consumer)`.
    pub fn execute_all(
        &self,
        source: S,
        consumer: Option<&ResultConsumer<S>>,
    ) -> Result<i32, CommandSyntaxException> {
        if self.modifiers.is_empty() {
            return Self::run_executable(&self.executable, &source, consumer, false);
        }
        let mut forked_mode = false;
        let mut sources = vec![source];
        for modifier in &self.modifiers {
            forked_mode |= modifier.is_forked();
            let mut next = Vec::new();
            for current in &sources {
                next.extend(Self::run_modifier(modifier, current, consumer, forked_mode)?);
            }
            if next.is_empty() {
                return Ok(0);
            }
            sources = next;
        }
        let mut result: i32 = 0;
        for current in &sources {
            result = result.wrapping_add(Self::run_executable(&self.executable, current, consumer, forked_mode)?);
        }
        Ok(result)
    }
}
