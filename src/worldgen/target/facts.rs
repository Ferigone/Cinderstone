use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub struct TargetStateFacts {
    pub is_air: bool,
    pub has_fluid: bool,
    pub randomly_ticking_block: bool,
    pub randomly_ticking_fluid: bool,
    pub world_surface_wg_opaque: bool,
    pub ocean_floor_wg_opaque: bool,
}
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub struct TargetDefaultStates {
    pub air: u32,
    pub void_air: u32,
    pub bedrock: u32,
    pub deepslate: u32,
}

/// Facts from the actual state's registry owner. Implementations cannot use
/// guessed block names, opacity, or random-tick defaults for missing states.
pub trait SourceStateFacts: Send + Sync {
    fn state(&self, state: u32) -> Result<TargetStateFacts>;
    fn defaults(&self) -> TargetDefaultStates;
}

/// Exact original-exported built-in state predicates. This does not supply the
/// light/shape callbacks required by already lighted or final-heightmap chunks.
pub struct BuiltinStateFacts;
#[derive(Deserialize)]
struct Fixture {
    minecraft: String,
    protocol: u32,
    inner_sha256: String,
    catalog_path: String,
    catalog_sha256: String,
    state_count: usize,
    defaults: TargetDefaultStates,
    states: Vec<TargetStateFacts>,
}
fn fixture() -> Result<&'static Fixture> {
    static DATA: OnceLock<std::result::Result<Fixture, String>> = OnceLock::new();
    DATA.get_or_init(|| {
        (|| -> Result<Fixture> {
            let f: Fixture = serde_json::from_str(include_str!(
                "../../../fixtures/world-generation-target-state-facts.json"
            ))?;
            ensure!(
                f.minecraft == "1.21"
                    && f.protocol == 767
                    && f.inner_sha256
                        == "8d47de9a76ab47186deba6c9beda6f9345c6f73618fa5a157c57608be0505a01",
                "target state facts use a different original artifact"
            );
            let catalog =
                openssl::sha::sha256(include_bytes!("../../../fixtures/placement-1.21.json"));
            let digest: String = catalog.iter().map(|b| format!("{b:02x}")).collect();
            ensure!(
                f.catalog_path == "fixtures/placement-1.21.json" && f.catalog_sha256 == digest,
                "target state facts differ from the canonical registry catalog"
            );
            ensure!(
                f.state_count == f.states.len(),
                "target state facts count differs"
            );
            for (id, name) in [
                (f.defaults.air, "air"),
                (f.defaults.void_air, "void_air"),
                (f.defaults.bedrock, "bedrock"),
                (f.defaults.deepslate, "deepslate"),
            ] {
                ensure!(
                    crate::inventory::block_state_name(id) == Some(name),
                    "target default state differs from canonical registry"
                );
            }
            Ok(f)
        })()
        .map_err(|e| e.to_string())
    })
    .as_ref()
    .map_err(|e| anyhow::anyhow!("invalid target state facts: {e}"))
}
impl BuiltinStateFacts {
    pub fn new() -> Result<Self> {
        fixture()?;
        Ok(Self)
    }
}
impl SourceStateFacts for BuiltinStateFacts {
    fn state(&self, state: u32) -> Result<TargetStateFacts> {
        fixture()?
            .states
            .get(state as usize)
            .copied()
            .context("unknown canonical target state")
    }
    fn defaults(&self) -> TargetDefaultStates {
        // Construction has already checked this immutable packaged fixture.
        fixture().expect("validated target state facts").defaults
    }
}
