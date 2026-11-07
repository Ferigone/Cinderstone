// SPDX-License-Identifier: GPL-2.0-only
//! One retained shared spawn for command mutation, admission, packets and saving.
use super::*;
use mc121_rust::{commands::{coordinates::CoordinateSource, setworldspawn_command::{self,WorldSpawn}},components::text::Component};
impl Server {
    pub(super) fn world_spawn(&self)->Result<WorldSpawn>{Ok(*self.world_spawn.read().map_err(|_|anyhow!("world spawn poisoned"))?)}
    pub(super) fn console_position(&self)->Result<Vec3>{let s=self.world_spawn()?;Ok(Vec3::new(f64::from(s.x),f64::from(s.y),f64::from(s.z)))}
    pub(super) fn spawn_packet(spawn:WorldSpawn)->Packet {
        Packet{id:0x56,payload:encoded(|w|{w.i64(pack_position(spawn.x,spawn.y,spawn.z)).f32(spawn.angle);})}
    }
    pub(super) fn execute_worldspawn_command(&self,line:&str,actor:Option<i32>,permission:u8)->Result<()> {
        let (at,recipients)={
            let p=self.players.read().map_err(|_|anyhow!("players poisoned"))?;
            let (position,rotation)=match actor {
                Some(id)=>{let p=p.registry.get(id).context("spawn command source absent")?.position;
                    (Vec3::new(p.x,p.y,p.z),(p.pitch,p.yaw))},
                None=>(self.console_position()?,(0.,0.)),
            };
            (CoordinateSource{position,anchored_position:position,rotation},p.registry.snapshot().iter().map(|p|p.entity_id).collect::<Vec<_>>())
        };
        match setworldspawn_command::execute(line,permission,at){
            Ok(spawn)=>{
                *self.world_spawn.write().map_err(|_|anyhow!("world spawn poisoned"))?=spawn;
                let _=self.entity_events.send(PlayerEvent::WorldAudience{recipients:Arc::new(recipients),packets:Arc::new(vec![Self::spawn_packet(spawn)])});
                self.command_component_feedback(actor,spawn.feedback(),true,false,Some(1))
            },
            Err(error)=>self.command_component_feedback(actor,Component::from_json(&json!({"text":error.message(),"color":"red"}))?,false,true,None),
        }
    }
}
