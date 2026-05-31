use std::collections::VecDeque;

use fxhash::FxHashMap;

use crate::{
    director::lingo::datum::{Datum, DatumType, PropListPair},
    js_api::JsApi,
    player::{
        DatumRef, DirPlayer, ScriptError,
        allocator::ScriptInstanceAllocatorTrait,
        bitmap::bitmap::Bitmap,
        cast_lib::CastMemberRef,
        cast_member::{BitmapMember, CastMember, CastMemberType},
        ci_string::{CiStr, CiString},
        handlers::datum_handlers::player_call_datum_handler,
        script_ref::ScriptInstanceRef,
        sprite::ColorRef,
    },
};
use wasm_bindgen_futures::spawn_local;

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
    "pNPClist",
    "pGender",
    "pInfo",
    "pAge",
    "pAction",
    "pCitizenType",
    "pPet",
    "pUsePet",
    "pSpriteNr",
    "pConv",
    "pAnimFrame",
    "pAnimTick",
    "pFrameMember",
];

pub struct LegacyOnlineActor {
    script_name: String,
}

impl LegacyOnlineActor {
    pub fn new(script_name: &str) -> Self {
        Self {
            script_name: script_name.to_string(),
        }
    }
}

#[derive(Clone, Copy)]
struct ActorProfile {
    display_name: &'static str,
    member_candidates: &'static [&'static str],
    loc: (f64, f64),
    has_dialogue: bool,
    frame_count: usize,
    frame_duration: i32,
    conversation_title: &'static str,
    conversation_lines: &'static [&'static str],
}

const BARTENDER_AQUA_MEMBERS: &[&str] = &["obj_bartender"];
const BARTENDER_DEN_MEMBERS: &[&str] = &["obj_bartender2", "obj_bartender"];
const BARTENDER_MEMBERS: &[&str] = &["obj_bartender", "obj_bartender2"];
const BARTENDER_AQUA_LINES: &[&str] = &[
    "Welcome to the Aqua Lounge.",
    "Take it easy and enjoy the bubbles.",
];
const BARTENDER_DEN_LINES: &[&str] = &[
    "Welcome to the Den.",
    "Pull up a seat and make yourself comfortable.",
];

fn actor_profile(script_name: &str) -> ActorProfile {
    let name = script_name.to_ascii_lowercase();
    match name.as_str() {
        "pbartender_aqua" => ActorProfile {
            display_name: "Bartender",
            member_candidates: BARTENDER_AQUA_MEMBERS,
            loc: (-480.0, 315.0),
            has_dialogue: true,
            frame_count: 18,
            frame_duration: 8,
            conversation_title: "Hello there.",
            conversation_lines: BARTENDER_AQUA_LINES,
        },
        "pbartender_den" => ActorProfile {
            display_name: "Bartender",
            member_candidates: BARTENDER_DEN_MEMBERS,
            loc: (225.0, 125.0),
            has_dialogue: true,
            frame_count: 21,
            frame_duration: 7,
            conversation_title: "Good to see you.",
            conversation_lines: BARTENDER_DEN_LINES,
        },
        _ => ActorProfile {
            display_name: "Bartender",
            member_candidates: BARTENDER_MEMBERS,
            loc: (-200.0, -200.0),
            has_dialogue: false,
            frame_count: 1,
            frame_duration: 8,
            conversation_title: "",
            conversation_lines: &[],
        },
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
                | "doclick"
                | "domouseover"
                | "lookatowner"
                | "checkemoticon"
                | "shownamesign"
                | "frameevent"
                | "startconv"
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
                let sprite_pos = get_int_prop(player, instance_ref, "pSpritePos").unwrap_or(0);
                let online_pos = get_int_prop(player, instance_ref, "pOnlinePos").unwrap_or(0);
                let actor_sprite_pos = actor_visible_sprite_pos(sprite_pos);
                clear_actor_sprite(player, sprite_pos);
                clear_actor_sprite(player, actor_sprite_pos);
                remove_object_list_entry(player, "pObjButtonList", actor_sprite_pos);
                remove_object_list_entry(player, "pObjMouseOverList", actor_sprite_pos);
                clear_online_actor_slot(player, online_pos);
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
                Ok(Some(player.alloc_datum(Datum::SpriteRef(
                    actor_visible_sprite_pos(sprite_pos) as i16,
                ))))
            }
            "getinfoimage" => Ok(Some(DatumRef::Void)),
            "shownamesign" => {
                if let Some(sprite_num) = args
                    .get(0)
                    .and_then(|arg| player.get_datum(arg).to_sprite_ref().ok())
                {
                    show_actor_name_sign(player, instance_ref, sprite_num);
                }
                Ok(Some(DatumRef::Void))
            }
            "domouseover" => {
                if actor_profile(&self.script_name).has_dialogue {
                    set_cursor_version(player, "npc");
                }
                Ok(Some(DatumRef::Void))
            }
            "doclick" | "startconv" => {
                let profile = actor_profile(&self.script_name);
                if profile.has_dialogue {
                    start_actor_conversation(player, instance_ref, &profile);
                }
                Ok(Some(DatumRef::Void))
            }
            "frameevent" | "gomove" => {
                let sprite_pos = get_int_prop(player, instance_ref, "pSpritePos").unwrap_or(0);
                let profile = actor_profile(&self.script_name);
                let frame = advance_actor_frame(player, instance_ref, &profile);
                let member_ref = get_actor_frame_member(player, instance_ref);
                if let Some(member_ref) = member_ref.as_ref() {
                    update_actor_frame_member(player, &profile, member_ref, frame);
                }
                if let Some((x, y)) = get_point_prop(player, instance_ref, "pLoc") {
                    set_actor_sprite_loc(
                        player,
                        actor_visible_sprite_pos(sprite_pos),
                        x,
                        y,
                        member_ref,
                    );
                }
                Ok(Some(DatumRef::Void))
            }
            "updateuser"
            | "killemoticon"
            | "lookatowner"
            | "checkemoticon"
            | "setaction"
            | "setactionfromlist"
            | "addpet"
            | "removepet" => Ok(Some(DatumRef::Void)),
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
        let profile = actor_profile(&self.script_name);
        let sprite_pos = 200 + online_pos * 3;
        let frame_member = build_actor_frame_member(player, &profile);
        let values = [
            ("pStatus", Datum::Symbol("chat".to_string())),
            ("pPoint", point(0.0, 0.0)),
            ("pLoc", point(profile.loc.0, profile.loc.1)),
            ("pPathLoc", point(profile.loc.0, profile.loc.1)),
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
            ("pSpritePos", Datum::Int(sprite_pos)),
            ("pSitHeight", Datum::Int(0)),
            ("pIsLeaving", Datum::Int(0)),
            ("pFadeUp", Datum::Int(0)),
            ("pName", Datum::String(profile.display_name.to_string())),
            ("pGender", Datum::Int(0)),
            ("pInfo", Datum::String(String::new())),
            ("pAge", Datum::String(String::new())),
            ("pAction", Datum::Void),
            ("pCitizenType", Datum::Int(0)),
            ("pPet", Datum::Int(0)),
            ("pUsePet", Datum::Int(0)),
            ("pSpriteNr", Datum::Int(sprite_pos)),
            ("pConv", list()),
            ("pAnimFrame", Datum::Int(0)),
            ("pAnimTick", Datum::Int(0)),
        ];

        let mut refs = FxHashMap::default();
        for (name, value) in values {
            refs.insert(CiString::from(name), player.alloc_datum(value));
        }

        let script_instance = player.allocator.get_script_instance_mut(instance_ref);
        script_instance.properties.extend(refs);

        let display_name_ref = player.alloc_datum(Datum::String(profile.display_name.to_string()));
        let npc_list_ref = prop_list(player, vec![("name", display_name_ref)]);
        set_instance_prop_ref(player, instance_ref, "pNPClist", npc_list_ref);

        if let Some(first_member_ref) = frame_member {
            let frame_member_ref = player.alloc_datum(Datum::CastMember(first_member_ref.clone()));
            set_instance_prop_ref(player, instance_ref, "pFrameMember", frame_member_ref);
            let actor_sprite_pos = actor_visible_sprite_pos(sprite_pos);
            clear_actor_sprite(player, sprite_pos);
            if assign_actor_sprite(player, &profile, actor_sprite_pos, first_member_ref) {
                append_object_list_entry(player, "pObjButtonList", actor_sprite_pos, instance_ref);
                append_object_list_entry(
                    player,
                    "pObjMouseOverList",
                    actor_sprite_pos,
                    instance_ref,
                );
            }
        }
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

fn get_point_prop(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    name: &str,
) -> Option<(f64, f64)> {
    let script_instance = player.allocator.get_script_instance(instance_ref);
    script_instance
        .properties
        .get(CiStr::new(name))
        .and_then(|value_ref| match player.get_datum(value_ref) {
            Datum::Point(values, _) => Some((values[0], values[1])),
            _ => None,
        })
}

fn assign_actor_sprite(
    player: &mut DirPlayer,
    profile: &ActorProfile,
    sprite_pos: i32,
    member_ref: CastMemberRef,
) -> bool {
    let (width, height) = member_size(player, &member_ref).unwrap_or((0, 0));
    let bg_color =
        member_background_color(player, &member_ref).unwrap_or(ColorRef::PaletteIndex(0));
    let (loc_h, loc_v) = screen_loc(player, profile.loc.0, profile.loc.1);
    let sprite_id = sprite_pos as i16;
    if player.movie.score.get_sprite(sprite_id).is_none() {
        return false;
    }

    let sprite = player.movie.score.get_sprite_mut(sprite_id);
    sprite.puppet = true;
    sprite.visible = true;
    sprite.member = Some(member_ref);
    sprite.loc_h = loc_h;
    sprite.loc_v = loc_v;
    sprite.loc_z = z_for_world_y(profile.loc.1);
    sprite.ink = 36;
    sprite.blend = 100;
    sprite.editable = true;
    sprite.width = width;
    sprite.height = height;
    sprite.base_width = width;
    sprite.base_height = height;
    sprite.has_loc_changed = true;
    sprite.has_size_changed = width > 0 && height > 0;
    sprite.color = ColorRef::PaletteIndex(255);
    sprite.bg_color = bg_color;
    player.stage_dirty = true;
    player.movie.score.invalidate_render_channel_cache();
    JsApi::on_sprite_member_changed(sprite_id);
    true
}

fn set_actor_sprite_loc(
    player: &mut DirPlayer,
    sprite_pos: i32,
    world_x: f64,
    world_y: f64,
    member_ref: Option<CastMemberRef>,
) {
    let sprite_id = sprite_pos as i16;
    if player.movie.score.get_sprite(sprite_id).is_none() {
        return;
    }
    let (loc_h, loc_v) = screen_loc(player, world_x, world_y);
    let member_size = member_ref
        .as_ref()
        .and_then(|member_ref| member_size(player, member_ref));
    let sprite = player.movie.score.get_sprite_mut(sprite_id);
    if let Some(member_ref) = member_ref {
        if sprite.member.as_ref() != Some(&member_ref) {
            sprite.member = Some(member_ref);
            if let Some((width, height)) = member_size {
                sprite.width = width;
                sprite.height = height;
                sprite.base_width = width;
                sprite.base_height = height;
                sprite.has_size_changed = true;
            }
            JsApi::on_sprite_member_changed(sprite_id);
        }
    }
    if sprite.loc_h != loc_h || sprite.loc_v != loc_v {
        sprite.loc_h = loc_h;
        sprite.loc_v = loc_v;
        sprite.loc_z = z_for_world_y(world_y);
        sprite.has_loc_changed = true;
        player.stage_dirty = true;
        player.movie.score.invalidate_render_channel_cache();
    }
}

fn clear_actor_sprite(player: &mut DirPlayer, sprite_pos: i32) {
    let sprite_id = sprite_pos as i16;
    if player.movie.score.get_sprite(sprite_id).is_none() {
        return;
    }
    let sprite = player.movie.score.get_sprite_mut(sprite_id);
    sprite.member = None;
    sprite.loc_h = -100;
    sprite.loc_v = -100;
    sprite.visible = false;
    player.stage_dirty = true;
    player.movie.score.invalidate_render_channel_cache();
    JsApi::on_sprite_member_changed(sprite_id);
}

fn show_actor_name_sign(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    sign_sprite_num: i16,
) {
    let sprite_pos = get_int_prop(player, instance_ref, "pSpritePos").unwrap_or(0);
    let actor_sprite_num = actor_visible_sprite_pos(sprite_pos) as i16;
    let Some(actor_sprite) = player.movie.score.get_sprite(actor_sprite_num) else {
        return;
    };
    let (loc_h, loc_v) = (
        actor_sprite.loc_h,
        actor_sprite.loc_v - actor_sprite.height - 2,
    );
    let sign_sprite = player.movie.score.get_sprite_mut(sign_sprite_num);
    if sign_sprite.loc_h != loc_h || sign_sprite.loc_v != loc_v {
        sign_sprite.loc_h = loc_h;
        sign_sprite.loc_v = loc_v;
        sign_sprite.has_loc_changed = true;
        player.stage_dirty = true;
        player.movie.score.invalidate_render_channel_cache();
    }
}

fn actor_visible_sprite_pos(sprite_pos: i32) -> i32 {
    sprite_pos + 1
}

fn advance_actor_frame(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    profile: &ActorProfile,
) -> usize {
    if profile.frame_count <= 1 {
        return 0;
    }

    let mut tick = get_int_prop(player, instance_ref, "pAnimTick").unwrap_or(0) + 1;
    let mut frame = get_int_prop(player, instance_ref, "pAnimFrame").unwrap_or(0);
    if tick >= profile.frame_duration {
        tick = 0;
        frame = (frame + 1) % profile.frame_count as i32;
        set_prop(player, instance_ref, "pAnimFrame", Datum::Int(frame));
    }
    set_prop(player, instance_ref, "pAnimTick", Datum::Int(tick));

    frame.max(0) as usize
}

fn build_actor_frame_member(
    player: &mut DirPlayer,
    profile: &ActorProfile,
) -> Option<CastMemberRef> {
    let Some(source_ref) = resolve_member(player, profile.member_candidates) else {
        return None;
    };
    if profile.frame_count <= 1 {
        return Some(source_ref);
    }

    let source_name = player
        .movie
        .cast_manager
        .find_member_by_ref(&source_ref)
        .map(|member| member.name.clone())
        .unwrap_or_else(|| "actor".to_string());

    let frame_name = format!("__legacy_actor_{}_display", source_name);
    if let Some(frame_ref) = player
        .movie
        .cast_manager
        .find_member_ref_by_name(&frame_name)
    {
        return Some(frame_ref);
    }

    let frame_ref =
        create_actor_frame_member(player, &source_ref, &frame_name, 0, profile.frame_count)?;
    player.movie.cast_manager.invalidate_member_name_cache();
    Some(frame_ref)
}

fn get_actor_frame_member(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
) -> Option<CastMemberRef> {
    let frame_member_ref = player
        .allocator
        .get_script_instance(instance_ref)
        .properties
        .get(CiStr::new("pFrameMember"))
        .cloned()?;

    match player.get_datum(&frame_member_ref) {
        Datum::CastMember(member_ref) => Some(member_ref.clone()),
        _ => None,
    }
}

fn update_actor_frame_member(
    player: &mut DirPlayer,
    profile: &ActorProfile,
    frame_ref: &CastMemberRef,
    frame: usize,
) {
    if profile.frame_count <= 1 {
        return;
    }

    let Some(source_ref) = resolve_member(player, profile.member_candidates) else {
        return;
    };
    let Some(source_bitmap_ref) = player
        .movie
        .cast_manager
        .find_member_by_ref(&source_ref)
        .and_then(|member| match &member.member_type {
            CastMemberType::Bitmap(bitmap_member) => Some(bitmap_member.image_ref),
            _ => None,
        })
    else {
        return;
    };
    let Some(frame_bitmap) = player
        .bitmap_manager
        .get_bitmap(source_bitmap_ref)
        .and_then(|bitmap| crop_actor_frame(bitmap, frame, profile.frame_count))
    else {
        return;
    };
    let frame_bg_color = bitmap_background_color(&frame_bitmap);
    let Some(frame_bitmap_ref) = player
        .movie
        .cast_manager
        .find_member_by_ref(frame_ref)
        .and_then(|member| match &member.member_type {
            CastMemberType::Bitmap(bitmap_member) => Some(bitmap_member.image_ref),
            _ => None,
        })
    else {
        return;
    };

    player
        .bitmap_manager
        .replace_bitmap(frame_bitmap_ref, frame_bitmap);
    if let Some(frame_member) = player.movie.cast_manager.find_mut_member_by_ref(frame_ref) {
        if let Some(frame_bg_color) = frame_bg_color {
            frame_member.bg_color = frame_bg_color;
        }
    }
    player
        .movie
        .cast_manager
        .queue_texture_invalidation(frame_ref.clone());
    player.movie.score.invalidate_render_channel_cache();
    player.stage_dirty = true;
}

fn create_actor_frame_member(
    player: &mut DirPlayer,
    source_ref: &CastMemberRef,
    frame_name: &str,
    frame: usize,
    frame_count: usize,
) -> Option<CastMemberRef> {
    let (source_member, source_bitmap_member) = {
        let source_member = player.movie.cast_manager.find_member_by_ref(source_ref)?;
        let CastMemberType::Bitmap(bitmap_member) = &source_member.member_type else {
            return None;
        };
        (source_member.clone(), bitmap_member.clone())
    };
    let source_bitmap = player
        .bitmap_manager
        .get_bitmap(source_bitmap_member.image_ref)?
        .clone();
    let frame_bitmap = crop_actor_frame(&source_bitmap, frame, frame_count)?;
    let bitmap_ref = player.bitmap_manager.add_bitmap(frame_bitmap.clone());

    let mut info = source_bitmap_member.info.clone();
    info.width = frame_bitmap.width;
    info.height = frame_bitmap.height;
    info.reg_x = (frame_bitmap.width / 2) as i16;
    info.reg_y = source_bitmap_member
        .info
        .reg_y
        .clamp(0, frame_bitmap.height as i16);
    let reg_point = (info.reg_x, info.reg_y);
    let frame_bg_color = bitmap_background_color(&frame_bitmap).unwrap_or(source_member.bg_color);

    let cast = &mut player.movie.cast_manager.casts[0];
    let member_number = cast.first_free_member_id();
    let member_ref = CastMemberRef {
        cast_lib: cast.number as i32,
        cast_member: member_number as i32,
    };
    let member = CastMember {
        number: member_number,
        name: frame_name.to_string(),
        comments: source_member.comments,
        member_type: CastMemberType::Bitmap(BitmapMember {
            image_ref: bitmap_ref,
            reg_point,
            script_id: source_bitmap_member.script_id,
            member_script_ref: source_bitmap_member.member_script_ref,
            info,
        }),
        color: source_member.color,
        bg_color: frame_bg_color,
        reg_point: (reg_point.0 as i32, reg_point.1 as i32),
    };
    cast.members.insert(member_number, member);
    Some(member_ref)
}

fn crop_actor_frame(source: &Bitmap, frame: usize, frame_count: usize) -> Option<Bitmap> {
    if frame_count == 0 || source.width == 0 || source.height == 0 {
        return None;
    }
    let bytes_per_pixel = (source.bit_depth as usize).checked_div(8)?;
    if bytes_per_pixel == 0 {
        return None;
    }
    let height = source.height as usize;
    let declared_row_stride = (source.width as usize).checked_mul(bytes_per_pixel)?;
    let row_stride = if declared_row_stride.checked_mul(height)? <= source.data.len() {
        declared_row_stride
    } else {
        let inferred_stride = source.data.len() / height;
        inferred_stride - (inferred_stride % bytes_per_pixel)
    };
    let source_width = row_stride / bytes_per_pixel;
    let frame_width = source_width / frame_count;
    if frame_width == 0 {
        return None;
    }
    let left = frame_width * (frame % frame_count);
    if left + frame_width > source_width {
        return None;
    }

    let frame_row_bytes = frame_width.checked_mul(bytes_per_pixel)?;
    let mut data = Vec::with_capacity(frame_row_bytes.checked_mul(height)?);
    let mut copied_rows = 0usize;

    for y in 0..height {
        let source_start = y
            .checked_mul(row_stride)?
            .checked_add(left.checked_mul(bytes_per_pixel)?)?;
        let source_end = source_start.checked_add(frame_row_bytes)?;
        if source_end > source.data.len() {
            break;
        }
        data.extend_from_slice(&source.data[source_start..source_end]);
        copied_rows += 1;
    }

    if copied_rows == 0 {
        return None;
    }

    Some(Bitmap {
        width: frame_width as u16,
        height: copied_rows as u16,
        bit_depth: source.bit_depth,
        original_bit_depth: source.original_bit_depth,
        data,
        palette_ref: source.palette_ref.clone(),
        matte: None,
        use_alpha: source.use_alpha,
        trim_white_space: source.trim_white_space,
        was_trimmed: source.was_trimmed,
        version: 0,
    })
}

fn start_actor_conversation(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    profile: &ActorProfile,
) {
    let Some(quest_bubble_ref) = get_instance_prop_ref(player, "gQuest", "pQuestBubble") else {
        return;
    };

    let me_ref = player.alloc_datum(Datum::ScriptInstanceRef(instance_ref.clone()));
    let conv_ref = build_conversation(player, profile);
    let actions_ref = player.alloc_datum(list());
    let payload_ref = prop_list(
        player,
        vec![("pMe", me_ref), ("pConv", conv_ref), ("pAc", actions_ref)],
    );
    let int_id_ref = player.alloc_datum(Datum::String(profile.display_name.to_string()));
    let args = vec![payload_ref, int_id_ref];

    spawn_local(async move {
        if let Err(err) = player_call_datum_handler(&quest_bubble_ref, "startConv", &args).await {
            web_sys::console::warn_1(
                &format!("Legacy actor conversation failed: {:?}", err).into(),
            );
        }
    });
}

fn build_conversation(player: &mut DirPlayer, profile: &ActorProfile) -> DatumRef {
    let bubbles = profile
        .conversation_lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let id_ref = player.alloc_datum(Datum::Int((idx + 1) as i32));
            let type_ref = player.alloc_datum(Datum::Int(0));
            let content_ref = player.alloc_datum(Datum::String((*line).to_string()));
            let child_bubbles_ref = player.alloc_datum(list());
            let actions_ref = player.alloc_datum(list());
            let options_ref = player.alloc_datum(list());
            prop_list(
                player,
                vec![
                    ("id", id_ref),
                    ("type", type_ref),
                    ("cont", content_ref),
                    ("bub", child_bubbles_ref),
                    ("ac", actions_ref),
                    ("copt", options_ref),
                ],
            )
        })
        .collect();

    let bubbles_ref = player.alloc_datum(Datum::List(DatumType::List, bubbles, false));
    let type_ref = player.alloc_datum(Datum::Int(0));
    let title_ref = player.alloc_datum(Datum::String(profile.conversation_title.to_string()));
    let actions_ref = player.alloc_datum(list());
    prop_list(
        player,
        vec![
            ("type", type_ref),
            ("title", title_ref),
            ("bub", bubbles_ref),
            ("ac", actions_ref),
        ],
    )
}

fn prop_list(player: &mut DirPlayer, props: Vec<(&str, DatumRef)>) -> DatumRef {
    let entries = props
        .into_iter()
        .map(|(key, value)| {
            let key_ref = player.alloc_datum(Datum::Symbol(key.to_string()));
            (key_ref, value)
        })
        .collect();
    player.alloc_datum(Datum::PropList(entries, false))
}

fn resolve_member(player: &DirPlayer, candidates: &[&str]) -> Option<CastMemberRef> {
    candidates
        .iter()
        .find_map(|name| player.movie.cast_manager.find_member_ref_by_name(name))
}

fn member_size(player: &DirPlayer, member_ref: &CastMemberRef) -> Option<(i32, i32)> {
    player
        .movie
        .cast_manager
        .find_member_by_ref(member_ref)
        .and_then(|member| match &member.member_type {
            CastMemberType::Bitmap(bitmap) => {
                Some((bitmap.info.width as i32, bitmap.info.height as i32))
            }
            CastMemberType::Shape(shape) => Some((
                shape.shape_info.width() as i32,
                shape.shape_info.height() as i32,
            )),
            CastMemberType::VectorShape(vector) => {
                Some((vector.width().ceil() as i32, vector.height().ceil() as i32))
            }
            CastMemberType::Flash(flash) => {
                let (left, top, right, bottom) = flash.effective_rect();
                Some(((right - left) as i32, (bottom - top) as i32))
            }
            _ => None,
        })
}

fn member_background_color(player: &DirPlayer, member_ref: &CastMemberRef) -> Option<ColorRef> {
    let member = player.movie.cast_manager.find_member_by_ref(member_ref)?;
    let CastMemberType::Bitmap(bitmap_member) = &member.member_type else {
        return None;
    };
    let bitmap = player.bitmap_manager.get_bitmap(bitmap_member.image_ref)?;
    bitmap_background_color(bitmap)
}

fn bitmap_background_color(bitmap: &Bitmap) -> Option<ColorRef> {
    if bitmap.width == 0 || bitmap.height == 0 {
        return None;
    }

    let bytes_per_pixel = (bitmap.bit_depth as usize).checked_div(8)?;
    if bytes_per_pixel == 0 || bitmap.data.len() < bytes_per_pixel {
        return None;
    }

    Some(bitmap.get_pixel_color_ref(0, 0))
}

fn screen_loc(player: &DirPlayer, world_x: f64, world_y: f64) -> (i32, i32) {
    let (cam_x, cam_y) = player
        .globals
        .get("camLoc")
        .and_then(|datum_ref| match player.get_datum(datum_ref) {
            Datum::Point(values, _) => Some((values[0], values[1])),
            _ => None,
        })
        .unwrap_or((0.0, 0.0));
    ((world_x - cam_x) as i32, (world_y - cam_y) as i32)
}

fn z_for_world_y(world_y: f64) -> i32 {
    (100 + (world_y as i32 / 2)).clamp(100, 800)
}

fn append_object_list_entry(
    player: &mut DirPlayer,
    list_prop: &str,
    sprite_pos: i32,
    instance_ref: &ScriptInstanceRef,
) {
    let Some(gmain_ref) = get_global_instance_ref(player, "gMain") else {
        return;
    };
    let mut items = get_instance_list(player, &gmain_ref, list_prop);
    items.retain(|entry_ref| !entry_has_sprite(player, entry_ref, sprite_pos));

    let pnr_key = player.alloc_datum(Datum::Symbol("pNr".to_string()));
    let pnr_value = player.alloc_datum(Datum::Int(sprite_pos));
    let pobj_key = player.alloc_datum(Datum::Symbol("pObj".to_string()));
    let pobj_value = player.alloc_datum(Datum::ScriptInstanceRef(instance_ref.clone()));
    let entry = player.alloc_datum(Datum::PropList(
        VecDeque::from([(pnr_key, pnr_value), (pobj_key, pobj_value)]),
        false,
    ));
    items.push_back(entry);
    let list_ref = player.alloc_datum(Datum::List(DatumType::List, items, false));
    set_instance_prop_ref(player, &gmain_ref, list_prop, list_ref);
}

fn remove_object_list_entry(player: &mut DirPlayer, list_prop: &str, sprite_pos: i32) {
    let Some(gmain_ref) = get_global_instance_ref(player, "gMain") else {
        return;
    };
    let mut items = get_instance_list(player, &gmain_ref, list_prop);
    let old_len = items.len();
    items.retain(|entry_ref| !entry_has_sprite(player, entry_ref, sprite_pos));
    if items.len() != old_len {
        let list_ref = player.alloc_datum(Datum::List(DatumType::List, items, false));
        set_instance_prop_ref(player, &gmain_ref, list_prop, list_ref);
    }
}

fn clear_online_actor_slot(player: &mut DirPlayer, online_pos: i32) {
    if online_pos <= 0 {
        return;
    }
    set_global_list_item(player, "gUserList", online_pos as usize, Datum::Int(0));
    set_global_list_item(player, "gOnline", online_pos as usize, Datum::Int(0));
}

fn set_global_list_item(player: &mut DirPlayer, global_name: &str, index: usize, value: Datum) {
    let Some(list_ref) = player.globals.get(global_name).cloned() else {
        return;
    };
    let value_ref = player.alloc_datum(value);
    let Ok((_, items, _)) = player.get_datum_mut(&list_ref).to_list_mut() else {
        return;
    };
    if let Some(slot) = items.get_mut(index.saturating_sub(1)) {
        *slot = value_ref;
    }
}

fn get_global_instance_ref(player: &mut DirPlayer, global_name: &str) -> Option<ScriptInstanceRef> {
    let global_ref = player.globals.get(global_name).cloned()?;
    match player.get_datum(&global_ref) {
        Datum::ScriptInstanceRef(instance_ref) => Some(instance_ref.clone()),
        _ => None,
    }
}

fn get_instance_prop_ref(
    player: &mut DirPlayer,
    global_name: &str,
    prop_name: &str,
) -> Option<DatumRef> {
    let instance_ref = get_global_instance_ref(player, global_name)?;
    player
        .allocator
        .get_script_instance(&instance_ref)
        .properties
        .get(CiStr::new(prop_name))
        .cloned()
}

fn get_instance_list(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    prop_name: &str,
) -> VecDeque<DatumRef> {
    let Some(list_ref) = player
        .allocator
        .get_script_instance(instance_ref)
        .properties
        .get(CiStr::new(prop_name))
        .cloned()
    else {
        return VecDeque::new();
    };

    match player.get_datum(&list_ref) {
        Datum::List(_, items, _) => items.clone(),
        _ => VecDeque::new(),
    }
}

fn entry_has_sprite(player: &DirPlayer, entry_ref: &DatumRef, sprite_pos: i32) -> bool {
    let Datum::PropList(props, _) = player.get_datum(entry_ref) else {
        return false;
    };
    prop_value(props, player, "pNr")
        .and_then(|value_ref| player.get_datum(&value_ref).int_value().ok())
        == Some(sprite_pos)
}

fn prop_value(
    props: &VecDeque<PropListPair>,
    player: &DirPlayer,
    key_name: &str,
) -> Option<DatumRef> {
    props
        .iter()
        .find_map(|(key_ref, value_ref)| match player.get_datum(key_ref) {
            Datum::Symbol(symbol) | Datum::String(symbol)
                if symbol.eq_ignore_ascii_case(key_name) =>
            {
                Some(value_ref.clone())
            }
            _ => None,
        })
}

fn set_instance_prop_ref(
    player: &mut DirPlayer,
    instance_ref: &ScriptInstanceRef,
    prop_name: &str,
    value_ref: DatumRef,
) {
    player
        .allocator
        .get_script_instance_mut(instance_ref)
        .properties
        .insert(CiString::from(prop_name), value_ref);
}

fn set_cursor_version(player: &mut DirPlayer, version: &str) {
    let Some(gmain_ref) = get_global_instance_ref(player, "gMain") else {
        return;
    };
    let Some(cursor_ref) = player
        .allocator
        .get_script_instance(&gmain_ref)
        .properties
        .get(CiStr::new("pcursor"))
        .cloned()
    else {
        return;
    };
    let Datum::ScriptInstanceRef(cursor_instance) = player.get_datum(&cursor_ref) else {
        return;
    };
    let cursor_instance = cursor_instance.clone();
    let value_ref = player.alloc_datum(Datum::Symbol(version.to_string()));
    set_instance_prop_ref(player, &cursor_instance, "pVer", value_ref);
}

fn point(x: f64, y: f64) -> Datum {
    Datum::Point([x, y], 0)
}

fn list() -> Datum {
    Datum::List(DatumType::List, VecDeque::new(), false)
}
