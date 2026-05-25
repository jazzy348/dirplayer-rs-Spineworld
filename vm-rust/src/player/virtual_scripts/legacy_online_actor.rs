use std::collections::VecDeque;

use fxhash::FxHashMap;

use crate::{
    director::lingo::datum::{Datum, DatumType},
    player::{
        DatumRef, DirPlayer, ScriptError,
        allocator::ScriptInstanceAllocatorTrait,
        ci_string::{CiStr, CiString},
        script_ref::ScriptInstanceRef,
    },
};

use super::VirtualScriptHandler;

const PROPERTIES: &[&str] = &[
    "pStatus",
    "pPoint",
    "pLoc",
    "pPathLoc",
    "pWalkList",
    "pKeyWalklist",
    "pKeyPoint",
    "pSpeedH",
    "pSpeedV",
    "pSpeed",
    "pWalkTime",
    "pWalkSpeed",
    "pTime",
    "pWalkStatus",
    "pWalkType",
    "pAvatar",
    "pOnlinePos",
    "pLastPoint",
    "pAppearance",
    "pEmoticon",
    "pEmoticonNr",
    "pSpritePos",
    "pSitHeight",
    "pIsLeaving",
    "pFadeUp",
    "pName",
    "pGender",
    "pInfo",
    "pAge",
    "pAction",
    "pCitizenType",
    "pPet",
    "pUsePet",
];

pub struct LegacyOnlineActor {
    display_name: &'static str,
}

impl LegacyOnlineActor {
    pub fn new(display_name: &'static str) -> Self {
        Self { display_name }
    }
}

impl VirtualScriptHandler for LegacyOnlineActor {
    fn has_handler(&self, name: &str) -> bool {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "new"
                | "gomove"
                | "updateuser"
                | "setupleave"
                | "dosetupleave"
                | "getavatarsprite"
                | "getinfoimage"
                | "getmyheight"
                | "setemoticon"
                | "kill"
                | "killemoticon"
                | "setkeywalk"
                | "setmousewalk"
                | "setaction"
                | "setactionfromlist"
                | "addpet"
                | "removepet"
        )
    }

    fn get_property_names(&self) -> Vec<String> {
        PROPERTIES.iter().map(|name| name.to_string()).collect()
    }

    fn call_handler(
        &self,
        player: &mut DirPlayer,
        instance: Option<&ScriptInstanceRef>,
        name: &str,
        args: &Vec<DatumRef>,
    ) -> Result<Option<DatumRef>, ScriptError> {
        let Some(instance_ref) = instance else {
            return Ok(None);
        };

        match name.to_ascii_lowercase().as_str() {
            "new" => {
                let online_pos = args
                    .get(0)
                    .map(|arg| player.get_datum(arg).int_value())
                    .transpose()?
                    .unwrap_or(0);
                self.init_instance(player, instance_ref, online_pos);
                Ok(Some(player.alloc_datum(Datum::ScriptInstanceRef(
                    instance_ref.clone(),
                ))))
            }
            "setupleave" | "dosetupleave" => Ok(Some(player.alloc_datum(Datum::Int(0)))),
            "kill" => {
                set_prop(
                    player,
                    instance_ref,
                    "pStatus",
                    Datum::Symbol("killed".to_string()),
                );
                Ok(Some(DatumRef::Void))
            }
            "setemoticon" => {
                let emoticon_nr = args
                    .get(0)
                    .map(|arg| player.get_datum(arg).int_value())
                    .transpose()?
                    .unwrap_or(0);
                set_prop(player, instance_ref, "pEmoticonNr", Datum::Int(emoticon_nr));
                Ok(Some(DatumRef::Void))
            }
            "setkeywalk" => {
                set_prop(
                    player,
                    instance_ref,
                    "pWalkType",
                    Datum::Symbol("keyWalk".to_string()),
                );
                Ok(Some(DatumRef::Void))
            }
            "setmousewalk" => {
                set_prop(
                    player,
                    instance_ref,
                    "pWalkType",
                    Datum::Symbol("mouseWalk".to_string()),
                );
                Ok(Some(DatumRef::Void))
            }
            "getmyheight" => Ok(Some(player.alloc_datum(Datum::Int(0)))),
            "getavatarsprite" => {
                let sprite_pos = get_int_prop(player, instance_ref, "pSpritePos").unwrap_or(0);
                Ok(Some(
                    player.alloc_datum(Datum::SpriteRef(sprite_pos as i16)),
                ))
            }
            "getinfoimage" => Ok(Some(DatumRef::Void)),
            "gomove" | "updateuser" | "killemoticon" | "setaction" | "setactionfromlist"
            | "addpet" | "removepet" => Ok(Some(DatumRef::Void)),
            _ => Ok(None),
        }
    }

    fn get_prop(
        &self,
        player: &mut DirPlayer,
        instance: &ScriptInstanceRef,
        name: &str,
    ) -> Result<Option<DatumRef>, ScriptError> {
        let script_instance = player.allocator.get_script_instance(instance);
        Ok(script_instance.properties.get(CiStr::new(name)).cloned())
    }

    fn set_prop(
        &self,
        player: &mut DirPlayer,
        instance: &ScriptInstanceRef,
        name: &str,
        value: &DatumRef,
    ) -> Result<Option<()>, ScriptError> {
        let script_instance = player.allocator.get_script_instance_mut(instance);
        script_instance
            .properties
            .insert(CiString::from(name.to_owned()), value.clone());
        Ok(Some(()))
    }
}

impl LegacyOnlineActor {
    fn init_instance(
        &self,
        player: &mut DirPlayer,
        instance_ref: &ScriptInstanceRef,
        online_pos: i32,
    ) {
        let values = [
            ("pStatus", Datum::Symbol("chat".to_string())),
            ("pPoint", point(0.0, 0.0)),
            ("pLoc", point(-200.0, -200.0)),
            ("pPathLoc", point(-200.0, -200.0)),
            ("pWalkList", list()),
            ("pKeyWalklist", list()),
            ("pKeyPoint", point(0.0, 0.0)),
            ("pSpeedH", Datum::Int(0)),
            ("pSpeedV", Datum::Int(0)),
            ("pSpeed", Datum::Int(0)),
            ("pWalkTime", Datum::Int(0)),
            ("pWalkSpeed", Datum::Float(0.0)),
            ("pTime", Datum::Int(0)),
            ("pWalkStatus", Datum::Symbol("stand".to_string())),
            ("pWalkType", Datum::Symbol("mouseWalk".to_string())),
            ("pAvatar", Datum::Int(0)),
            ("pOnlinePos", Datum::Int(online_pos)),
            ("pLastPoint", point(0.0, 0.0)),
            ("pAppearance", list()),
            ("pEmoticon", Datum::Int(0)),
            ("pEmoticonNr", Datum::Int(0)),
            ("pSpritePos", Datum::Int(200 + online_pos * 3)),
            ("pSitHeight", Datum::Int(0)),
            ("pIsLeaving", Datum::Int(0)),
            ("pFadeUp", Datum::Int(0)),
            ("pName", Datum::String(self.display_name.to_string())),
            ("pGender", Datum::Int(0)),
            ("pInfo", Datum::String(String::new())),
            ("pAge", Datum::String(String::new())),
            ("pAction", Datum::Void),
            ("pCitizenType", Datum::Int(0)),
            ("pPet", Datum::Int(0)),
            ("pUsePet", Datum::Int(0)),
        ];

        let mut refs = FxHashMap::default();
        for (name, value) in values {
            refs.insert(CiString::from(name), player.alloc_datum(value));
        }

        let script_instance = player.allocator.get_script_instance_mut(instance_ref);
        script_instance.properties.extend(refs);
    }
}

fn set_prop(player: &mut DirPlayer, instance_ref: &ScriptInstanceRef, name: &str, value: Datum) {
    let value_ref = player.alloc_datum(value);
    let script_instance = player.allocator.get_script_instance_mut(instance_ref);
    script_instance
        .properties
        .insert(CiString::from(name), value_ref);
}

fn get_int_prop(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    name: &str,
) -> Option<i32> {
    let script_instance = player.allocator.get_script_instance(instance_ref);
    script_instance
        .properties
        .get(CiStr::new(name))
        .and_then(|value_ref| player.get_datum(value_ref).int_value().ok())
}

fn point(x: f64, y: f64) -> Datum {
    Datum::Point([x, y], 0)
}

fn list() -> Datum {
    Datum::List(DatumType::List, VecDeque::new(), false)
}
