use crate::{
    director::lingo::datum::Datum,
    player::{
        DatumRef, DirPlayer, ScriptError,
        allocator::ScriptInstanceAllocatorTrait,
        ci_string::{CiStr, CiString},
        score::get_concrete_sprite_rect,
        script_ref::ScriptInstanceRef,
    },
};

use super::VirtualScriptHandler;

pub struct DoorGuard;

impl VirtualScriptHandler for DoorGuard {
    fn has_handler(&self, name: &str) -> bool {
        name.eq_ignore_ascii_case("checkHit") || name.eq_ignore_ascii_case("checkNoHit")
    }

    fn call_handler(
        &self,
        player: &mut DirPlayer,
        instance: Option<&ScriptInstanceRef>,
        name: &str,
        _args: &Vec<DatumRef>,
    ) -> Result<Option<DatumRef>, ScriptError> {
        if name.eq_ignore_ascii_case("checkNoHit") {
            let Some(instance_ref) = instance else {
                return Ok(None);
            };

            if is_entry_door_explicitly_targeted(player, instance_ref)? {
                set_instance_prop(
                    player,
                    instance_ref,
                    "pStatus",
                    Datum::Symbol("checkHit".to_string()),
                );
                return Ok(Some(DatumRef::Void));
            }

            if door_points_to_previous_room(player, instance_ref)? {
                return Ok(Some(DatumRef::Void));
            }

            return Ok(None);
        }

        if !name.eq_ignore_ascii_case("checkHit") {
            return Ok(None);
        }

        let Some(instance_ref) = instance else {
            return Ok(None);
        };

        if should_debounce_entry_door(player, instance_ref)? {
            set_instance_prop(
                player,
                instance_ref,
                "pStatus",
                Datum::Symbol("checkNoHit".to_string()),
            );
            return Ok(Some(DatumRef::Void));
        }

        Ok(None)
    }
}

fn should_debounce_entry_door(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
) -> Result<bool, ScriptError> {
    if !door_points_to_previous_room(player, instance_ref)? {
        return Ok(false);
    }

    if is_entry_door_explicitly_targeted(player, instance_ref)? {
        return Ok(false);
    }

    let Some(door_sprite) = get_instance_sprite_prop(player, instance_ref, "pSprite")? else {
        return Ok(false);
    };
    let Some(user_sprite) = get_instance_sprite_prop(player, instance_ref, "pUser")? else {
        return Ok(false);
    };

    Ok(sprites_intersect(player, door_sprite, user_sprite))
}

fn is_entry_door_explicitly_targeted(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
) -> Result<bool, ScriptError> {
    if !door_points_to_previous_room(player, instance_ref)? {
        return Ok(false);
    }

    let Some(door_point) = get_door_iso_point(player, instance_ref)? else {
        return Ok(false);
    };

    let Some(walk_list_ref) = get_global_list_instance_prop(player, "gOnline", 1, "pWalkList")?
    else {
        return Ok(false);
    };

    let Datum::List(_, walk_list, _) = player.get_datum(&walk_list_ref) else {
        return Ok(false);
    };

    for point_ref in walk_list {
        if let Datum::Point(point, _) = player.get_datum(point_ref) {
            if near_room_point(door_point, *point) {
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn door_points_to_previous_room(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
) -> Result<bool, ScriptError> {
    let Some(target_room) = get_instance_string_prop(player, instance_ref, "pRoomId")? else {
        return Ok(false);
    };
    let Some(previous_room) = get_global_instance_string_prop(player, "gUser", "pOldRoomGroup")?
    else {
        return Ok(false);
    };

    Ok(!previous_room.is_empty()
        && room_name_key(&target_room).eq_ignore_ascii_case(room_name_key(&previous_room)))
}

fn room_name_key(name: &str) -> &str {
    name.strip_prefix('@').unwrap_or(name)
}

fn sprites_intersect(player: &DirPlayer, left_sprite_num: i16, right_sprite_num: i16) -> bool {
    let Some(left_sprite) = player.movie.score.get_sprite(left_sprite_num) else {
        return false;
    };
    let Some(right_sprite) = player.movie.score.get_sprite(right_sprite_num) else {
        return false;
    };

    let left = get_concrete_sprite_rect(player, left_sprite);
    let right = get_concrete_sprite_rect(player, right_sprite);

    !(left.right <= right.left
        || left.left >= right.right
        || left.bottom <= right.top
        || left.top >= right.bottom)
}

fn near_room_point(left: [f64; 2], right: [f64; 2]) -> bool {
    (left[0].round() as i32 - right[0].round() as i32).abs() <= 1
        && (left[1].round() as i32 - right[1].round() as i32).abs() <= 1
}

fn get_door_iso_point(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
) -> Result<Option<[f64; 2]>, ScriptError> {
    let Some(loc) = get_instance_point_prop(player, instance_ref, "pLoc")? else {
        return Ok(None);
    };
    let grid_ratio = get_global_instance_number_prop(player, "gMain", "pGridRatio")?.unwrap_or(3.0);

    let x = ((loc[1] + (loc[0] / grid_ratio)) * (grid_ratio / 2.0) / 24.0) as i32 + 1;
    let y = ((loc[1] - (loc[0] / grid_ratio)) * (grid_ratio / 2.0) / 24.0) as i32 + 1;

    Ok(Some([x as f64, y as f64]))
}

fn get_instance_sprite_prop(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    name: &str,
) -> Result<Option<i16>, ScriptError> {
    let Some(value_ref) = get_instance_prop(player, instance_ref, name) else {
        return Ok(None);
    };
    match player.get_datum(&value_ref) {
        Datum::SpriteRef(sprite_num) => Ok(Some(*sprite_num)),
        Datum::Int(sprite_num) => Ok(Some(*sprite_num as i16)),
        Datum::Void => Ok(None),
        other => Err(ScriptError::new(format!(
            "Expected sprite property {}, got {}",
            name,
            other.type_str()
        ))),
    }
}

fn get_instance_point_prop(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    name: &str,
) -> Result<Option<[f64; 2]>, ScriptError> {
    let Some(value_ref) = get_instance_prop(player, instance_ref, name) else {
        return Ok(None);
    };
    match player.get_datum(&value_ref) {
        Datum::Point(vals, _) => Ok(Some(*vals)),
        Datum::Void => Ok(None),
        other => Err(ScriptError::new(format!(
            "Expected point property {}, got {}",
            name,
            other.type_str()
        ))),
    }
}

fn get_instance_string_prop(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    name: &str,
) -> Result<Option<String>, ScriptError> {
    let Some(value_ref) = get_instance_prop(player, instance_ref, name) else {
        return Ok(None);
    };
    match player.get_datum(&value_ref) {
        Datum::Void => Ok(None),
        datum => Ok(Some(datum.string_value()?)),
    }
}

fn get_instance_number_prop(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    name: &str,
) -> Result<Option<f64>, ScriptError> {
    let Some(value_ref) = get_instance_prop(player, instance_ref, name) else {
        return Ok(None);
    };
    match player.get_datum(&value_ref) {
        Datum::Int(value) => Ok(Some(*value as f64)),
        Datum::Float(value) => Ok(Some(*value)),
        Datum::Void => Ok(None),
        other => Err(ScriptError::new(format!(
            "Expected number property {}, got {}",
            name,
            other.type_str()
        ))),
    }
}

fn get_global_instance_string_prop(
    player: &mut DirPlayer,
    global_name: &str,
    prop_name: &str,
) -> Result<Option<String>, ScriptError> {
    let Some(global_ref) = player.globals.get(global_name).cloned() else {
        return Ok(None);
    };
    let Datum::ScriptInstanceRef(instance_ref) = player.get_datum(&global_ref) else {
        return Ok(None);
    };
    get_instance_string_prop(player, &instance_ref.clone(), prop_name)
}

fn get_global_instance_number_prop(
    player: &mut DirPlayer,
    global_name: &str,
    prop_name: &str,
) -> Result<Option<f64>, ScriptError> {
    let Some(global_ref) = player.globals.get(global_name).cloned() else {
        return Ok(None);
    };
    let Datum::ScriptInstanceRef(instance_ref) = player.get_datum(&global_ref) else {
        return Ok(None);
    };
    get_instance_number_prop(player, &instance_ref.clone(), prop_name)
}

fn get_global_list_instance_prop(
    player: &mut DirPlayer,
    global_name: &str,
    one_based_index: usize,
    prop_name: &str,
) -> Result<Option<DatumRef>, ScriptError> {
    let Some(global_ref) = player.globals.get(global_name).cloned() else {
        return Ok(None);
    };
    let Datum::List(_, items, _) = player.get_datum(&global_ref) else {
        return Ok(None);
    };
    let Some(item_ref) = items.get(one_based_index.saturating_sub(1)).cloned() else {
        return Ok(None);
    };
    let Datum::ScriptInstanceRef(instance_ref) = player.get_datum(&item_ref) else {
        return Ok(None);
    };
    Ok(get_instance_prop(player, &instance_ref.clone(), prop_name))
}

fn get_instance_prop(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    name: &str,
) -> Option<DatumRef> {
    player
        .allocator
        .get_script_instance(instance_ref)
        .properties
        .get(CiStr::new(name))
        .cloned()
}

fn set_instance_prop(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    name: &str,
    value: Datum,
) {
    let value_ref = player.alloc_datum(value);
    player
        .allocator
        .get_script_instance_mut(instance_ref)
        .properties
        .insert(CiString::from(name), value_ref);
}
