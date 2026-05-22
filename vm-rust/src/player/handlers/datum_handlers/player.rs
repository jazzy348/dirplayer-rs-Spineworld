use super::super::types::TypeHandlers;
use crate::{
    director::lingo::datum::Datum,
    player::{DatumRef, ScriptError, ScriptErrorCode, reserve_player_mut, reserve_player_ref},
};

pub struct PlayerDatumHandlers {}

impl PlayerDatumHandlers {
    pub fn call(handler_name: &str, args: &Vec<DatumRef>) -> Result<DatumRef, ScriptError> {
        match handler_name {
            "count" => Self::count(args),
            "cursor" => TypeHandlers::cursor(args),
            _ => reserve_player_ref(|player| {
                Err(ScriptError::new_code(
                    ScriptErrorCode::HandlerNotFound,
                    format!("No handler {handler_name} for player datum"),
                ))
            }),
        }
    }

    fn count(args: &Vec<DatumRef>) -> Result<DatumRef, ScriptError> {
        reserve_player_mut(|player| {
            let subject = player.get_datum(&args[0]).string_value().unwrap();
            match subject.as_str() {
                "windowList" => Ok(player.alloc_datum(Datum::Int(0))),
                _ => Err(ScriptError::new(
                    format!("Invalid call _player.count({subject})").to_string(),
                )),
            }
        })
    }
}
