// SPDX-License-Identifier: GPL-2.0-only
//! Executed-original witnesses against Main's actual retained/file-backed ban owner.
use super::*;
#[test]
fn original_pardon_ip_outcomes_persist_exactly_one_retained_key() {
    let fixture:serde_json::Value=serde_json::from_str(include_str!("../fixtures/pardon-ip-command-vectors.json")).unwrap();
    for (index,row) in fixture["execution"].as_array().unwrap().iter().enumerate(){
        let directory=std::env::temp_dir().join(format!("pardon-ip-host-{}-{}-{}",std::process::id(),index,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&directory).unwrap();
        let entries=["192.0.2.7","198.51.100.9","2001:db8::1"].map(|ip|serde_json::json!({
            "ip":ip,"created":"1970-01-01 00:00:00 +0000","source":"Console","reason":"fixture",
            "expires":if ip=="198.51.100.9"{"1970-01-01 00:00:01 +0000"}else{"forever"}}));
        let path=directory.join("banned-ips.json");std::fs::write(&path,serde_json::to_vec(&entries).unwrap()).unwrap();
        let mut owner=Administration::load(directory.clone(),Default::default(),2000).unwrap();
        let before=std::fs::read(&path).unwrap();
        let outcome=owner.execute_line(row["input"].as_str().unwrap(),&CommandSource{name:"Test".into(),permission_level:row["permission"].as_u64().unwrap() as u8},None,&[],2000);
        assert_eq!(outcome.result.is_ok(),row["ok"].as_bool().unwrap(),"{row}");
        if let Ok(result)=&outcome.result{assert_eq!(*result,row["result"].as_i64().unwrap() as i32);}
        if !row["ok"].as_bool().unwrap(){assert_eq!(std::fs::read(&path).unwrap(),before,"failed command wrote IP bans: {row}");}
        let mut retained=owner.lists().ip_bans.keys().map(String::as_str).collect::<Vec<_>>();retained.sort();
        assert_eq!(retained,row["remaining"].as_array().unwrap().iter().map(|v|v.as_str().unwrap()).collect::<Vec<_>>());
        let mut messages=Vec::new();
        for effect in outcome.effects {
            if let AdminEffect::Feedback{message,broadcast_to_ops}=effect{assert!(broadcast_to_ops);messages.push(crate::main_time_command::feedback_component(&message).unwrap());}
        }
        if row["parsed"]==true {
            if let Err(CommandError::Message(message))=&outcome.result{
                assert_eq!(message.key,row["error_key"].as_str().unwrap());
                messages.push(mc121_rust::components::text::Component::from_json(&serde_json::json!({"text":"","extra":[message.json_component()],"color":"red"})).unwrap().to_network_nbt().unwrap());
            }
            let encoded=messages.into_iter().map(|mut b|{b.push(0);b.iter().map(|v|format!("{v:02x}")).collect::<String>()}).collect::<Vec<_>>();
            assert_eq!(encoded,row["feedback_hex"].as_array().unwrap().iter().map(|v|v.as_str().unwrap()).collect::<Vec<_>>(),"{row}");
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
