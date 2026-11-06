//! `NbtPathArgument.NbtPath` of 1.21: the path syntax (`a.b[0]`, `a[]`, `a[{k:1}]`, `{k:1}`,
//! `"quoted key"`, `a{k:1}`) and the operations commands and loot functions run on a tag tree
//! (`get`, `countMatching`, `getOrCreate`, `set`, `insert`, `remove`).
//!
//! The original walks object references; this port addresses each matched tag by its steps from
//! the root, which is equivalent because every node descends exactly one level (or, for a root
//! pattern, none), so the matches of one step never contain each other. Behaviour is checked
//! against the original by `tools/loot_oracle.java nbt-path` (`tools/nbt_path_generate.py`).

use crate::nbt::{JavaString, Tag};
use crate::tag_parser::{read_struct_at, SnbtError};

/// A parse or evaluation failure (`CommandSyntaxException`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    /// The path text is invalid (message key, cursor in characters).
    Syntax(String, usize),
    /// `arguments.nbtpath.nothing_found` with the path prefix that matched nothing.
    NothingFound(String),
    /// `arguments.nbtpath.too_deep`.
    TooDeep,
    /// `commands.data.modify.expected_list`.
    ExpectedList,
    /// `commands.data.modify.invalid_index`.
    InvalidIndex(i32),
    /// A Java runtime exception the original throws out of the operation (for example adding a
    /// compound to a list of another element type).
    Runtime(String),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PathError {}

type Result<T> = std::result::Result<T, PathError>;

/// One node of a path.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    CompoundChild(String),
    MatchObject(String, Tag),
    MatchRootObject(Tag),
    MatchElement(Tag),
    AllElements,
    Indexed(i32),
}

/// A parsed path.
#[derive(Debug, Clone, PartialEq)]
pub struct NbtPath {
    original: String,
    nodes: Vec<Node>,
    /// The cursor after each node, relative to the start (for `nothing_found`).
    ends: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    Key(JavaString),
    Index(usize),
}

type Location = Vec<Step>;

fn syntax<T>(key: &str, cursor: usize) -> Result<T> {
    Err(PathError::Syntax(key.to_owned(), cursor))
}

fn from_snbt(error: SnbtError) -> PathError {
    PathError::Syntax(error.message, error.cursor)
}

fn quoted_start(c: char) -> bool {
    c == '"' || c == '\''
}

fn unquoted_name_char(c: char) -> bool {
    !matches!(c, ' ' | '"' | '\'' | '[' | ']' | '.' | '{' | '}')
}

struct Reader {
    chars: Vec<char>,
    cursor: usize,
}

impl Reader {
    fn can_read(&self) -> bool {
        self.cursor < self.chars.len()
    }

    fn peek(&self) -> Result<char> {
        // The original peeks without a bounds check here and fails with
        // `StringIndexOutOfBoundsException`.
        self.chars.get(self.cursor).copied().ok_or_else(|| PathError::Runtime("StringIndexOutOfBoundsException".to_owned()))
    }

    /// Brigadier `StringReader.expect` (no whitespace skipping).
    fn expect(&mut self, c: char) -> Result<()> {
        if !self.can_read() || self.chars[self.cursor] != c {
            return syntax("parsing.expected", self.cursor);
        }
        self.cursor += 1;
        Ok(())
    }

    fn read_string_until(&mut self, terminator: char) -> Result<String> {
        let mut out = String::new();
        let mut escaped = false;
        while self.can_read() {
            let c = self.chars[self.cursor];
            self.cursor += 1;
            if escaped {
                if c == terminator || c == '\\' {
                    out.push(c);
                    escaped = false;
                } else {
                    self.cursor -= 1;
                    return syntax("parsing.quote.escape", self.cursor);
                }
            } else if c == '\\' {
                escaped = true;
            } else if c == terminator {
                return Ok(out);
            } else {
                out.push(c);
            }
        }
        syntax("parsing.quote.expected.end", self.cursor)
    }

    /// Brigadier `StringReader.readString`.
    fn read_string(&mut self) -> Result<String> {
        if !self.can_read() {
            return Ok(String::new());
        }
        let next = self.chars[self.cursor];
        if quoted_start(next) {
            self.cursor += 1;
            return self.read_string_until(next);
        }
        let start = self.cursor;
        while self.can_read() && {
            let c = self.chars[self.cursor];
            c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+')
        } {
            self.cursor += 1;
        }
        Ok(self.chars[start..self.cursor].iter().collect())
    }

    /// Brigadier `StringReader.readInt`.
    fn read_int(&mut self) -> Result<i32> {
        let start = self.cursor;
        while self.can_read() && matches!(self.chars[self.cursor], '0'..='9' | '.' | '-') {
            self.cursor += 1;
        }
        let number: String = self.chars[start..self.cursor].iter().collect();
        if number.is_empty() {
            return syntax("parsing.int.expected", self.cursor);
        }
        number.parse::<i32>().or_else(|_| {
            self.cursor = start;
            syntax("parsing.int.invalid", start)
        })
    }

    fn read_struct(&mut self) -> Result<Tag> {
        let (tag, cursor) = read_struct_at(&self.chars, self.cursor).map_err(from_snbt)?;
        self.cursor = cursor;
        Ok(tag)
    }

    fn read_object_node(&mut self, name: String) -> Result<Node> {
        if self.can_read() && self.chars[self.cursor] == '{' {
            let pattern = self.read_struct()?;
            Ok(Node::MatchObject(name, pattern))
        } else {
            Ok(Node::CompoundChild(name))
        }
    }

    fn parse_node(&mut self, first: bool) -> Result<Node> {
        match self.peek()? {
            '"' | '\'' => {
                let name = self.read_string()?;
                self.read_object_node(name)
            }
            '[' => {
                self.cursor += 1;
                match self.peek()? {
                    '{' => {
                        let pattern = self.read_struct()?;
                        self.expect(']')?;
                        Ok(Node::MatchElement(pattern))
                    }
                    ']' => {
                        self.cursor += 1;
                        Ok(Node::AllElements)
                    }
                    _ => {
                        let index = self.read_int()?;
                        self.expect(']')?;
                        Ok(Node::Indexed(index))
                    }
                }
            }
            '{' => {
                if !first {
                    return syntax("arguments.nbtpath.node.invalid", self.cursor);
                }
                Ok(Node::MatchRootObject(self.read_struct()?))
            }
            _ => {
                let start = self.cursor;
                while self.can_read() && unquoted_name_char(self.chars[self.cursor]) {
                    self.cursor += 1;
                }
                if self.cursor == start {
                    return syntax("arguments.nbtpath.node.invalid", self.cursor);
                }
                let name = self.chars[start..self.cursor].iter().collect();
                self.read_object_node(name)
            }
        }
    }
}

impl NbtPath {
    /// `NbtPathArgument.parse`: reads up to the first space; returns the path and the number of
    /// characters read.
    pub fn parse_prefix(text: &str) -> Result<(Self, usize)> {
        let mut reader = Reader { chars: text.chars().collect(), cursor: 0 };
        let mut nodes = Vec::new();
        let mut ends = Vec::new();
        let mut first = true;
        while reader.can_read() && reader.chars[reader.cursor] != ' ' {
            let node = reader.parse_node(first)?;
            nodes.push(node);
            ends.push(reader.cursor);
            first = false;
            if reader.can_read() {
                let c = reader.chars[reader.cursor];
                if c != ' ' && c != '[' && c != '{' {
                    reader.expect('.')?;
                }
            }
        }
        let original = reader.chars[..reader.cursor].iter().collect();
        Ok((Self { original, nodes, ends }, reader.cursor))
    }

    /// `NbtPath.of` (`NbtPath.CODEC`): the path; text after a space is ignored, as in the original.
    pub fn of(text: &str) -> Result<Self> {
        Self::parse_prefix(text).map(|(path, _)| path)
    }

    pub fn as_str(&self) -> &str {
        &self.original
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    fn not_found(&self, index: usize) -> PathError {
        PathError::NothingFound(self.original.chars().take(self.ends[index]).collect())
    }

    /// `NbtPath.get`: copies of the matched tags.
    pub fn get(&self, root: &Tag) -> Result<Vec<Tag>> {
        let mut locations: Vec<Location> = vec![Vec::new()];
        for (index, node) in self.nodes.iter().enumerate() {
            locations = step_get(node, root, &locations);
            if locations.is_empty() {
                return Err(self.not_found(index));
            }
        }
        Ok(locations.iter().map(|location| value_at(root, location).expect("matched location")).collect())
    }

    /// `NbtPath.countMatching`.
    pub fn count_matching(&self, root: &Tag) -> usize {
        let mut locations: Vec<Location> = vec![Vec::new()];
        for node in &self.nodes {
            locations = step_get(node, root, &locations);
            if locations.is_empty() {
                return 0;
            }
        }
        locations.len()
    }

    fn get_or_create_parents(&self, root: &mut Tag) -> Result<Vec<Location>> {
        let mut locations: Vec<Location> = vec![Vec::new()];
        for index in 0..self.nodes.len().saturating_sub(1) {
            let preferred = preferred_parent(&self.nodes[index + 1]);
            locations = step_get_or_create(&self.nodes[index], root, &locations, &|| preferred.clone())?;
            if locations.is_empty() {
                return Err(self.not_found(index));
            }
        }
        Ok(locations)
    }

    fn last(&self) -> Result<&Node> {
        self.nodes.last().ok_or_else(|| PathError::Runtime("ArrayIndexOutOfBoundsException".to_owned()))
    }

    /// `NbtPath.getOrCreate(root, supplier)`: copies of the matched or created tags.
    pub fn get_or_create(&self, root: &mut Tag, supplier: &dyn Fn() -> Tag) -> Result<Vec<Tag>> {
        let locations = self.get_or_create_matches(root, supplier)?;
        Ok(locations.iter().map(|location| value_at(root, location).expect("matched location")).collect())
    }

    fn get_or_create_matches(&self, root: &mut Tag, supplier: &dyn Fn() -> Tag) -> Result<Vec<Location>> {
        let parents = self.get_or_create_parents(root)?;
        let last = self.last()?;
        step_get_or_create(last, root, &parents, supplier)
    }

    /// Runs `apply` on every tag matched or created by `getOrCreate` (the merge strategies of
    /// `copy_custom_data` mutate them in place).
    pub fn get_or_create_and_modify(&self, root: &mut Tag, supplier: &dyn Fn() -> Tag, apply: &mut dyn FnMut(&mut Tag) -> Result<()>) -> Result<()> {
        for location in self.get_or_create_matches(root, supplier)? {
            if let Some(tag) = tag_at_mut(root, &location) {
                apply(tag)?;
            }
        }
        Ok(())
    }

    /// `NbtPath.set`: the number of tags changed.
    pub fn set(&self, root: &mut Tag, value: &Tag) -> Result<i32> {
        if is_too_deep(value, self.nodes.len()) {
            return Err(PathError::TooDeep);
        }
        let parents = self.get_or_create_parents(root)?;
        let last = self.last()?.clone();
        let mut total: i32 = 0;
        for parent in parents {
            if let Some(tag) = tag_at_mut(root, &parent) {
                total = total.wrapping_add(set_tag(&last, tag, &|| value.clone())?);
            }
        }
        Ok(total)
    }

    /// `NbtPath.insert(index, root, values)`: the number of target lists changed.
    pub fn insert(&self, index: i32, root: &mut Tag, values: &[Tag]) -> Result<i32> {
        for value in values {
            if is_too_deep(value, self.nodes.len()) {
                return Err(PathError::TooDeep);
            }
        }
        let targets = self.get_or_create_matches(root, &|| Tag::List { element_type: 0, values: Vec::new() })?;
        let mut changed = 0;
        for target in targets {
            let Some(tag) = tag_at_mut(root, &target) else {
                // An array element is a fresh numeric tag in the original: not a collection.
                return Err(PathError::ExpectedList);
            };
            if !is_collection(tag) {
                return Err(PathError::ExpectedList);
            }
            let mut position = if index < 0 { collection_len(tag) as i32 + index + 1 } else { index };
            let mut any = false;
            for value in values {
                // `addTag` checks the element type first; only an accepted tag can hit the index.
                if !accepts(tag, value) {
                    continue;
                }
                if position < 0 || position as usize > collection_len(tag) {
                    return Err(PathError::InvalidIndex(position));
                }
                if add_tag(tag, position as usize, value.clone()) {
                    position += 1;
                    any = true;
                }
            }
            changed += i32::from(any);
        }
        Ok(changed)
    }

    /// `NbtPath.remove`: the number of tags removed.
    pub fn remove(&self, root: &mut Tag) -> Result<i32> {
        let mut locations: Vec<Location> = vec![Vec::new()];
        for node in &self.nodes[..self.nodes.len().saturating_sub(1)] {
            locations = step_get(node, root, &locations);
        }
        let last = self.last()?.clone();
        let mut total: i32 = 0;
        for location in locations {
            if let Some(tag) = tag_at_mut(root, &location) {
                total = total.wrapping_add(remove_tag(&last, tag));
            }
        }
        Ok(total)
    }
}

/// `NbtPath.isTooDeep`.
pub fn is_too_deep(tag: &Tag, depth: usize) -> bool {
    if depth >= 512 {
        return true;
    }
    match tag {
        Tag::Compound(entries) => entries.iter().any(|(_, value)| is_too_deep(value, depth + 1)),
        Tag::List { values, .. } => values.iter().any(|value| is_too_deep(value, depth + 1)),
        _ => false,
    }
}

/// `Tag.equals`: compounds as maps, floating values with `==`.
pub fn java_equals(a: &Tag, b: &Tag) -> bool {
    match (a, b) {
        (Tag::Float(x), Tag::Float(y)) => x == y,
        (Tag::Double(x), Tag::Double(y)) => x == y,
        (Tag::List { values: v1, .. }, Tag::List { values: v2, .. }) => {
            v1.len() == v2.len() && v1.iter().zip(v2).all(|(x, y)| java_equals(x, y))
        }
        (Tag::Compound(e1), Tag::Compound(e2)) => {
            e1.len() == e2.len() && e1.iter().all(|(k, v)| e2.iter().any(|(k2, v2)| k == k2 && java_equals(v, v2)))
        }
        _ => a == b,
    }
}

/// `NbtUtils.compareNbt(expected, actual, true)`.
pub fn compare_nbt(expected: &Tag, actual: Option<&Tag>) -> bool {
    let Some(actual) = actual else { return false };
    if std::mem::discriminant(expected) != std::mem::discriminant(actual) {
        return false;
    }
    match (expected, actual) {
        (Tag::Compound(want), Tag::Compound(have)) => {
            have.len() >= want.len() && want.iter().all(|(key, value)| compare_nbt(value, child(actual, key)))
        }
        (Tag::List { values: want, .. }, Tag::List { values: have, .. }) => {
            if want.is_empty() {
                return have.is_empty();
            }
            have.len() >= want.len() && want.iter().all(|w| have.iter().any(|h| compare_nbt(w, Some(h))))
        }
        _ => java_equals(expected, actual),
    }
}

/// `CompoundTag.get`: a decoded compound may repeat a name; the last one is the effective value
/// (as `Tag::get` treats it).
fn child<'a>(tag: &'a Tag, key: &JavaString) -> Option<&'a Tag> {
    match tag {
        Tag::Compound(entries) => entries.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn is_collection(tag: &Tag) -> bool {
    matches!(tag, Tag::List { .. } | Tag::ByteArray(_) | Tag::IntArray(_) | Tag::LongArray(_))
}

fn collection_len(tag: &Tag) -> usize {
    match tag {
        Tag::List { values, .. } => values.len(),
        Tag::ByteArray(v) => v.len(),
        Tag::IntArray(v) => v.len(),
        Tag::LongArray(v) => v.len(),
        _ => 0,
    }
}

/// `CollectionTag.get(index)` (array elements are fresh numeric tags).
fn element(tag: &Tag, index: usize) -> Option<Tag> {
    match tag {
        Tag::List { values, .. } => values.get(index).cloned(),
        Tag::ByteArray(v) => v.get(index).map(|b| Tag::Byte(*b)),
        Tag::IntArray(v) => v.get(index).map(|n| Tag::Int(*n)),
        Tag::LongArray(v) => v.get(index).map(|n| Tag::Long(*n)),
        _ => None,
    }
}

/// The tag at `location`, borrowed while the path stays inside compounds and lists; an array
/// element is materialised as a fresh numeric tag (as `CollectionTag.get` does).
fn tag_at<'a>(root: &'a Tag, location: &[Step]) -> Option<std::borrow::Cow<'a, Tag>> {
    use std::borrow::Cow;
    let mut current: &'a Tag = root;
    for (position, step) in location.iter().enumerate() {
        match step {
            Step::Key(key) => current = child(current, key)?,
            Step::Index(index) => match current {
                Tag::List { values, .. } => current = values.get(*index)?,
                other => {
                    // An array element ends the walk: nothing below a numeric tag.
                    let value = element(other, *index)?;
                    return (position + 1 == location.len()).then_some(Cow::Owned(value));
                }
            },
        }
    }
    Some(Cow::Borrowed(current))
}

fn value_at(root: &Tag, location: &[Step]) -> Option<Tag> {
    tag_at(root, location).map(std::borrow::Cow::into_owned)
}

/// A mutable reference to a container at `location`; `None` for array elements (the original
/// hands out fresh copies, so writes to them are lost).
fn tag_at_mut<'a>(root: &'a mut Tag, location: &[Step]) -> Option<&'a mut Tag> {
    let mut current = root;
    for step in location {
        current = match (step, current) {
            (Step::Key(key), Tag::Compound(entries)) => entries.iter_mut().rev().find(|(k, _)| k == key).map(|(_, v)| v)?,
            (Step::Index(index), Tag::List { values, .. }) => values.get_mut(*index)?,
            _ => return None,
        };
    }
    Some(current)
}

fn with(location: &[Step], step: Step) -> Location {
    let mut next = location.to_vec();
    next.push(step);
    next
}

fn step_get(node: &Node, root: &Tag, locations: &[Location]) -> Vec<Location> {
    let mut out = Vec::new();
    for location in locations {
        let Some(tag) = tag_at(root, location) else { continue };
        let tag: &Tag = &tag;
        match node {
            Node::CompoundChild(name) => {
                let key = JavaString::Utf8(name.clone());
                if child(tag, &key).is_some() {
                    out.push(with(location, Step::Key(key)));
                }
            }
            Node::Indexed(index) => {
                if is_collection(tag) {
                    let size = collection_len(tag) as i64;
                    let at = if *index < 0 { size + i64::from(*index) } else { i64::from(*index) };
                    if 0 <= at && at < size {
                        out.push(with(location, Step::Index(at as usize)));
                    }
                }
            }
            Node::AllElements => {
                for index in 0..collection_len(tag) {
                    out.push(with(location, Step::Index(index)));
                }
            }
            Node::MatchElement(pattern) => {
                if let Tag::List { values, .. } = &tag {
                    for (index, value) in values.iter().enumerate() {
                        if compare_nbt(pattern, Some(value)) {
                            out.push(with(location, Step::Index(index)));
                        }
                    }
                }
            }
            Node::MatchObject(name, pattern) => {
                let key = JavaString::Utf8(name.clone());
                if matches!(tag, Tag::Compound(_)) && compare_nbt(pattern, child(tag, &key)) {
                    out.push(with(location, Step::Key(key)));
                }
            }
            Node::MatchRootObject(pattern) => {
                if matches!(tag, Tag::Compound(_)) && compare_nbt(pattern, Some(tag)) {
                    out.push(location.clone());
                }
            }
        }
    }
    out
}

fn preferred_parent(node: &Node) -> Tag {
    match node {
        Node::AllElements | Node::Indexed(_) | Node::MatchElement(_) => Tag::List { element_type: 0, values: Vec::new() },
        Node::CompoundChild(_) | Node::MatchObject(..) | Node::MatchRootObject(_) => Tag::Compound(Vec::new()),
    }
}

/// Whether `addTag` would take `tag` (element type or numeric checks).
fn accepts(target: &Tag, tag: &Tag) -> bool {
    match target {
        Tag::List { element_type, .. } => {
            let id = tag.tag_id();
            id != 0 && (*element_type == 0 || *element_type == id)
        }
        Tag::ByteArray(_) | Tag::IntArray(_) | Tag::LongArray(_) => numeric(tag).is_some(),
        _ => false,
    }
}

/// `CollectionTag.addTag(index, tag)`.
fn add_tag(target: &mut Tag, index: usize, tag: Tag) -> bool {
    match target {
        Tag::List { element_type, values } => {
            let id = tag.tag_id();
            if id == 0 || (*element_type != 0 && *element_type != id) {
                return false;
            }
            *element_type = id;
            values.insert(index, tag);
            true
        }
        Tag::ByteArray(v) => numeric(&tag).map(|n| v.insert(index, n.as_byte())).is_some(),
        Tag::IntArray(v) => numeric(&tag).map(|n| v.insert(index, n.as_int())).is_some(),
        Tag::LongArray(v) => numeric(&tag).map(|n| v.insert(index, n.as_long())).is_some(),
        _ => false,
    }
}

/// `CollectionTag.setTag(index, tag)`.
fn set_tag_at(target: &mut Tag, index: usize, tag: Tag) -> bool {
    match target {
        Tag::List { element_type, values } => {
            let id = tag.tag_id();
            if id == 0 || (*element_type != 0 && *element_type != id) {
                return false;
            }
            *element_type = id;
            values[index] = tag;
            true
        }
        Tag::ByteArray(v) => numeric(&tag).map(|n| v[index] = n.as_byte()).is_some(),
        Tag::IntArray(v) => numeric(&tag).map(|n| v[index] = n.as_int()).is_some(),
        Tag::LongArray(v) => numeric(&tag).map(|n| v[index] = n.as_long()).is_some(),
        _ => false,
    }
}

fn remove_at(target: &mut Tag, index: usize) {
    match target {
        Tag::List { element_type, values } => {
            values.remove(index);
            if values.is_empty() {
                *element_type = 0;
            }
        }
        Tag::ByteArray(v) => {
            v.remove(index);
        }
        Tag::IntArray(v) => {
            v.remove(index);
        }
        Tag::LongArray(v) => {
            v.remove(index);
        }
        _ => {}
    }
}

fn clear(target: &mut Tag) {
    match target {
        Tag::List { element_type, values } => {
            values.clear();
            *element_type = 0;
        }
        Tag::ByteArray(v) => v.clear(),
        Tag::IntArray(v) => v.clear(),
        Tag::LongArray(v) => v.clear(),
        _ => {}
    }
}

/// `NumericTag` narrowing.
#[derive(Clone, Copy)]
enum Numeric {
    Integral(i64),
    Float(f32),
    Double(f64),
}

impl Numeric {
    fn as_long(self) -> i64 {
        match self {
            Self::Integral(n) => n,
            // `FloatTag.getAsLong` is a plain cast; `DoubleTag.getAsLong` floors first (Java saturates).
            Self::Float(f) => f as i64,
            Self::Double(d) => d.floor() as i64,
        }
    }
    fn as_int(self) -> i32 {
        // `Mth.floor`: `(int) v`, minus one below it (wrapping, as Java does).
        fn floor(value: f64) -> i32 {
            let i = value as i32;
            if value < f64::from(i) {
                i.wrapping_sub(1)
            } else {
                i
            }
        }
        match self {
            Self::Integral(n) => n as i32,
            Self::Float(f) => {
                let i = f as i32;
                if f < i as f32 {
                    i.wrapping_sub(1)
                } else {
                    i
                }
            }
            Self::Double(d) => floor(d),
        }
    }
    fn as_byte(self) -> i8 {
        (self.as_int() & 0xff) as u8 as i8
    }
}

fn numeric(tag: &Tag) -> Option<Numeric> {
    Some(match tag {
        Tag::Byte(n) => Numeric::Integral(i64::from(*n)),
        Tag::Short(n) => Numeric::Integral(i64::from(*n)),
        Tag::Int(n) => Numeric::Integral(i64::from(*n)),
        Tag::Long(n) => Numeric::Integral(*n),
        Tag::Float(f) => Numeric::Float(*f),
        Tag::Double(d) => Numeric::Double(*d),
        _ => return None,
    })
}

fn compound_put(target: &mut Tag, name: &str, value: Tag) -> Option<Tag> {
    let Tag::Compound(entries) = target else { return None };
    let key = JavaString::Utf8(name.to_owned());
    match entries.iter_mut().rev().find(|(k, _)| *k == key) {
        Some(slot) => Some(std::mem::replace(&mut slot.1, value)),
        None => {
            entries.push((key, value));
            None
        }
    }
}

fn step_get_or_create(node: &Node, root: &mut Tag, locations: &[Location], supplier: &dyn Fn() -> Tag) -> Result<Vec<Location>> {
    let mut out = Vec::new();
    for location in locations {
        let Some(tag) = tag_at_mut(root, location) else { continue };
        match node {
            Node::CompoundChild(name) => {
                if matches!(tag, Tag::Compound(_)) {
                    let key = JavaString::Utf8(name.clone());
                    if child(tag, &key).is_none() {
                        compound_put(tag, name, supplier());
                    }
                    out.push(with(location, Step::Key(key)));
                }
            }
            Node::Indexed(_) | Node::MatchRootObject(_) => {
                let snapshot = tag.clone();
                out.extend(step_get(node, &snapshot, &[Vec::new()]).into_iter().map(|rel| [location.clone(), rel].concat()));
            }
            Node::AllElements => {
                if is_collection(tag) {
                    if collection_len(tag) == 0 {
                        if add_tag(tag, 0, supplier()) {
                            out.push(with(location, Step::Index(0)));
                        }
                    } else {
                        for index in 0..collection_len(tag) {
                            out.push(with(location, Step::Index(index)));
                        }
                    }
                }
            }
            Node::MatchElement(pattern) => {
                if let Tag::List { values, .. } = tag {
                    let mut found = false;
                    for (index, value) in values.iter().enumerate() {
                        if compare_nbt(pattern, Some(value)) {
                            out.push(with(location, Step::Index(index)));
                            found = true;
                        }
                    }
                    if !found {
                        let end = collection_len(tag);
                        // `ListTag.add` throws for a list of another element type.
                        if !add_tag(tag, end, pattern.clone()) {
                            return Err(PathError::Runtime("UnsupportedOperationException".to_owned()));
                        }
                        out.push(with(location, Step::Index(end)));
                    }
                }
            }
            Node::MatchObject(name, pattern) => {
                if matches!(tag, Tag::Compound(_)) {
                    let key = JavaString::Utf8(name.clone());
                    match child(tag, &key) {
                        None => {
                            compound_put(tag, name, pattern.clone());
                            out.push(with(location, Step::Key(key)));
                        }
                        Some(existing) if compare_nbt(pattern, Some(existing)) => out.push(with(location, Step::Key(key))),
                        Some(_) => {}
                    }
                }
            }
        }
    }
    Ok(out)
}

fn set_tag(node: &Node, tag: &mut Tag, supplier: &dyn Fn() -> Tag) -> Result<i32> {
    Ok(match node {
        Node::CompoundChild(name) => {
            if !matches!(tag, Tag::Compound(_)) {
                return Ok(0);
            }
            let value = supplier();
            let old = compound_put(tag, name, value.clone());
            i32::from(!old.is_some_and(|old| java_equals(&value, &old)))
        }
        Node::Indexed(index) => {
            if !is_collection(tag) {
                return Ok(0);
            }
            let size = collection_len(tag) as i64;
            let at = if *index < 0 { size + i64::from(*index) } else { i64::from(*index) };
            if !(0 <= at && at < size) {
                return Ok(0);
            }
            let old = element(tag, at as usize).expect("in range");
            let value = supplier();
            i32::from(!java_equals(&value, &old) && set_tag_at(tag, at as usize, value))
        }
        Node::AllElements => {
            if !is_collection(tag) {
                return Ok(0);
            }
            let size = collection_len(tag);
            if size == 0 {
                add_tag(tag, 0, supplier());
                return Ok(1);
            }
            let value = supplier();
            let unchanged = (0..size).filter(|i| java_equals(&value, &element(tag, *i).expect("in range"))).count();
            let changed = (size - unchanged) as i32;
            if changed == 0 {
                return Ok(0);
            }
            clear(tag);
            if !add_tag(tag, 0, value) {
                return Ok(0);
            }
            for index in 1..size {
                add_tag(tag, index, supplier());
            }
            changed
        }
        Node::MatchElement(pattern) => {
            let Tag::List { values, .. } = &*tag else { return Ok(0) };
            if values.is_empty() {
                if !add_tag(tag, 0, supplier()) {
                    return Err(PathError::Runtime("UnsupportedOperationException".to_owned()));
                }
                return Ok(1);
            }
            let mut count = 0;
            for index in 0..collection_len(tag) {
                let current = element(tag, index).expect("in range");
                if compare_nbt(pattern, Some(&current)) {
                    let value = supplier();
                    if !java_equals(&value, &current) && set_tag_at(tag, index, value) {
                        count += 1;
                    }
                }
            }
            count
        }
        Node::MatchObject(name, pattern) => {
            if !matches!(tag, Tag::Compound(_)) {
                return Ok(0);
            }
            let key = JavaString::Utf8(name.clone());
            let current = child(tag, &key).cloned();
            if !compare_nbt(pattern, current.as_ref()) {
                return Ok(0);
            }
            let value = supplier();
            if current.is_some_and(|current| java_equals(&value, &current)) {
                return Ok(0);
            }
            compound_put(tag, name, value);
            1
        }
        Node::MatchRootObject(_) => 0,
    })
}

fn remove_tag(node: &Node, tag: &mut Tag) -> i32 {
    match node {
        Node::CompoundChild(name) => {
            let key = JavaString::Utf8(name.clone());
            match tag {
                Tag::Compound(entries) => {
                    let before = entries.len();
                    entries.retain(|(k, _)| *k != key);
                    i32::from(entries.len() != before)
                }
                _ => 0,
            }
        }
        Node::Indexed(index) => {
            if !is_collection(tag) {
                return 0;
            }
            let size = collection_len(tag) as i64;
            let at = if *index < 0 { size + i64::from(*index) } else { i64::from(*index) };
            if 0 <= at && at < size {
                remove_at(tag, at as usize);
                1
            } else {
                0
            }
        }
        Node::AllElements => {
            let size = collection_len(tag);
            if is_collection(tag) && size > 0 {
                clear(tag);
                size as i32
            } else {
                0
            }
        }
        Node::MatchElement(pattern) => {
            let Tag::List { values, .. } = &*tag else { return 0 };
            let matching: Vec<usize> = values.iter().enumerate().filter(|(_, v)| compare_nbt(pattern, Some(v))).map(|(i, _)| i).collect();
            for index in matching.iter().rev() {
                remove_at(tag, *index);
            }
            matching.len() as i32
        }
        Node::MatchObject(name, pattern) => {
            let key = JavaString::Utf8(name.clone());
            if matches!(tag, Tag::Compound(_)) && compare_nbt(pattern, child(tag, &key)) {
                if let Tag::Compound(entries) = tag {
                    entries.retain(|(k, _)| *k != key);
                }
                1
            } else {
                0
            }
        }
        Node::MatchRootObject(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag_parser::{parse_compound, parse_value_prefix};

    /// Values with float bits compared exactly (the oracle prints exact bytes), compounds as maps.
    fn same(a: &Tag, b: &Tag) -> bool {
        match (a, b) {
            (Tag::Float(x), Tag::Float(y)) => x.to_bits() == y.to_bits(),
            (Tag::Double(x), Tag::Double(y)) => x.to_bits() == y.to_bits(),
            (Tag::List { element_type: t1, values: v1 }, Tag::List { element_type: t2, values: v2 }) => {
                (t1 == t2 || v1.is_empty()) && v1.len() == v2.len() && v1.iter().zip(v2).all(|(x, y)| same(x, y))
            }
            (Tag::Compound(e1), Tag::Compound(e2)) => {
                e1.len() == e2.len() && e1.iter().all(|(k, v)| e2.iter().any(|(k2, v2)| k == k2 && same(v, v2)))
            }
            _ => a == b,
        }
    }

    fn decode(hex: &str) -> Tag {
        let bytes: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
        Tag::decode_anonymous(&bytes).unwrap()
    }

    fn value(text: &str) -> Tag {
        parse_value_prefix(text).unwrap().0
    }

    fn kind(error: &PathError) -> String {
        match error {
            PathError::Runtime(class) => class.clone(),
            _ => "syntax".to_owned(),
        }
    }

    struct Outcome {
        parsed: String,
        count: Option<i64>,
        tags: Option<Vec<Tag>>,
        root: Option<Tag>,
    }

    fn run(row: &serde_json::Value) -> std::result::Result<Outcome, (&'static str, String)> {
        let path = NbtPath::of(row["path"].as_str().unwrap()).map_err(|e| ("parse", kind(&e)))?;
        let mut outcome = Outcome { parsed: path.as_str().to_owned(), count: None, tags: None, root: None };
        let Some(root_text) = row["root"].as_str() else { return Ok(outcome) };
        let mut root = parse_compound(root_text).unwrap();
        let op = |e: PathError| ("op", kind(&e));
        match row["op"].as_str().unwrap() {
            "get" => outcome.tags = Some(path.get(&root).map_err(op)?),
            "count" => outcome.count = Some(path.count_matching(&root) as i64),
            "get_or_create" => outcome.tags = Some(path.get_or_create(&mut root, &|| Tag::Compound(Vec::new())).map_err(op)?),
            "set" => outcome.count = Some(i64::from(path.set(&mut root, &value(row["value"].as_str().unwrap())).map_err(op)?)),
            "remove" => outcome.count = Some(i64::from(path.remove(&mut root).map_err(op)?)),
            "insert" => {
                let values: Vec<Tag> = row["values"].as_array().unwrap().iter().map(|v| value(v.as_str().unwrap())).collect();
                outcome.count = Some(i64::from(path.insert(row["index"].as_i64().unwrap() as i32, &mut root, &values).map_err(op)?));
            }
            other => panic!("{other}"),
        }
        outcome.root = Some(root);
        Ok(outcome)
    }

    /// Review findings: `FloatTag.getAsLong` truncates while `DoubleTag.getAsLong` floors; a
    /// repeated compound name resolves to its last value; and a list pattern needs at least as
    /// many elements as it has (`NbtUtils.compareNbt` in 1.21 keeps that size check).
    #[test]
    fn narrowing_duplicate_names_and_list_pattern_size() {
        let mut root = parse_compound("{a:[L;0l,0l]}").unwrap();
        NbtPath::of("a[0]").unwrap().set(&mut root, &Tag::Float(-1.5)).unwrap();
        NbtPath::of("a[1]").unwrap().set(&mut root, &Tag::Double(-1.5)).unwrap();
        assert_eq!(root, parse_compound("{a:[L;-1l,-2l]}").unwrap());

        let duplicate = Tag::Compound(vec![
            (JavaString::Utf8("x".into()), Tag::Int(1)),
            (JavaString::Utf8("x".into()), Tag::Int(2)),
        ]);
        assert_eq!(NbtPath::of("x").unwrap().get(&duplicate).unwrap(), vec![Tag::Int(2)]);

        let root = parse_compound("{list:[{k:[1]}]}").unwrap();
        assert_eq!(NbtPath::of("list[{k:[1,1]}]").unwrap().count_matching(&root), 0);
        assert_eq!(NbtPath::of("list[{k:[1]}]").unwrap().count_matching(&root), 1);
    }

    /// Reading every element of a large list is linear (each match borrows the tree).
    #[test]
    fn wildcard_reads_do_not_clone_the_tree_per_match() {
        let values = (0..20_000).map(Tag::Int).collect();
        let root = Tag::Compound(vec![(JavaString::Utf8("a".into()), Tag::List { element_type: 3, values })]);
        let found = NbtPath::of("a[]").unwrap().get(&root).unwrap();
        assert_eq!(found.len(), 20_000);
        assert_eq!(found[19_999], Tag::Int(19_999));
    }

    /// Every vector of the fixture (or of `NBT_PATH_VECTORS`, a full local run): the path parses or
    /// fails as the original's does, and each operation returns the same count or tags and leaves
    /// the same root.
    #[test]
    fn nbt_paths_match_the_original() {
        let text = match std::env::var("NBT_PATH_VECTORS") {
            Ok(path) => std::fs::read_to_string(path).unwrap(),
            Err(_) => include_str!("../fixtures/nbt-path-vectors.json").to_owned(),
        };
        let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        let rows = doc["vectors"].as_array().unwrap();
        let mut failures = Vec::new();
        for row in rows {
            let expect = &row["expect"];
            let problem = match (run(row), expect["ok"].as_bool().unwrap()) {
                (Err((stage, error)), false) => {
                    (stage != expect["stage"].as_str().unwrap() || error != expect["error"].as_str().unwrap())
                        .then(|| format!("ours {stage}/{error}, original {}/{}", expect["stage"], expect["error"]))
                }
                (Err((stage, error)), true) => Some(format!("ours {stage}/{error}, original ok")),
                (Ok(_), false) => Some(format!("ours ok, original {}/{}", expect["stage"], expect["error"])),
                (Ok(outcome), true) => {
                    let mut problem = None;
                    if outcome.parsed != expect["parsed"].as_str().unwrap() {
                        problem = Some(format!("parsed {:?} vs {}", outcome.parsed, expect["parsed"]));
                    }
                    if let (Some(count), Some(want)) = (outcome.count, expect.get("count")) {
                        if count != want.as_i64().unwrap() {
                            problem = Some(format!("count {count} vs {want}"));
                        }
                    }
                    if let (Some(tags), Some(want)) = (&outcome.tags, expect.get("tags")) {
                        let want: Vec<Tag> = want.as_array().unwrap().iter().map(|t| decode(t.as_str().unwrap())).collect();
                        if tags.len() != want.len() || !tags.iter().zip(&want).all(|(a, b)| same(a, b)) {
                            problem = Some(format!("tags {tags:?} vs {want:?}"));
                        }
                    }
                    if let (Some(root), Some(want)) = (&outcome.root, expect.get("root")) {
                        let want = decode(want.as_str().unwrap());
                        if !same(root, &want) {
                            problem = Some(format!("root {root:?} vs {want:?}"));
                        }
                    }
                    problem
                }
            };
            if let Some(problem) = problem {
                failures.push(format!(
                    "{} {} {} {} {}: {problem}",
                    row["path"], row["op"], row.get("value").or(row.get("values")).unwrap_or(&serde_json::Value::Null),
                    row.get("index").unwrap_or(&serde_json::Value::Null), row.get("root").unwrap_or(&serde_json::Value::Null)
                ));
            }
        }
        eprintln!("nbt path vectors: {} checked", rows.len());
        assert!(failures.is_empty(), "{} of {} differ:\n{}", failures.len(), rows.len(), failures[..failures.len().min(25)].join("\n"));
    }
}
