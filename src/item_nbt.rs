//! Persistent ItemStack.CODEC for the fixed Java 1.21 data version (3953).
//! Persistent component values are translated to their individual stream
//! codecs; a Slot byte sequence is never substituted for an NBT compound.
//! Unsupported component codecs fail explicitly, preserving the source file.
use crate::{inventory::{self, Component, ItemStack}, nbt::{JavaString, Tag}, protocol::{Reader, Writer}};
use anyhow::{anyhow, bail, ensure, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{Map, Value};
use std::{collections::BTreeMap, sync::OnceLock};

const MAX_DEPTH: usize = 64;
const MAX_COMPONENTS: usize = 57;

fn catalog() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| serde_json::from_str(include_str!("../fixtures/items-1.21.json")).expect("pinned item catalogue"))
}
fn items() -> &'static BTreeMap<String, i32> {
    static DATA: OnceLock<BTreeMap<String, i32>> = OnceLock::new();
    DATA.get_or_init(|| catalog()["items"].as_object().unwrap().iter().map(|(id, item)|
        (format!("minecraft:{}", item["name"].as_str().unwrap()), id.parse().unwrap())).collect())
}
fn component_ids() -> &'static BTreeMap<String, i32> {
    static DATA: OnceLock<BTreeMap<String, i32>> = OnceLock::new();
    DATA.get_or_init(|| catalog()["component_types"].as_object().unwrap().iter().map(|(id, item)|
        (format!("minecraft:{}", item["name"].as_str().unwrap()), id.parse().unwrap())).collect())
}
fn component_name(id: i32) -> Result<String> {
    Ok(format!("minecraft:{}", catalog()["component_types"][id.to_string()]["name"].as_str().context("unknown component type")?))
}
fn persistent_component(id: i32) -> Result<bool> {
    static FLAGS: OnceLock<[bool;57]> = OnceLock::new();
    let flags=FLAGS.get_or_init(||{
        let facts:Value=serde_json::from_str(include_str!("../fixtures/item-nbt-component-facts.json")).expect("original component persistence flags");
        let values=facts["components"].as_array().expect("component facts array");
        assert_eq!(values.len(),57);
        let mut out=[false;57];
        for value in values{let id=value["id"].as_u64().expect("component id") as usize;out[id]=value["persistent"].as_bool().expect("component persistence");}
        out
    });
    flags.get(id as usize).copied().context("unknown component id")
}
fn key(value: &str) -> Result<String> {
    let full = if value.contains(':') { value.to_owned() } else { format!("minecraft:{value}") };
    let (namespace, path) = full.split_once(':').context("resource location has no namespace")?;
    ensure!(!namespace.is_empty() && !path.is_empty()
        && namespace.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        && path.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"/_.-".contains(&b)), "invalid resource location {value}");
    Ok(full)
}
fn text(t: &Tag) -> Result<&str> {
    match t { Tag::String(s) => s.as_str().context("codec requires a valid Unicode string"), _ => bail!("codec requires a string") }
}
fn compound(t: &Tag) -> Result<&[(JavaString, Tag)]> {
    match t { Tag::Compound(v) => Ok(v), _ => bail!("codec requires a compound") }
}
fn list(t: &Tag) -> Result<&[Tag]> {
    match t { Tag::List { values, .. } => Ok(values), _ => bail!("codec requires a list") }
}
fn number(t: &Tag) -> Result<i32> {
    Ok(match t { Tag::Byte(v)=>i32::from(*v),Tag::Short(v)=>i32::from(*v),Tag::Int(v)=>*v,Tag::Long(v)=>*v as i32,
        Tag::Float(v)=>*v as i32,Tag::Double(v)=>*v as i32,_=>bail!("codec requires a number") })
}
fn flag(t: &Tag) -> Result<bool> { Ok(number(t)? as i8 != 0) }
fn tooltip(t: &Tag) -> Result<bool> { t.get("show_in_tooltip").map(flag).transpose().map(|v|v.unwrap_or(true)) }
fn record(values: Vec<(&str, Tag)>) -> Tag { Tag::Compound(values.into_iter().map(|(k,v)|(k.into(),v)).collect()) }
fn list_tag(element_type: u8, values: Vec<Tag>) -> Tag { Tag::List { element_type, values } }
fn empty() -> Tag { Tag::Compound(Vec::new()) }
fn limited(v: i32, min: i32, max: i32) -> Result<i32> { ensure!((min..=max).contains(&v), "component value {v} outside {min}..={max}"); Ok(v) }

/// Persistent nonempty ItemStack.CODEC. Empty compound input is the original
/// OPTIONAL_CODEC's empty value, used by callers storing optional items.
pub fn decode_item_stack(tag: &Tag) -> Result<ItemStack> { decode_stack(tag, 0) }
fn decode_stack(tag: &Tag, depth: usize) -> Result<ItemStack> {
    ensure!(depth <= MAX_DEPTH, "nested item component depth exceeded");
    if compound(tag)?.is_empty() { return Ok(ItemStack::empty()); }
    let name = key(text(tag.get("id").context("item NBT has no id")?)?)?;
    let id = *items().get(&name).with_context(||format!("unknown item {name}"))?;
    ensure!(id != 0, "persistent nonempty item cannot be air");
    // Original cuq.b: intRange(1,99).fieldOf("count").orElse(1).
    let count = tag.get("count").and_then(|t|number(t).ok()).filter(|n|(1..=99).contains(n)).unwrap_or(1);
    let mut stack = ItemStack::new(id, count)?;
    if let Some(patch) = tag.get("components") {
        let mut values = BTreeMap::new();
        for (name, value) in compound(patch)? {
            let name = name.as_str().context("component key requires valid Unicode")?;
            let (removed, name) = name.strip_prefix('!').map_or((false, name), |s|(true,s));
            let name = key(name)?;
            let type_id = *component_ids().get(&name).with_context(||format!("unknown persistent component {name}"))?;
            ensure!(persistent_component(type_id)?, "transient component {name} has no persistent codec");
            let value = if removed { None } else { Some(to_wire(type_id, value, depth + 1).with_context(||format!("decoding {name}"))?) };
            values.insert(type_id, value);
        }
        ensure!(values.len() <= MAX_COMPONENTS, "too many item components");
        for (type_id, data) in values {
            match data { Some(data)=>stack.components.push(Component{type_id,data}),None=>stack.removed_components.push(type_id) }
        }
    }
    // Reuse the native stream validator, including nested-slot/component bounds.
    stack.encode_canonical()?;
    Ok(stack)
}
pub fn encode_item_stack(stack: &ItemStack, original: Option<&Tag>) -> Result<Tag> { encode_stack(stack, original, 0) }
fn encode_stack(stack: &ItemStack, original: Option<&Tag>, depth: usize) -> Result<Tag> {
    ensure!(depth <= MAX_DEPTH, "nested item component depth exceeded");
    if stack.is_empty() { return Ok(empty()); }
    ensure!((1..=99).contains(&stack.count), "persistent ItemStack count outside1..99");
    let name = inventory::item_name(stack.item_id).context("unknown item id")?;
    stack.encode_canonical()?;
    let mut out = original.filter(|v|matches!(v,Tag::Compound(_))).cloned().unwrap_or_else(empty);
    out.insert("id",Tag::String(format!("minecraft:{name}").into()))?;
    out.insert("count",Tag::Int(stack.count))?;
    if let Some(source) = original {
        if let Ok(old) = decode_stack(source, depth) {
            if stack.same_item_same_components(&old)? {
                // Retain original component payloads, spelling and unknown root
                // fields if only count changed. Java parser may normalize them.
                return Ok(out);
            }
        }
    }
    if let Tag::Compound(fields) = &mut out { fields.retain(|(name,_)|name!="components"); }
    let mut patch = BTreeMap::new();
    for component in &stack.components {
        // kn.CODEC omits transient component types even if native code has one.
        if !persistent_component(component.type_id)? { continue; }
        if stack.removed_components.contains(&component.type_id) { continue; }
        let name = component_name(component.type_id)?;
        patch.insert(name, from_wire(component.type_id,&component.data,depth+1)?);
    }
    for id in &stack.removed_components {
        if !persistent_component(*id)? { continue; }
        patch.insert(format!("!{}",component_name(*id)?),empty());
    }
    if !patch.is_empty() { out.insert("components",Tag::Compound(patch.into_iter().map(|(k,v)|(k.into(),v)).collect()))?; }
    Ok(out)
}

fn enchantment_keys() -> &'static Vec<String> {
    static DATA: OnceLock<Vec<String>> = OnceLock::new();
    DATA.get_or_init(|| {
        let capture: Value = serde_json::from_str(include_str!("../fixtures/configuration.json")).expect("pinned registries");
        for packet in capture["packets"].as_array().unwrap() {
            if packet["name"] != "registry_data" { continue; }
            let bytes = STANDARD.decode(packet["payload"].as_str().unwrap()).expect("registry base64");
            let mut r = Reader::new(&bytes);
            let registry = r.string(32767).expect("registry name");
            if registry != "minecraft:enchantment" { continue; }
            let count = r.varint().expect("registry count");
            let mut out = Vec::new();
            for _ in 0..count {
                out.push(r.string(32767).expect("registry entry"));
                if r.bool().expect("registry presence") {
                    let (_,used) = Tag::decode_anonymous_prefix(r.remaining()).expect("registry data");
                    r.bytes(used).expect("registry payload");
                }
            }
            r.finish().expect("registry trailing bytes");
            return out;
        }
        panic!("captured registry missing enchantments")
    })
}
fn to_wire(id: i32, t: &Tag, depth: usize) -> Result<Vec<u8>> {
    ensure!(depth<=MAX_DEPTH,"nested component depth exceeded");
    let mut w=Writer::new();
    match id {
        0|37|38|39 => { compound(t)?; w.raw(&t.encode_network_anonymous()?); }
        1 => { w.varint(limited(number(t)?,1,99)?); }
        2 => { w.varint(limited(number(t)?,1,i32::MAX)?); }
        3|16 => { w.varint(limited(number(t)?,0,i32::MAX)?); }
        4 => { compound(t)?; w.bool(tooltip(t)?); }
        5|6 => { w.raw(&component_from_json_string(t,depth)?.encode_network_anonymous()?); }
        7 => { let v=list(t)?;ensure!(v.len()<=256,"too many lore lines");w.varint(v.len() as i32);for t in v {w.raw(&component_from_json_string(t,depth)?.encode_network_anonymous()?);} }
        8 => { let value=text(t)?;w.varint(["common","uncommon","rare","epic"].iter().position(|s|*s==value).context("invalid rarity")? as i32); }
        9|23 => {
            let levels=t.get("levels").unwrap_or(t);
            let mut entries=BTreeMap::new();
            for (name,level) in compound(levels)? {
                let key=key(name.as_str().context("enchantment key Unicode")?)?;
                let id=enchantment_keys().iter().position(|s|s==&key).context("unknown enchantment registry key")? as i32;
                entries.insert(id,limited(number(level)?,0,255)?);
            }
            w.varint(entries.len() as i32);for (id,level) in entries {w.varint(id).varint(level);} w.bool(tooltip(t)?);
        }
        13|26 => {w.varint(number(t)?);}
        14|15|21 => {}
        18 => {w.bool(flag(t)?);}
        19 => {w.raw(&empty().encode_network_anonymous()?);}
        24 => {w.i32(number(t.get("rgb").unwrap_or(t))?).bool(tooltip(t)?);}
        25 => {w.i32(number(t)?);}
        29|30 => {let values=list(t)?;ensure!(values.len()<=256,"nested item list too long");w.varint(values.len() as i32);for v in values {let item=decode_stack(v,depth+1)?;ensure!(!item.is_empty(),"nested nonempty item is empty");w.raw(&item.encode_canonical()?);} }
        41 => {w.varint(limited(number(t)?,0,4)?);}
        43 => {let values=list(t)?;let names=values.iter().map(|t|key(text(t)?).map(|s|Tag::String(s.into()))).collect::<Result<_>>()?;w.raw(&list_tag(8,names).encode_network_anonymous()?);}
        48 => {w.string(&key(text(t)?)?);}
        50 => {let key=text(t)?;w.varint(COLORS.iter().position(|v|*v==key).context("unknown dye color")? as i32);}
        52 => {
            let mut slots=BTreeMap::new();
            for entry in list(t)? {let slot=limited(number(entry.get("slot").context("container slot missing")?)?,0,255)?;
                let item=decode_stack(entry.get("item").context("container item missing")?,depth+1)?;ensure!(!item.is_empty(),"container slot requires nonempty ItemStack.CODEC");slots.insert(slot,item);}
            let size=slots.keys().next_back().map_or(0,|id|id+1);w.varint(size);
            for id in 0..size {w.raw(&slots.get(&id).cloned().unwrap_or_default().encode_canonical()?);}
        }
        53 => {let props=compound(t)?;w.varint(props.len() as i32);for (k,v) in props {w.string(k.as_str().context("block property key Unicode")?).string(text(v)?);} }
        _ => bail!("persistent component codec {} is not implemented",component_name(id)?),
    }
    Ok(w.into_inner())
}
fn from_wire(id: i32, data: &[u8], depth: usize) -> Result<Tag> {
    ensure!(depth<=MAX_DEPTH,"nested component depth exceeded");
    let mut r=Reader::new(data);
    let value=match id {
        0|37|38|39 => {let t=read_tag(&mut r)?;compound(&t)?;t}
        1|2|3|13|16|26|41 => Tag::Int(r.varint()?),
        4 => {let show=r.bool()?;if show{empty()}else{record(vec![("show_in_tooltip",Tag::Byte(0))])}}
        5|6 => component_to_json_string(&read_tag(&mut r)?,depth)?,
        7 => {let count=read_count(&mut r,256)?;let mut lines=Vec::new();for _ in 0..count {lines.push(component_to_json_string(&read_tag(&mut r)?,depth)?);}list_tag(8,lines)}
        8 => {let id=r.varint()?;Tag::String(["common","uncommon","rare","epic"].get(id as usize).context("unknown rarity")?.to_string().into())}
        9|23 => {let count=read_count(&mut r,256)?;let mut levels=BTreeMap::new();for _ in 0..count {let id=r.varint()?;let level=limited(r.varint()?,0,255)?;let name=enchantment_keys().get(id as usize).context("unknown enchantment id")?;levels.insert(name.clone(),Tag::Int(level));}
            let mut out=record(vec![("levels",Tag::Compound(levels.into_iter().map(|(k,v)|(k.into(),v)).collect()))]);if !r.bool()? {out.insert("show_in_tooltip",Tag::Byte(0))?;}out}
        14|15|21 => empty(),
        18 => Tag::Byte(i8::from(r.bool()?)),
        19 => {let _=read_tag(&mut r)?;empty()}
        24 => {let color=r.i32()?;let mut value=record(vec![("rgb",Tag::Int(color))]);if !r.bool()?{value.insert("show_in_tooltip",Tag::Byte(0))?;}value}
        25 => Tag::Int(r.i32()?),
        29|30 => {let count=read_count(&mut r,256)?;let mut items=Vec::new();for _ in 0..count {let item=read_stack(&mut r)?;ensure!(!item.is_empty(),"nonempty item stream is empty");items.push(encode_stack(&item,None,depth+1)?);}list_tag(10,items)}
        43 => {let value=read_tag(&mut r)?;for t in list(&value)?{key(text(t)?)?;}value}
        48 => Tag::String(key(&r.string(32767)?)?.into()),
        50 => {let id=r.varint()?;Tag::String(COLORS.get(id as usize).context("unknown dye id")?.to_string().into())}
        52 => {let count=read_count(&mut r,256)?;let mut slots=Vec::new();for slot in 0..count {let item=read_stack(&mut r)?;if !item.is_empty(){slots.push(record(vec![("slot",Tag::Int(slot as i32)),("item",encode_stack(&item,None,depth+1)?)]));}}list_tag(10,slots)}
        53 => {let count=read_count(&mut r,65536)?;let mut props=BTreeMap::new();for _ in 0..count{props.insert(r.string(32767)?,Tag::String(r.string(32767)?.into()));}Tag::Compound(props.into_iter().map(|(k,v)|(k.into(),v)).collect())}
        _ => bail!("persistent component codec {} is not implemented",component_name(id)?),
    };
    r.finish()?;
    Ok(value)
}
const COLORS: [&str;16]=["white","orange","magenta","light_blue","yellow","lime","pink","gray","light_gray","cyan","purple","blue","brown","green","red","black"];
fn read_count(r:&mut Reader<'_>,max:usize)->Result<usize>{let n=r.varint()?;ensure!(n>=0&&(n as usize)<=max,"component list length invalid");Ok(n as usize)}
fn read_tag(r:&mut Reader<'_>)->Result<Tag>{let (tag,used)=Tag::decode_anonymous_prefix(r.remaining())?;r.bytes(used)?;Ok(tag)}
fn read_stack(r:&mut Reader<'_>)->Result<ItemStack>{let (stack,used)=ItemStack::decode(r.remaining())?;r.bytes(used)?;Ok(stack)}

// Item names/lore use the persistent JSON Component serializer, whereas their
// stream codec uses structured NBT. Unsupported content/style keys are rejected.
fn component_from_json_string(t:&Tag,depth:usize)->Result<Tag>{let v:Value=serde_json::from_str(text(t)?)?;json_component(&v,depth+1)}
pub(crate) fn text_component_to_wire(t:&Tag)->Result<Vec<u8>>{component_from_json_string(t,0)?.encode_network_anonymous()}
pub(crate) fn text_component_from_wire(bytes:&[u8])->Result<Tag>{component_to_json_string(&Tag::decode_anonymous(bytes)?,0)}
fn json_component(v:&Value,depth:usize)->Result<Tag>{
    ensure!(depth<=MAX_DEPTH,"text component nesting exceeded");
    match v {
        Value::String(s)=>Ok(Tag::String(s.clone().into())),
        Value::Bool(v)=>Ok(Tag::String(v.to_string().into())),
        Value::Number(v)=>Ok(Tag::String(v.to_string().into())),
        Value::Array(values)=>{let (first,rest)=values.split_first().context("empty text component array")?;let mut out=json_component(first,depth+1)?;if !rest.is_empty(){if let Tag::String(s)=out{out=record(vec![("text",Tag::String(s))]);}let mut extra=out.get("extra").map(list).transpose()?.unwrap_or(&[]).to_vec();for v in rest{extra.push(json_component(v,depth+1)?);}out.insert("extra",mixed_list(extra))?;}Ok(out)}
        Value::Object(fields)=>{
            let mut out=empty();let mut content=false;
            for (name,v) in fields {
                match name.as_str(){
                    "text"|"translate"|"fallback"|"keybind"|"selector"|"nbt"|"block"|"entity"|"storage"|"color"|"font"|"insertion"=>{out.insert(name.as_str(),Tag::String(v.as_str().context("text field requires a string")?.into()))?;if matches!(name.as_str(),"text"|"translate"|"keybind"|"selector"|"nbt"){content=true;}}
                    "bold"|"italic"|"underlined"|"strikethrough"|"obfuscated"|"interpret"=>{out.insert(name.as_str(),Tag::Byte(i8::from(v.as_bool().context("text field requires boolean")?)))?;}
                    "extra"=>{let values=v.as_array().context("extra requires array")?;ensure!(!values.is_empty(),"extra cannot be empty");out.insert("extra",mixed_list(values.iter().map(|v|json_component(v,depth+1)).collect::<Result<_>>()?))?;}
                    "separator"=>{out.insert("separator",json_component(v,depth+1)?)?;}
                    _=>bail!("unimplemented text component field {name}"),
                }
            }
            ensure!(content,"text component has no supported content");Ok(out)
        }
        Value::Null=>bail!("null text component"),
    }
}
fn component_to_json_string(t:&Tag,depth:usize)->Result<Tag>{Ok(Tag::String(serde_json::to_string(&component_json(t,depth+1)?)?.into()))}
fn component_json(t:&Tag,depth:usize)->Result<Value>{
    ensure!(depth<=MAX_DEPTH,"text component nesting exceeded");
    if let Tag::String(s)=t{return Ok(Value::String(s.as_str().context("text Unicode")?.into()));}
    let mut out=Map::new();let mut content=false;
    for (name,t) in compound(t)?{
        let name=name.as_str().context("text field Unicode")?;
        let value=match name{
            "text"|"translate"|"fallback"|"keybind"|"selector"|"nbt"|"block"|"entity"|"storage"|"color"|"font"|"insertion"=>{if matches!(name,"text"|"translate"|"keybind"|"selector"|"nbt"){content=true;}Value::String(text(t)?.into())}
            "bold"|"italic"|"underlined"|"strikethrough"|"obfuscated"|"interpret"=>Value::Bool(flag(t)?),
            "extra"=>Value::Array(list(t)?.iter().map(|t|component_json(unwrapped(t),depth+1)).collect::<Result<_>>()?),
            "separator"=>component_json(t,depth+1)?,
            _=>bail!("unimplemented text component field {name}"),
        };out.insert(name.into(),value);
    }
    ensure!(content,"text component has no supported content");Ok(Value::Object(out))
}
fn mixed_list(values:Vec<Tag>)->Tag{
    if values.is_empty(){return list_tag(0,values);}
    let first=values[0].tag_id();
    if values.iter().all(|v|v.tag_id()==first){list_tag(first,values)}else{list_tag(10,values.into_iter().map(|v|if matches!(v,Tag::Compound(_)){v}else{record(vec![("",v)])}).collect())}
}
fn unwrapped(t:&Tag)->&Tag{if let Tag::Compound(fields)=t{if fields.len()==1&&fields[0].0==""{return &fields[0].1;}}t}
