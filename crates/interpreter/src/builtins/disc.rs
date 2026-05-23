use std::sync::Arc;
use std::collections::HashMap;
use crate::{Ouroboros, EvalContext, BuiltinFn, Peer};
use crate::value::{Value, EffectTag, BottomCause, ContentHash};
use crate::storage::ObjectStore;
use nlang_parser::ast::AtomKind;

pub fn register_disc_builtins(m: &mut HashMap<String, Arc<BuiltinFn>>) {
    m.insert("disc.connect".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        if let Value::Combo(ref c) = arg { 
            if let (Some(vname), Some(vpath)) = (c.get_field("0"), c.get_field("1")) {
                let name = oo.force(vname.clone(), ctx).to_string_plain();
                let path_str = oo.force(vpath.clone(), ctx).to_string_plain();
                if path_str.starts_with("tcp://") {
                    if let Ok(mut peers) = oo.peers.write() {
                        peers.insert(name, Peer::Remote(path_str[6..].to_string()));
                        return Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None);
                    }
                } else {
                    let path = std::path::PathBuf::from(path_str);
                    if let Ok(store) = ObjectStore::init(&path) {
                        if let Ok(mut peers) = oo.peers.write() {
                            peers.insert(name, Peer::Local(Arc::new(store)));
                            return Value::Atom(AtomKind::Tag("true".to_string()), EffectTag::IO, None);
                        }
                    }
                }
            }
        }
        Value::Atom(AtomKind::Tag("false".to_string()), EffectTag::IO, None)
    }) as Arc<BuiltinFn>);
    
    m.insert("disc.fetch".to_string(), Arc::new(|arg: Value, oo: &Ouroboros, ctx: &mut EvalContext| {
        let (node_name, caid_str) = if let Value::Combo(ref c) = arg {
            if let (Some(v0), Some(v1)) = (c.get_field("0"), c.get_field("1")) {
                (Some(oo.force(v0.clone(), ctx).to_string_plain()), oo.force(v1.clone(), ctx).to_string_plain())
            } else if let Some(v0) = c.get_field("0") {
                (None, oo.force(v0.clone(), ctx).to_string_plain())
            } else { return BottomCause::Conflict.into(); }
        } else { (None, arg.collapse().to_string_plain()) };

        if let Ok(hash) = ContentHash::parse(&caid_str) {
            if let Some(name) = node_name {
                let peer_opt = if let Ok(peers) = oo.peers.read() { peers.get(&name).cloned() } else { None };
                if let Some(peer) = peer_opt {
                    match peer {
                        Peer::Local(store) => { if let Ok(val) = store.get_value(&hash) { return val; } }
                        Peer::Remote(addr) => { if let Ok(val) = oo.remote_fetch(&addr, &hash) { return val; } }
                    }
                }
            } else {
                let mut results = Vec::new();
                if let Ok(val) = oo.store.get_value(&hash) { results.push(val); }
                
                let peers_copy = if let Ok(peers) = oo.peers.read() { peers.values().cloned().collect::<Vec<_>>() } else { vec![] };
                for peer in peers_copy {
                    match peer {
                        Peer::Local(store) => { if let Ok(val) = store.get_value(&hash) { results.push(val); } }
                        Peer::Remote(addr) => { if let Ok(val) = oo.remote_fetch(&addr, &hash) { return val; } }
                    }
                }
                
                if results.is_empty() { return BottomCause::Conflict.into(); }
                
                let mut final_val = results.remove(0);
                for v in results {
                    let merged = oo.unify_internal(final_val.clone(), v.clone(), ctx);
                    if let Value::Bottom(_) = merged {
                        if v.bits() > final_val.bits() { final_val = v; }
                    } else {
                        final_val = merged;
                    }
                }
                return final_val;
            }
        }
        BottomCause::Conflict.into()
    }) as Arc<BuiltinFn>);
    
    m.insert("disc.identify".to_string(), Arc::new(|arg: Value, _oo: &Ouroboros, _ctx: &mut EvalContext| {
        Value::Atom(AtomKind::Str(arg.content_hash().to_string()), EffectTag::Pure, None)
    }) as Arc<BuiltinFn>);
}