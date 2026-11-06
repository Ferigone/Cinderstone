//! `AdvancementRequirements`: an AND of ORs over criterion names.

use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdvancementRequirements {
    pub requirements: Vec<Vec<String>>,
}

impl AdvancementRequirements {
    pub fn new(requirements: Vec<Vec<String>>) -> Self {
        Self { requirements }
    }

    /// `allOf(names)`: every criterion is its own requirement.
    pub fn all_of<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            requirements: names
                .into_iter()
                .map(|name| vec![name.to_owned()])
                .collect(),
        }
    }

    /// `anyOf(names)`: one requirement satisfied by any criterion.
    pub fn any_of<'a>(names: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            requirements: vec![names.into_iter().map(str::to_owned).collect()],
        }
    }

    pub fn len(&self) -> usize {
        self.requirements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.requirements.is_empty()
    }

    /// `test(predicate)`: every requirement has a satisfied criterion; an empty list is never met.
    pub fn test(&self, done: impl Fn(&str) -> bool) -> bool {
        !self.requirements.is_empty()
            && self
                .requirements
                .iter()
                .all(|group| group.iter().any(|name| done(name)))
    }

    /// `count(predicate)`: how many requirements have a satisfied criterion.
    pub fn count(&self, done: impl Fn(&str) -> bool) -> usize {
        self.requirements
            .iter()
            .filter(|group| group.iter().any(|name| done(name)))
            .count()
    }

    /// `names()`: every criterion mentioned.
    pub fn names(&self) -> BTreeSet<&str> {
        self.requirements
            .iter()
            .flatten()
            .map(String::as_str)
            .collect()
    }

    /// `validate(criteria)`: the requirements must mention exactly the advancement's criteria.
    pub fn validate(&self, criteria: &BTreeSet<&str>) -> Result<(), String> {
        let mut mentioned: BTreeSet<&str> = BTreeSet::new();
        for group in &self.requirements {
            if group.is_empty() && criteria.is_empty() {
                return Err("Requirement entry cannot be empty".to_owned());
            }
            mentioned.extend(group.iter().map(String::as_str));
        }
        if &mentioned != criteria {
            let list = |names: Vec<&&str>| {
                format!(
                    "[{}]",
                    names.into_iter().copied().collect::<Vec<_>>().join(", ")
                )
            };
            return Err(format!(
                "Advancement completion requirements did not exactly match specified criteria. Missing: {}. Unknown: {}",
                list(criteria.difference(&mentioned).collect()),
                list(mentioned.difference(criteria).collect())
            ));
        }
        Ok(())
    }
}
