//! Current-version Java player.dat import/export. Unknown original data is
//! retained separately from implemented native runtime state. Older versions,
//! active effects and nondefault attributes require their actual owners.
use crate::{
    entity::Position, entity_state::java_floor, geometry::Vec3,
    inventory::{ItemStack, PlayerInventory}, item_nbt,
    metadata::{MetadataValue, PlayerEntityData}, nbt::{NbtRoot, Tag},
    player_data::{AbilitiesData, ExperienceData, FoodData, PlayerData, PlayerLifecycle},
    player_enchantment_seed::{read_xp_seed_value, PlayerEnchantmentSeed},
    player_stats::{stat_key, PlayerStats},
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use flate2::{read::MultiGzDecoder, write::GzEncoder, Compression};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, fmt, fs::{self, File, OpenOptions}, io::{Read, Write}, path::Path,
    sync::{atomic::{AtomicU64,Ordering}, OnceLock}, sync::Arc};

pub const DATA_VERSION:i32=3953;
const MAX_BYTES:u64=64*1024*1024;
static TEMP_SEQUENCE:AtomicU64=AtomicU64::new(0);

#[derive(Clone,Debug)]
pub enum PlayerImportIssue {
    DataVersion(Option<i32>),
    ActiveEffects,
    UnknownAttribute(String),
    NonDefaultAttribute {id:String, actual:f64, expected:f64},
    AttributeModifiers(String),
}
impl fmt::Display for PlayerImportIssue{
    fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{match self{
        Self::DataVersion(v)=>write!(f,"Java player DataVersion {v:?} requires a DataFixer; only3953 is supported"),
        Self::ActiveEffects=>f.write_str("Java player has active effects without a native effect owner"),
        Self::UnknownAttribute(id)=>write!(f,"Java player attribute {id} has no native Player default"),
        Self::NonDefaultAttribute{id,actual,expected}=>write!(f,"Java player attribute {id}={actual} differs from original Player default {expected}"),
        Self::AttributeModifiers(id)=>write!(f,"Java player attribute {id} has unported persistent modifiers"),
    }}
}
impl std::error::Error for PlayerImportIssue{}

#[derive(Clone,Debug)]
pub struct JavaPlayerSource{root:NbtRoot, original_gzip:Vec<u8>}
impl JavaPlayerSource{
    pub fn from_gzip(bytes:&[u8])->Result<Self>{
        ensure!(bytes.len() as u64<=MAX_BYTES,"compressed Java player exceeds64MiB");
        let mut raw=Vec::new();MultiGzDecoder::new(bytes).take(MAX_BYTES+1).read_to_end(&mut raw)?;
        ensure!(raw.len() as u64<=MAX_BYTES,"decompressed Java player exceeds64MiB");
        let root=NbtRoot::decode(&raw).context("decode original Java player NBT")?;
        ensure!(matches!(root.tag,Tag::Compound(_)),"Java player root must be a compound");
        Ok(Self{root,original_gzip:bytes.to_vec()})
    }
    pub fn read(path:&Path)->Result<Self>{Self::from_gzip(&read_bounded(path)?)}
    pub fn root(&self)->&Tag{&self.root.tag}
    pub fn classify(&self)->Result<Vec<PlayerImportIssue>>{
        let tag=&self.root.tag;let mut issues=Vec::new();
        let version=tag.get("DataVersion").and_then(|v|integer(v).ok());
        if version!=Some(DATA_VERSION){issues.push(PlayerImportIssue::DataVersion(version));}
        if let Some(effects)=tag.get("active_effects"){
            if !list(effects)?.is_empty(){issues.push(PlayerImportIssue::ActiveEffects);}
        }
        if let Some(attributes)=tag.get("attributes"){
            for attr in list(attributes)?{
                let id=resource(text(attr.get("id").context("attribute id missing")?)?)?;
                if attr.get("modifiers").map(list).transpose()?.is_some_and(|v|!v.is_empty()){
                    issues.push(PlayerImportIssue::AttributeModifiers(id.clone()));
                }
                let actual=attr.get("base").map(real).transpose()?.unwrap_or(0.);
                match default_attributes().get(&id){
                    None=>issues.push(PlayerImportIssue::UnknownAttribute(id)),
                    Some(expected) if actual!=*expected=>issues.push(PlayerImportIssue::NonDefaultAttribute{id,actual,expected:*expected}),
                    _=>{}
                }
            }
        }
        Ok(issues)
    }
    pub fn import(&self,uuid:[u8;16],stats_json:Option<&str>)->Result<PlayerData>{
        if let Some(issue)=self.classify()?.into_iter().next(){return Err(issue.into());}
        let tag=&self.root.tag;
        if let Some(Tag::IntArray(saved))=tag.get("UUID"){
            ensure!(saved.len()==4,"Java player UUID must contain four ints");
            let mut bytes=[0;16];for (chunk,value) in bytes.chunks_exact_mut(4).zip(saved){chunk.copy_from_slice(&value.to_be_bytes());}
            ensure!(bytes==uuid,"Java player UUID does not match authenticated profile");
        }
        let pos=vector(tag.get("Pos").context("Java player position missing")?,3)?;
        let rot=vector(tag.get("Rotation").context("Java player rotation missing")?,2)?;
        let velocity=vector(tag.get("Motion").context("Java player motion missing")?,3)?;
        let position=Position{x:pos[0].clamp(-30_000_512.,30_000_512.),y:pos[1].clamp(-20_000_000.,20_000_000.),z:pos[2].clamp(-30_000_512.,30_000_512.),
            yaw:rot[0] as f32,pitch:rot[1] as f32,on_ground:boolean(tag.get("OnGround"),false)?};
        position.validate().context("Java player position outside native supported range")?;
        let gamemode=int_field(tag,"playerGameType",0)?;
        let mut lifecycle=PlayerLifecycle::new(resource(text(tag.get("Dimension").context("Java player dimension missing")?)?)?,gamemode)?;
        let axis=|v:f64|if v.abs()>10.{0.}else{v};
        lifecycle.motion_velocity=Some(Vec3::new(axis(velocity[0]),axis(velocity[1]),axis(velocity[2])));
        lifecycle.food=FoodData{
            level:int_field(tag,"foodLevel",20)?,saturation:float_field(tag,"foodSaturationLevel",5.)?,
            exhaustion:float_field(tag,"foodExhaustionLevel",0.)?,tick_timer:int_field(tag,"foodTickTimer",0)?,
        };
        lifecycle.experience=ExperienceData{level:int_field(tag,"XpLevel",0)?,total:int_field(tag,"XpTotal",0)?,progress:float_field(tag,"XpP",0.)?};
        // Passive projection of the genuine Java saved-data getter. Loading
        // the live player and its one conditional RNG draw belongs to Main.
        lifecycle.enchantment_seed=Some(PlayerEnchantmentSeed::from_saved_value(read_xp_seed_value(tag)?));
        lifecycle.remaining_fire_ticks=int_field(tag,"Fire",-1)? as i16 as i32;
        lifecycle.fall_distance=float_field(tag,"FallDistance",0.)?;
        lifecycle.death_time=int_field(tag,"DeathTime",0)? as i16 as i32;
        lifecycle.sleep_timer=int_field(tag,"SleepTimer",0)? as i16 as i32;
        if let Some(abilities)=tag.get("abilities"){
            compound(abilities)?;
            lifecycle.abilities=AbilitiesData{
                invulnerable:boolean(abilities.get("invulnerable"),false)?,flying:boolean(abilities.get("flying"),false)?,
                may_fly:boolean(abilities.get("mayfly"),false)?,instant_build:boolean(abilities.get("instabuild"),false)?,
                may_build:boolean(abilities.get("mayBuild"),true)?,flying_speed:float_field(abilities,"flySpeed",0.05)?,walking_speed:float_field(abilities,"walkSpeed",0.1)?,
            };
        }
        if let Some(recipe)=tag.get("recipeBook"){compound(recipe)?;lifecycle.recipe_book_nbt=Some(recipe.encode_network_anonymous()?);}
        lifecycle.java_source_nbt=Some(self.original_gzip.clone());
        lifecycle.java_stats_json=stats_json.map(str::to_owned);
        let mut inventory=PlayerInventory::new();
        if let Some(items)=tag.get("Inventory"){
            for item in list(items)?{
                let slot=int_field(item,"Slot",0)? as u8;
                if let Some(native)=native_slot(slot){inventory.set_slot_authoritative(native,item_nbt::decode_item_stack(item)?);}
            }
        }
        let selected=int_field(tag,"SelectedItemSlot",0)?;
        ensure!((0..=8).contains(&selected),"unsupported Java selected inventory index {selected}");
        inventory.set_selected_hotbar(selected as i16)?;
        let mut entity_data=PlayerEntityData::new_player();
        entity_data.set(1,MetadataValue::Int(int_field(tag,"Air",300)? as i16 as i32),false)?;
        entity_data.set(7,MetadataValue::Int(int_field(tag,"TicksFrozen",0)?),false)?;
        entity_data.set(9,MetadataValue::Float(float_field(tag,"Health",20.)?.clamp(0.,20.)),false)?;
        entity_data.set(15,MetadataValue::Float(float_field(tag,"AbsorptionAmount",0.)?),false)?;
        entity_data.set(16,MetadataValue::Int(int_field(tag,"Score",0)?),false)?;
        for (field,name) in [(3,"CustomNameVisible"),(4,"Silent"),(5,"NoGravity")]{entity_data.set(field,MetadataValue::Boolean(boolean(tag.get(name),false)?),false)?;}
        if let Some(name)=tag.get("CustomName"){entity_data.set(2,MetadataValue::OptionalComponent(Some(item_nbt::text_component_to_wire(name)?)),false)?;}
        for (field,name) in [(19,"ShoulderEntityLeft"),(20,"ShoulderEntityRight")]{if let Some(value)=tag.get(name){compound(value)?;entity_data.set(field,MetadataValue::CompoundTag(Arc::new(value.clone())),false)?;}}
        entity_data.clear_dirty();
        let data=PlayerData{uuid,position,gamemode,entity_data,inventory,stats:import_stats(stats_json)?,lifecycle};
        data.validate()?;Ok(data)
    }
    pub fn export(&self,data:&PlayerData)->Result<Vec<u8>>{
        validate_export(data)?;
        // Source addAdditionalSaveData writes an Int even if an old source
        // getter produced the same value from a missing or other numeric tag.
        let seed_matches=data.lifecycle.enchantment_seed.as_ref().map_or(true,|seed|
            matches!(self.root.tag.get("XpSeed"),Some(Tag::Int(value)) if *value==seed.get()));
        if seed_matches{
            if let Ok(original)=self.import(data.uuid,data.lifecycle.java_stats_json.as_deref()){
                if original.to_bytes()?==data.to_bytes()?{return Ok(self.original_gzip.clone());}
            }
        }
        let mut root=self.root.clone();write_player(&mut root.tag,data)?;gzip_root(&root)
    }
    pub fn from_player_data(data:&PlayerData)->Result<Self>{
        if let Some(bytes)=&data.lifecycle.java_source_nbt{Self::from_gzip(bytes)}else{
            validate_export(data)?;let mut root=NbtRoot{name:"".into(),tag:empty()};write_player(&mut root.tag,data)?;
            let original_gzip=gzip_root(&root)?;Ok(Self{root,original_gzip})
        }
    }
}
pub fn import_player_file(path:&Path,uuid:[u8;16],stats_path:Option<&Path>)->Result<PlayerData>{
    let stats=stats_path.map(|path|match read_bounded(path){Ok(bytes)=>Ok(Some(String::from_utf8(bytes)?)),Err(e)if e.downcast_ref::<std::io::Error>().is_some_and(|e|e.kind()==std::io::ErrorKind::NotFound)=>Ok(None),Err(e)=>Err(e)}).transpose()?.flatten();
    JavaPlayerSource::read(path)?.import(uuid,stats.as_deref())
}
pub fn save_player_file(path:&Path,data:&PlayerData,stats_path:Option<&Path>)->Result<()>{
    let source=JavaPlayerSource::from_player_data(data)?;let bytes=source.export(data)?;
    let stats=stats_path.map(|_|export_stats(data)).transpose()?;
    atomic_write(path,&bytes)?;
    if let (Some(path),Some(stats))=(stats_path,stats){atomic_write(path,stats.as_bytes())?;}
    Ok(())
}
fn validate_export(data:&PlayerData)->Result<()>{
    data.validate()?;
    ensure!(data.inventory.carried().is_empty(),"Java player export requires actual menu close: cursor is not empty");
    ensure!(data.inventory.slots()[0..5].iter().all(ItemStack::is_empty),"Java player export requires actual menu close: crafting grid/result is not empty");
    ensure!(data.lifecycle.motion_velocity.is_some(),"Java player export has no actual motion owner");
    Ok(())
}
fn write_player(tag:&mut Tag,data:&PlayerData)->Result<()>{
    let l=&data.lifecycle;
    tag.insert("DataVersion",Tag::Int(DATA_VERSION))?;
    tag.insert("UUID",Tag::IntArray(data.uuid.chunks_exact(4).map(|c|i32::from_be_bytes(c.try_into().unwrap())).collect()))?;
    tag.insert("Pos",list_tag(6,vec![Tag::Double(data.position.x),Tag::Double(data.position.y),Tag::Double(data.position.z)]))?;
    tag.insert("Rotation",list_tag(5,vec![Tag::Float(data.position.yaw),Tag::Float(data.position.pitch)]))?;
    let v=l.motion_velocity.context("missing actual motion")?;
    tag.insert("Motion",list_tag(6,vec![Tag::Double(v.x),Tag::Double(v.y),Tag::Double(v.z)]))?;
    tag.insert("OnGround",Tag::Byte(i8::from(data.position.on_ground)))?;
    tag.insert("Dimension",Tag::String(resource(&l.dimension)?.into()))?;
    tag.insert("playerGameType",Tag::Int(data.gamemode))?;
    tag.insert("SelectedItemSlot",Tag::Int(data.inventory.selected_hotbar() as i32))?;
    tag.insert("Health",Tag::Float(meta_float(data,9)?))?;
    tag.insert("AbsorptionAmount",Tag::Float(meta_float(data,15)?))?;
    tag.insert("Score",Tag::Int(meta_int(data,16)?))?;
    tag.insert("Air",Tag::Short(meta_int(data,1)? as i16))?;
    tag.insert("TicksFrozen",Tag::Int(meta_int(data,7)?))?;
    tag.insert("Fire",Tag::Short(l.remaining_fire_ticks as i16))?;
    tag.insert("FallDistance",Tag::Float(l.fall_distance))?;
    tag.insert("DeathTime",Tag::Short(l.death_time as i16))?;
    tag.insert("SleepTimer",Tag::Short(l.sleep_timer as i16))?;
    for (key,value) in [("foodLevel",l.food.level),("foodTickTimer",l.food.tick_timer),("XpLevel",l.experience.level),("XpTotal",l.experience.total)]{tag.insert(key,Tag::Int(value))?;}
    // A known native field, including zero, supersedes preserved source NBT.
    // Unknown legacy state does not manufacture or overwrite a Java seed.
    if let Some(seed)=&l.enchantment_seed{seed.add_additional_save_data(tag)?;}
    for (key,value) in [("foodSaturationLevel",l.food.saturation),("foodExhaustionLevel",l.food.exhaustion),("XpP",l.experience.progress)]{tag.insert(key,Tag::Float(value))?;}
    let mut abilities=tag.get("abilities").cloned().unwrap_or_else(empty);compound(&abilities)?;
    for (key,value) in [("invulnerable",l.abilities.invulnerable),("flying",l.abilities.flying),("mayfly",l.abilities.may_fly),("instabuild",l.abilities.instant_build),("mayBuild",l.abilities.may_build)]{abilities.insert(key,Tag::Byte(i8::from(value)))?;}
    abilities.insert("flySpeed",Tag::Float(l.abilities.flying_speed))?;abilities.insert("walkSpeed",Tag::Float(l.abilities.walking_speed))?;tag.insert("abilities",abilities)?;
    if let Some(bytes)=&l.recipe_book_nbt{
        let recipe=Tag::decode_anonymous(bytes)?;let mut merged=tag.get("recipeBook").cloned().unwrap_or_else(empty);compound(&merged)?;
        for (key,value) in compound(&recipe)?{merged.insert(key.clone(),value.clone())?;}
        tag.insert("recipeBook",merged)?;
    }
    let mut original=BTreeMap::new();let mut retained=Vec::new();
    if let Some(items)=tag.get("Inventory"){
        for item in list(items)?{let slot=int_field(item,"Slot",0)? as u8;if native_slot(slot).is_some(){original.insert(slot,item.clone());}else{retained.push(item.clone());}}
    }
    for slot in (0..36).chain(100..104).chain([150]){
        let native=native_slot(slot).expect("native inventory mapping");let stack=data.inventory.slot(native).unwrap();
        if stack.is_empty(){continue;}
        let mut item=item_nbt::encode_item_stack(stack,original.get(&slot))?;item.insert("Slot",Tag::Byte(slot as i8))?;retained.push(item);
    }
    tag.insert("Inventory",list_tag(10,retained))?;
    for (field,name) in [(3,"CustomNameVisible"),(4,"Silent"),(5,"NoGravity")]{let Some(MetadataValue::Boolean(v))=data.entity_data.get(field)else{bail!("missing boolean player metadata {field}");};tag.insert(name,Tag::Byte(i8::from(*v)))?;}
    match data.entity_data.get(2){Some(MetadataValue::OptionalComponent(Some(bytes)))=>{tag.insert("CustomName",item_nbt::text_component_from_wire(bytes)?)?;},Some(MetadataValue::OptionalComponent(None))=>remove(tag,"CustomName"),_=>bail!("invalid custom-name metadata")}
    for (field,name) in [(19,"ShoulderEntityLeft"),(20,"ShoulderEntityRight")]{let Some(MetadataValue::CompoundTag(t))=data.entity_data.get(field)else{bail!("missing shoulder metadata");};tag.insert(name,(**t).clone())?;}
    Ok(())
}
fn default_attributes()->&'static BTreeMap<String,f64>{
    static DATA:OnceLock<BTreeMap<String,f64>>=OnceLock::new();DATA.get_or_init(||{
        let facts:Value=serde_json::from_str(include_str!("../fixtures/player-nbt-attributes.json")).expect("actual Player attribute facts");
        facts["attributes"].as_object().unwrap().iter().map(|(k,v)|(k.clone(),v.as_f64().unwrap())).collect()
    })
}
fn import_stats(source:Option<&str>)->Result<PlayerStats>{
    let mut stats=PlayerStats::default();let Some(source)=source else{return Ok(stats);};
    let value:Value=serde_json::from_str(source)?;
    ensure!(value["DataVersion"].as_i64()==Some(i64::from(DATA_VERSION)),"Java statistics requires unsupported DataFixer version");
    if let Some(types)=value.get("stats"){
        for (type_name,values) in types.as_object().context("statistics types must be object")?{
            for (name,value) in values.as_object().context("statistics values must be object")?{
                if let Some(key)=stat_key(type_name,name){let value=i32::try_from(value.as_i64().context("known statistic value must be int32")?)?;stats.set(key,value)?;}
            }
        }
    }
    stats.drain_dirty();Ok(stats)
}
fn export_stats(data:&PlayerData)->Result<String>{
    if let Some(original)=&data.lifecycle.java_stats_json{
        if import_stats(Some(original))?.values()==data.stats.values(){return Ok(original.clone());}
    }
    let mut original=data.lifecycle.java_stats_json.as_ref().map(|s|serde_json::from_str::<Value>(s)).transpose()?.unwrap_or_else(||Value::Object(Map::new()));
    let canonical:Value=serde_json::from_str(&data.stats.to_stat_file_json()?)?;
    let object=original.as_object_mut().context("statistics root must be object")?;
    object.insert("DataVersion".into(),Value::from(DATA_VERSION));
    let types=object.entry("stats").or_insert_with(||Value::Object(Map::new())).as_object_mut().context("statistics types must be object")?;
    for (name,values) in canonical["stats"].as_object().unwrap(){let out=types.entry(name.clone()).or_insert_with(||Value::Object(Map::new())).as_object_mut().context("statistics type must be object")?;for (key,value) in values.as_object().unwrap(){out.insert(key.clone(),value.clone());}}
    Ok(serde_json::to_string(&original)?)
}
fn native_slot(slot:u8)->Option<usize>{match slot{0..=8=>Some(36+usize::from(slot)),9..=35=>Some(usize::from(slot)),100..=103=>Some(108-usize::from(slot)),150=>Some(45),_=>None}}
fn resource(value:&str)->Result<String>{let full=if value.contains(':'){value.to_owned()}else{format!("minecraft:{value}")};let (ns,path)=full.split_once(':').context("resource namespace")?;ensure!(!ns.is_empty()&&!path.is_empty()&&ns.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b"_.-".contains(&b))&&path.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b"/_.-".contains(&b)),"invalid resource identifier");Ok(full)}
fn empty()->Tag{Tag::Compound(Vec::new())}
fn compound(t:&Tag)->Result<&[(crate::nbt::JavaString,Tag)]>{match t{Tag::Compound(v)=>Ok(v),_=>bail!("expected compound")}}
fn list(t:&Tag)->Result<&[Tag]>{match t{Tag::List{values,..}=>Ok(values),_=>bail!("expected list")}}
fn text(t:&Tag)->Result<&str>{match t{Tag::String(s)=>s.as_str().context("invalid Unicode string"),_=>bail!("expected string")}}
fn integer(t:&Tag)->Result<i32>{Ok(match t{Tag::Byte(v)=>i32::from(*v),Tag::Short(v)=>i32::from(*v),Tag::Int(v)=>*v,Tag::Long(v)=>*v as i32,Tag::Float(v)=>java_floor(f64::from(*v)),Tag::Double(v)=>java_floor(*v),_=>bail!("expected numeric tag")})}
fn real(t:&Tag)->Result<f64>{let v=match t{Tag::Byte(v)=>f64::from(*v),Tag::Short(v)=>f64::from(*v),Tag::Int(v)=>f64::from(*v),Tag::Long(v)=>*v as f64,Tag::Float(v)=>f64::from(*v),Tag::Double(v)=>*v,_=>bail!("expected numeric tag")};ensure!(v.is_finite(),"nonfinite player NBT number");Ok(v)}
fn int_field(t:&Tag,key:&str,default:i32)->Result<i32>{t.get(key).map(integer).transpose().map(|v|v.unwrap_or(default))}
fn float_field(t:&Tag,key:&str,default:f32)->Result<f32>{t.get(key).map(real).transpose().map(|v|v.map(|v|v as f32).unwrap_or(default))}
fn boolean(t:Option<&Tag>,default:bool)->Result<bool>{t.map(integer).transpose().map(|v|v.map(|v|v as i8!=0).unwrap_or(default))}
fn vector(t:&Tag,length:usize)->Result<Vec<f64>>{let values=list(t)?;ensure!(values.len()==length,"invalid player vector length");values.iter().map(real).collect()}
fn list_tag(element_type:u8,values:Vec<Tag>)->Tag{Tag::List{element_type,values}}
fn meta_float(data:&PlayerData,id:u8)->Result<f32>{match data.entity_data.get(id){Some(MetadataValue::Float(v))=>Ok(*v),_=>bail!("missing player float metadata{id}")}}
fn meta_int(data:&PlayerData,id:u8)->Result<i32>{match data.entity_data.get(id){Some(MetadataValue::Int(v))=>Ok(*v),_=>bail!("missing player integer metadata{id}")}}
fn remove(t:&mut Tag,key:&str){if let Tag::Compound(v)=t{v.retain(|(k,_)|k!=key);}}
fn gzip_root(root:&NbtRoot)->Result<Vec<u8>>{let bytes=root.encode()?;let mut out=GzEncoder::new(Vec::new(),Compression::default());out.write_all(&bytes)?;Ok(out.finish()?)}
fn read_bounded(path:&Path)->Result<Vec<u8>>{let file=File::open(path)?;let mut bytes=Vec::new();file.take(MAX_BYTES+1).read_to_end(&mut bytes)?;ensure!(bytes.len() as u64<=MAX_BYTES,"Java player/stat source exceeds64MiB");Ok(bytes)}
fn atomic_write(path:&Path,bytes:&[u8])->Result<()>{
    let parent=path.parent().filter(|p|!p.as_os_str().is_empty()).unwrap_or_else(||Path::new("."));fs::create_dir_all(parent)?;
    let name=path.file_name().context("player destination has no filename")?.to_string_lossy();
    let temporary=parent.join(format!(".{name}.{}.{}.tmp",std::process::id(),TEMP_SEQUENCE.fetch_add(1,Ordering::Relaxed)));
    let mut options=OpenOptions::new();options.write(true).create_new(true);
    #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
    let result=(||->Result<()>{let mut file=options.open(&temporary)?;file.write_all(bytes)?;file.sync_all()?;fs::rename(&temporary,path)?;File::open(parent)?.sync_all()?;Ok(())})();
    if result.is_err(){let _=fs::remove_file(&temporary);}result.with_context(||format!("atomically saving {}",path.display()))
}
