//! `Suggestion`, `IntegerSuggestion`, `Suggestions` and `SuggestionsBuilder`
//! (Brigadier 1.2.9, `com.mojang.brigadier.suggestion`).
//!
//! Derived from Mojang's Brigadier (MIT, see `licenses/brigadier-MIT.txt`). The behaviour was
//! read from the bytecode of the bundled classes (`javap`) and is compared with the executed
//! original in `fixtures/commands-brigadier-suggest-vectors.json`.
//!
//! Details that are observable and therefore reproduced:
//!
//! * all offsets are UTF-16 code-unit offsets, like Java's `String`;
//! * `Suggestions.create` de-duplicates and sorts with `compareToIgnoreCase`, but the input
//!   passes through two `java.util.HashSet`s first. Suggestions that compare equal while
//!   being different (for example `Foo` and `foo`) therefore keep the order of the hash
//!   buckets, which [`JavaHashSet`] models (resizing included);
//! * `IntegerSuggestion` does not override `expand`, so an expanded one becomes a plain
//!   `Suggestion`; its `compareToIgnoreCase` is numeric against another integer suggestion;
//! * `SuggestionsBuilder.suggest(String)` ignores text equal to the remaining input.
//!
//! Not modelled: tooltips (`Message`s hash by identity in the original), asynchronous or
//! failing futures (suggestions are computed synchronously) and `HashSet` buckets treeified
//! after nine hash collisions in a table of 64 or more slots.

use super::dispatcher::{CommandContext, StringRange};
use super::exception::CommandSyntaxException;
use super::java_text::{java_compare_to, java_compare_to_ignore_case, java_string_hash, java_to_lower_case_root};
use std::cmp::Ordering;

/// The unchecked exceptions Brigadier's suggestion code can raise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuggestionError {
    /// `IllegalStateException("Can't find node before cursor")`.
    NoNodeBeforeCursor,
    /// `StringIndexOutOfBoundsException` from `String.substring`.
    StringIndexOutOfBounds,
}

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn from_units(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

/// `String.substring(begin, end)` on UTF-16 units.
fn substring(units: &[u16], begin: usize, end: usize) -> Result<&[u16], SuggestionError> {
    if begin > end || end > units.len() {
        return Err(SuggestionError::StringIndexOutOfBounds);
    }
    Ok(&units[begin..end])
}

/// `Suggestion` (and `IntegerSuggestion` when `value` is set).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Suggestion {
    range: StringRange,
    text: String,
    value: Option<i32>,
}

impl Suggestion {
    pub fn new(range: StringRange, text: &str) -> Self {
        Self {
            range,
            text: text.to_string(),
            value: None,
        }
    }

    /// `new IntegerSuggestion(range, value)`: the text is `Integer.toString(value)`.
    pub fn integer(range: StringRange, value: i32) -> Self {
        Self {
            range,
            text: value.to_string(),
            value: Some(value),
        }
    }

    pub fn range(&self) -> StringRange {
        self.range
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// `IntegerSuggestion.getValue()`; `None` for a plain suggestion.
    pub fn integer_value(&self) -> Option<i32> {
        self.value
    }

    /// `apply(input)`: the input with this suggestion replacing its range.
    pub fn apply(&self, input: &str) -> Result<String, SuggestionError> {
        let input = units(input);
        if self.range.start() == 0 && self.range.end() == input.len() {
            return Ok(self.text.clone());
        }
        let mut result = Vec::new();
        if self.range.start() > 0 {
            result.extend_from_slice(substring(&input, 0, self.range.start())?);
        }
        result.extend(self.text.encode_utf16());
        if self.range.end() < input.len() {
            result.extend_from_slice(substring(&input, self.range.end(), input.len())?);
        }
        Ok(from_units(&result))
    }

    /// `expand(command, range)`: widen the replaced range, copying the extra command text.
    /// Like the original, an `IntegerSuggestion` turns into a plain `Suggestion` unless the
    /// range is unchanged.
    pub fn expand(&self, command: &[u16], range: StringRange) -> Result<Self, SuggestionError> {
        if range == self.range {
            return Ok(self.clone());
        }
        let mut result: Vec<u16> = Vec::new();
        if range.start() < self.range.start() {
            result.extend_from_slice(substring(command, range.start(), self.range.start())?);
        }
        result.extend(self.text.encode_utf16());
        if range.end() > self.range.end() {
            result.extend_from_slice(substring(command, self.range.end(), range.end())?);
        }
        Ok(Self {
            range,
            text: from_units(&result),
            value: None,
        })
    }

    /// `compareToIgnoreCase`: the `create` sort order. Numeric between two integer
    /// suggestions, otherwise case-insensitive on the text.
    pub fn compare_to_ignore_case(&self, other: &Self) -> i32 {
        if let Some(value) = self.value {
            // IntegerSuggestion.compareToIgnoreCase delegates to its compareTo.
            return match other.value {
                Some(other_value) => compare_ints(value, other_value),
                None => java_compare_to(&units(&self.text), &units(&other.text)),
            };
        }
        java_compare_to_ignore_case(&units(&self.text), &units(&other.text))
    }

    /// `Objects.hash(range, text, tooltip)` (tooltip `null`), and for an integer suggestion
    /// `Objects.hash(super.hashCode(), value)`.
    pub fn java_hash(&self) -> i32 {
        let range = 31i32
            .wrapping_mul(31i32.wrapping_add(self.range.start() as i32))
            .wrapping_add(self.range.end() as i32);
        let mut hash = 1i32;
        hash = hash.wrapping_mul(31).wrapping_add(range);
        hash = hash.wrapping_mul(31).wrapping_add(java_string_hash(&units(&self.text)));
        hash = hash.wrapping_mul(31); // null tooltip hashes to 0
        match self.value {
            Some(value) => 31i32.wrapping_mul(31i32.wrapping_add(hash)).wrapping_add(value),
            None => hash,
        }
    }

    /// `newKey.equals(existing)`: `IntegerSuggestion.equals` also demands an integer
    /// suggestion with the same value, while `Suggestion.equals` accepts either.
    fn java_equals(&self, existing: &Self) -> bool {
        let same_fields = self.range == existing.range && self.text == existing.text;
        match self.value {
            Some(value) => existing.value == Some(value) && same_fields,
            None => same_fields,
        }
    }
}

fn compare_ints(a: i32, b: i32) -> i32 {
    match a.cmp(&b) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

/// The iteration order of a `java.util.HashSet<Suggestion>` filled by `add` calls.
struct JavaHashSet {
    buckets: Vec<Vec<(i32, Suggestion)>>,
    size: usize,
}

impl JavaHashSet {
    fn new() -> Self {
        Self {
            buckets: Vec::new(),
            size: 0,
        }
    }

    fn spread(hash: i32) -> u32 {
        let hash = hash as u32;
        hash ^ (hash >> 16)
    }

    fn threshold(&self) -> usize {
        self.buckets.len() * 3 / 4
    }

    /// `HashMap.resize`: double the table (16 slots on first use); every slot splits into
    /// two keeping the relative order of its entries.
    fn resize(&mut self) {
        let capacity = if self.buckets.is_empty() { 16 } else { self.buckets.len() * 2 };
        let mut grown: Vec<Vec<(i32, Suggestion)>> = (0..capacity).map(|_| Vec::new()).collect();
        for entry in self.buckets.drain(..).flatten() {
            let index = (Self::spread(entry.0) as usize) & (capacity - 1);
            grown[index].push(entry);
        }
        self.buckets = grown;
    }

    fn add(&mut self, suggestion: Suggestion) {
        if self.buckets.is_empty() {
            self.resize();
        }
        let hash = suggestion.java_hash();
        let index = (Self::spread(hash) as usize) & (self.buckets.len() - 1);
        let bucket = &mut self.buckets[index];
        if bucket
            .iter()
            .any(|(existing_hash, existing)| *existing_hash == hash && suggestion.java_equals(existing))
        {
            return;
        }
        let chain = bucket.len();
        bucket.push((hash, suggestion));
        // treeifyBin: below 64 slots a chain of nine makes the table grow instead.
        if chain >= 8 && self.buckets.len() < 64 {
            self.resize();
        }
        self.size += 1;
        if self.size > self.threshold() {
            self.resize();
        }
    }

    fn into_order(self) -> Vec<Suggestion> {
        self.buckets.into_iter().flatten().map(|(_, suggestion)| suggestion).collect()
    }
}

/// `Suggestions`: the combined replaced range and the sorted, de-duplicated list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Suggestions {
    range: StringRange,
    list: Vec<Suggestion>,
}

impl Suggestions {
    pub fn new(range: StringRange, list: Vec<Suggestion>) -> Self {
        Self { range, list }
    }

    /// `Suggestions.empty()` / the shared `EMPTY`: range `[0, 0]`, no suggestions.
    pub fn empty() -> Self {
        Self {
            range: StringRange::at(0),
            list: Vec::new(),
        }
    }

    pub fn range(&self) -> StringRange {
        self.range
    }

    pub fn list(&self) -> &[Suggestion] {
        &self.list
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// `Suggestions.create(command, suggestions)`; `suggestions` is in the iteration order of
    /// the Java collection (insertion order for a list, bucket order for a `HashSet`).
    pub fn create(command: &str, suggestions: &[Suggestion]) -> Result<Self, SuggestionError> {
        if suggestions.is_empty() {
            return Ok(Self::empty());
        }
        let command = units(command);
        let mut start = i64::from(i32::MAX);
        let mut end = i64::from(i32::MIN);
        for suggestion in suggestions {
            start = start.min(suggestion.range.start() as i64);
            end = end.max(suggestion.range.end() as i64);
        }
        let range = StringRange::between(start as usize, end as usize);
        let mut texts = JavaHashSet::new();
        for suggestion in suggestions {
            texts.add(suggestion.expand(&command, range)?);
        }
        let mut sorted = texts.into_order();
        // List.sort is a stable merge sort; so is slice::sort_by.
        sorted.sort_by(|a, b| a.compare_to_ignore_case(b).cmp(&0));
        Ok(Self { range, list: sorted })
    }

    /// `Suggestions.merge(command, input)`.
    pub fn merge(command: &str, input: &[Suggestions]) -> Result<Self, SuggestionError> {
        match input {
            [] => Ok(Self::empty()),
            [only] => Ok(only.clone()),
            _ => {
                let mut texts = JavaHashSet::new();
                for suggestions in input {
                    for suggestion in &suggestions.list {
                        texts.add(suggestion.clone());
                    }
                }
                Self::create(command, &texts.into_order())
            }
        }
    }
}

/// `SuggestionsBuilder`.
#[derive(Clone, Debug)]
pub struct SuggestionsBuilder {
    input: Vec<u16>,
    input_lower_case: Vec<u16>,
    start: usize,
    remaining: Vec<u16>,
    remaining_lower_case: Vec<u16>,
    result: Vec<Suggestion>,
}

impl SuggestionsBuilder {
    /// `new SuggestionsBuilder(input, start)`; the lower-case copy uses `Locale.ROOT`.
    pub fn new(input: &str, start: usize) -> Result<Self, SuggestionError> {
        Self::with_lower_case(input, &java_to_lower_case_root(input), start)
    }

    /// `new SuggestionsBuilder(input, inputLowerCase, start)`.
    pub fn with_lower_case(input: &str, input_lower_case: &str, start: usize) -> Result<Self, SuggestionError> {
        let input = units(input);
        let input_lower_case = units(input_lower_case);
        let remaining = substring(&input, start, input.len())?.to_vec();
        let remaining_lower_case = substring(&input_lower_case, start, input_lower_case.len())?.to_vec();
        Ok(Self {
            input,
            input_lower_case,
            start,
            remaining,
            remaining_lower_case,
            result: Vec::new(),
        })
    }

    pub fn input(&self) -> String {
        from_units(&self.input)
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn remaining(&self) -> String {
        from_units(&self.remaining)
    }

    pub fn remaining_lower_case(&self) -> String {
        from_units(&self.remaining_lower_case)
    }

    /// `getRemainingLowerCase()` as UTF-16 units, for prefix tests like `String.startsWith`.
    pub fn remaining_lower_case_units(&self) -> &[u16] {
        &self.remaining_lower_case
    }

    fn full_range(&self) -> StringRange {
        StringRange::between(self.start, self.input.len())
    }

    /// `suggest(String)`: text equal to the remaining input is not a suggestion.
    pub fn suggest(&mut self, text: &str) -> &mut Self {
        if text.encode_utf16().eq(self.remaining.iter().copied()) {
            return self;
        }
        let suggestion = Suggestion::new(self.full_range(), text);
        self.result.push(suggestion);
        self
    }

    /// `suggest(int)`.
    pub fn suggest_integer(&mut self, value: i32) -> &mut Self {
        let suggestion = Suggestion::integer(self.full_range(), value);
        self.result.push(suggestion);
        self
    }

    /// `add(other)`: append the other builder's suggestions.
    pub fn add(&mut self, other: &SuggestionsBuilder) -> &mut Self {
        self.result.extend(other.result.iter().cloned());
        self
    }

    /// `createOffset(start)`: a fresh builder over the same input.
    pub fn create_offset(&self, start: usize) -> Result<Self, SuggestionError> {
        Self::with_lower_case(&from_units(&self.input), &from_units(&self.input_lower_case), start)
    }

    /// `restart()`.
    pub fn restart(&self) -> Self {
        self.create_offset(self.start)
            .expect("the start offset was valid for this input when the builder was created")
    }

    /// `build()`: every suggestion lies inside the input, so `create` cannot fail.
    pub fn build(&self) -> Suggestions {
        Suggestions::create(&from_units(&self.input), &self.result)
            .expect("builder suggestions lie inside the input")
    }
}

/// `SuggestionProvider.getSuggestions(context, builder)`, computed synchronously.
pub type SuggestionProvider<S> =
    std::rc::Rc<dyn Fn(&CommandContext<S>, &mut SuggestionsBuilder) -> Result<Suggestions, CommandSyntaxException>>;
