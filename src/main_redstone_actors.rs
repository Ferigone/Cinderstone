//! Real native pressure queries over the shared section/index owner.
//! External entries are immutable same-lock query facts, not proxy entities.
use super::*;
use mc121_rust::{entity_state::EntityPhysicalState, falling_block::FallingBlockState,
    runtime_entity_index::{EntityHandle,EntityKind}};

#[derive(Clone,Copy,Debug)]
pub(crate) struct NativeTriggerBody {
    handle:EntityHandle,
    kind:EntityKind,
    uuid:[u8;16],
    bounds:Aabb,
    // Retain the actual Falling field as a fact, never as a trigger filter.
    // BasePressurePlate does not inspect blocksBuilding.
    blocks_building:Option<bool>,
}

pub(crate) fn capture_external_trigger_bodies<'a>(items:&RuntimeEntities,
    experience:impl IntoIterator<Item=&'a EntityPhysicalState>,
    falling:impl IntoIterator<Item=&'a FallingBlockState>)->Result<Vec<NativeTriggerBody>> {
    let mut bodies=Vec::new();
    let mut seen=BTreeSet::new();
    let mut add=|physical:&EntityPhysicalState,kind,blocks_building|->Result<()> {
        if physical.removed { return Ok(()); }
        let handle=items.entity_handle(physical.entity_id)
            .context("actual external pressure-query body has no shared index owner")?;
        ensure!(items.entity_kind(handle)==Some(kind)&&seen.insert(handle.id),
            "external pressure query has a foreign or duplicate actual handle");
        ensure!(!physical.is_spectator,"native nonplayer spectator override is unavailable");
        bodies.push(NativeTriggerBody {handle,kind,uuid:physical.uuid,bounds:physical.bounds,
            blocks_building});
        Ok(())
    };
    for physical in experience { add(physical,EntityKind::ExperienceOrb,None)?; }
    for actor in falling { add(&actor.physical,EntityKind::FallingBlock,Some(actor.blocks_building))?; }
    Ok(bodies)
}

pub(super) struct NativePlateQuery<'a> {
    pub(super) registry:&'a EntityRegistry,
    pub(super) motion:&'a main_player_motion_hooks::MainPlayerMotionRuntime,
    pub(super) external:&'a [NativeTriggerBody],
    pub(super) trigger:Option<(EntityHandle,&'a EntityPhysicalState)>,
}

impl NativePlateQuery<'_> {
    pub(super) fn validate_current_trigger(&self,items:&dyn mc121_rust::runtime_entities::RuntimeEntityQuerySound)->Result<()> {
        let Some((handle,physical))=self.trigger else {return Ok(())};
        ensure!(physical.entity_id==handle.id&&items.entity_handle(handle.id)==Some(handle),
            "actual pressure trigger lost its registered identity/generation");
        let kind=items.entity_kind(handle).context("actual pressure trigger class is absent")?;
        match kind {
            EntityKind::Player=>{
                let player=self.registry.get(handle.id).context("actual pressure Player is absent")?;
                ensure!(player.profile.uuid==physical.uuid,"actual pressure Player UUID changed");
            }
            EntityKind::ExperienceOrb|EntityKind::FallingBlock=>{
                let saved=self.external.iter().find(|body|body.handle==handle)
                    .context("current external pressure trigger lacks its same-lock owner view")?;
                ensure!(saved.kind==kind&&saved.uuid==physical.uuid,
                    "current pressure trigger differs from its actual retained owner");
                ensure!(!physical.is_spectator,"native nonplayer spectator override is unavailable");
            }
            EntityKind::Item=>{
                // ItemCallbacks holds the genuine current Item physical owner
                // outside its store. The actual TickParts loan certifies its
                // handle; do not look up a proxy or the temporarily absent Item.
                ensure!(!physical.is_spectator,"native nonplayer spectator override is unavailable");
            }
        }
        Ok(())
    }

    fn physical_bounds(&self,handle:EntityHandle,kind:EntityKind)->Result<Option<Aabb>> {
        // The runtime resolves the actual current physical lease before this
        // external resolver. Its required identity checks run at admission.
        match kind {
            EntityKind::Player=>{
                let player=self.registry.get(handle.id).context("indexed pressure Player is absent")?;
                // The actual source selector excludes this known spectator;
                // it does not fabricate a missing motion body for that player.
                if player.gamemode==3 { return Ok(None); }
                let physical=self.motion.physical(handle.id)
                    .context("actual indexed pressure Player physical owner is absent")?;
                ensure!(physical.entity_id==handle.id&&physical.uuid==player.profile.uuid,
                    "indexed pressure Player physical identity differs");
                Ok(Some(physical.bounds))
            }
            EntityKind::ExperienceOrb|EntityKind::FallingBlock=>{
                let body=self.external.iter().find(|body|body.handle==handle)
                    .context("indexed external pressure entity lacks an actual owner view")?;
                ensure!(body.kind==kind,"indexed pressure class differs from actual body");
                // This value is preserved, but is not a source plate predicate.
                let _=body.blocks_building;
                Ok(Some(body.bounds))
            }
            EntityKind::Item=>bail!("actual Item bounds must come from the shared Item store"),
        }
    }

    pub(super) fn count(&self,items:&mut dyn mc121_rust::runtime_entities::RuntimeEntityQuerySound,position:P,living_only:bool)->Result<usize> {
        self.validate_current_trigger(items)?;
        let bounds=crate::main_falling_block_redstone::pressure_touch_bounds(position);
        // Player is the only currently indexed native LivingEntity class.
        // The explicit four-kind domain is checked again below; future classes
        // need their actual inheritance and trigger predicates, not a default.
        let class=living_only.then_some(EntityKind::Player);
        let mut inspect=|handle,kind|self.physical_bounds(handle,kind);
        let candidates=items.query_native_entities_with_actor(bounds,class,self.trigger,&mut inspect)?;
        let mut count=0;
        for handle in candidates {
            let kind=items.entity_kind(handle).context("pressure query returned a stale actual handle")?;
            if living_only&&!crate::main_falling_block_redstone::native_plate_is_living(kind) { continue; }
            if kind==EntityKind::Player {
                let player=self.registry.get(handle.id).context("actual pressure Player disappeared")?;
                if player.gamemode==3 { continue; }
            }
            if crate::main_falling_block_redstone::native_plate_trigger_ignores(kind) { continue; }
            count+=1;
        }
        Ok(count)
    }
}
