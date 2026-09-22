//! Arms whose crossing is a named search (`cena_map::routine`).

use cena_map::{Crossing, Landmark, Opening, RoomId, Routine};

use super::moves::quoted_list;
use super::{holes, is_plain_argument, is_word};

/// Every exit of the Confluence, 3,233 of them, is two statements: name a
/// goal, then call the one script that holds the search
/// (`Room[23282].wayto['23282']`, 4 KB, itself an exit from a room to itself
/// and never routed). The goal is the exit's own destination, or the word
/// `tranquility` for the exits that lead out of the plane.
pub(super) fn confluence(script: &str, to: u32) -> Option<Crossing> {
    let [goal] = holes(
        script,
        &[
            ";e $mapdb_confluence_target = ",
            "; Room[23282].wayto['23282'].call",
        ],
    )?[..] else {
        return None;
    };
    let leave = match goal {
        "'tranquility'" => true,
        room if room.parse() == Ok(to) => false,
        _ => return None,
    };
    Some(Crossing::Routine(Routine::Confluence { leave }))
}

/// The minotaur maze: 497 exits whose whole configuration is a goal and a set
/// of rooms, followed by 1.2 KB of search that is **the same text on every
/// one** -- kept verbatim beside this file, so a change to it upstream stops
/// the arm matching like any other.
pub(super) fn minotaur_maze(script: &str, to: u32) -> Option<Crossing> {
    const SEARCH: &str = include_str!("../upstream_scripts/minotaur_maze.rb");
    let [target, rooms] = holes(
        script,
        &[
            ";e target_room_id = ",
            "; maze_rooms = [",
            &format!("]; {SEARCH}"),
        ],
    )?[..] else {
        return None;
    };
    (target.parse() == Ok(to)).then_some(())?;
    let rooms: Vec<RoomId> = rooms
        .split(',')
        .map(|room| room.trim().parse().map(RoomId))
        .collect::<Result<_, _>>()
        .ok()?;
    (!rooms.is_empty()).then_some(())?;
    Some(Crossing::Routine(Routine::MinotaurMaze { rooms }))
}

/// The Rift's ways out: 570 exits in five spellings of one script. Two tables
/// -- where each room sits on a circuit, and the direction to take from each
/// position -- then "go round until you see X", then go through it.
///
/// Upstream's own `else; echo 'error: ...'` (the walker is not on the circuit)
/// and its closing `$go2_restart = true` are both what [`Routine::Patrol`]
/// means, so neither is an argument.
pub(super) fn patrol(script: &str) -> Option<Crossing> {
    const START: &str = ";e start_room = [";
    const DIRS: &str = "]; dirs = [";
    const WALK: &str =
        "]; if index = start_room.index(Room.current.id); until checkloot.include?('";
    const ROUND: &str = "; move dirs[index]; index += 1; index = 0 if index >= dirs.length; end; ";
    const LOST: &str = "else; echo 'error: mini-script expected a different room'; end; \
                        $go2_restart = true";
    const PUSHED: &str = "A wide fissure cannot be opened any farther.";

    let [starts, dirs, seen, through] = holes(script, &[START, DIRS, WALK, ROUND, LOST])?[..]
    else {
        return None;
    };
    let starts = starts
        .split(',')
        .map(|room| match room.trim() {
            "nil" => Some(None),
            id => id.parse().ok().map(|id| Some(RoomId(id))),
        })
        .collect::<Option<Vec<_>>>()?;
    let dirs: Vec<String> = quoted_list(dirs)?.into_iter().map(str::to_owned).collect();
    (dirs.iter().all(|dir| is_word(dir)) && !dirs.is_empty()).then_some(())?;

    let plain = |noun: &str, enter: &str| {
        (is_word(noun) && is_plain_argument(enter)).then(|| Landmark {
            noun: noun.to_owned(),
            enter: enter.to_owned(),
            open: None,
        })
    };
    // What is looked for is either one noun or two.
    let (landmarks, after) = if let Some((first, second)) = seen
        .strip_suffix("')")
        .and_then(|seen| seen.split_once("') or checkloot.include?('"))
    {
        let choose = format!("if checkloot.include?('{first}'); move '");
        let otherwise = format!("'; elsif checkloot.include?('{second}'); move '");
        let [one, two] = [("'; end;; "), ("'; end; ")]
            .iter()
            .find_map(|end| holes(through, &[&choose, &otherwise, end]))?[..]
        else {
            return None;
        };
        (vec![plain(first, one)?, plain(second, two)?], Vec::new())
    } else {
        let noun = seen.strip_suffix("')")?;
        if let Some(found) = holes(through, &["move '", "'; waitrt?; fput '", "'; "]) {
            let [enter, after] = found[..] else {
                return None;
            };
            is_plain_argument(after).then_some(())?;
            (vec![plain(noun, enter)?], vec![after.to_owned()])
        } else if let Some(found) = holes(through, &["move '", "'; "]) {
            (vec![plain(noun, found.first()?)?], Vec::new())
        } else {
            let pry = format!(
                "5.times {{ waitrt?; fput 'stand' unless standing?; waitrt?; result = \
                 dothistimeout 'push {noun}', 3, /^Grasping the distorted edges|^A wide \
                 {noun} cannot be opened any farther\\.|^As you move to touch a sealed \
                 {noun}|^What were you referring to\\?/; waitrt?; fput 'stand' unless \
                 standing?; waitrt?; break if result =~ /^A wide {noun} cannot be opened any \
                 farther\\./ }}; move '"
            );
            let found = holes(through, &[&pry, "'; "])?;
            (noun == "fissure").then_some(())?;
            let mut landmark = plain(noun, found.first()?)?;
            landmark.open = Some(Opening {
                command: format!("push {noun}"),
                until: PUSHED.to_owned(),
                tries: 5,
            });
            (vec![landmark], Vec::new())
        }
    };
    Some(Crossing::Routine(Routine::Patrol {
        starts,
        dirs,
        landmarks,
        after,
    }))
}

/// The underwater route: 75 exits, three tables, two spellings. The longer
/// spelling also waits for an escorted child to catch up after each stroke;
/// the walk's own pacing covers that, and the child is not an argument.
pub(super) fn signposts(script: &str, to: u32) -> Option<Crossing> {
    const HANDS: &str = ";e empty_hand if [";
    const TABLE: &str = "].include?(Room.current.id); swim_dir = {";
    const SWIM: &str = "; if swim_dir[Room.current.id]; put \"swim #{swim_dir[Room.current.id]}\"; \
        else; echo \"Oh crap.. I'm lost..\"; put \"swim #{checkpaths[rand(checkpaths.length)]}\"; \
        end; sleep 1; waitrt?; ";
    const ALONE: [&str; 5] = [
        HANDS,
        TABLE,
        "}; while Room.current.id != ",
        SWIM,
        "end; fill_hand",
    ];
    const ESCORTING: [&str; 5] = [
        HANDS,
        TABLE,
        "}; child = (bounty? =~ /^You have made contact with the child/) && \
         GameObj.npcs.find { |npc| npc.noun == 'child' }; while (Room.current.id != ",
        SWIM,
        "50.times { break if GameObj.npcs.any? { |npc| npc.id == child.id }; sleep 0.1 } \
         if child; end; fill_hand",
    ];
    let found = holes(script, &ALONE).or_else(|| holes(script, &ESCORTING))?;
    let [hands, table, target, between] = found[..] else {
        return None;
    };
    // The escorting spelling brackets its test: `!= 12662)`.
    let target = target.strip_suffix(')').unwrap_or(target);
    (target.parse() == Ok(to) && between.is_empty()).then_some(())?;

    let rooms = |list: &str| {
        list.split(',')
            .map(|room| room.trim().parse().ok().map(RoomId))
            .collect::<Option<Vec<_>>>()
    };
    let dirs = table
        .split(',')
        .map(|entry| {
            let (room, dir) = entry.split_once("=>")?;
            let dir = dir.trim().strip_prefix('\'')?.strip_suffix('\'')?;
            is_word(dir).then_some((RoomId(room.trim().parse().ok()?), dir.to_owned()))
        })
        .collect::<Option<Vec<_>>>()?;
    (!dirs.is_empty()).then_some(())?;
    Some(Crossing::Routine(Routine::Signposts {
        verb: "swim".to_owned(),
        dirs,
        hands_free_in: rooms(hands)?,
    }))
}

/// The scripts whose whole text is kept beside this file. Each is one exit --
/// a room to itself, or the first town -- that every other exit of its kind
/// calls; if upstream edits one, this stops matching and the ratchet says so.
const TRINKET: &str = include_str!("../upstream_scripts/fwi_trinket.rb");
const PASSWORD: &str = ";e if UserVars.rogue_password.nil? or UserVars.rogue_password.empty?; \
    echo 'No Rogue Guild password has been set.'; echo 'example:  ;vars set \
    rogue_password=kick, slap, turn, scratch, kick, slap'; exit; end; fput 'lean door'; \
    UserVars.rogue_password.split(/, */).each { |verb| fput \"#{verb} door\" }; fput 'go door'";

/// Voln's symbol of seeking: 36 exits that name a destination and call the
/// one script that seeks it. The destination is the exit's own.
///
/// That script also writes `redforest_location` when the destination is the
/// Red Forest (24715), by which of two rooms the walker sought from; the
/// converter knows the room, so the memory is settled here.
pub(super) fn seeking(script: &str, from: u32, to: u32) -> Option<Crossing> {
    const RED_FOREST: u32 = 24715;
    let [destination] = holes(
        script,
        &[
            ";e $mapdb_seeking_destination = ",
            ";Map[3600].wayto['3600'].call;",
        ],
    )?[..] else {
        return None;
    };
    (destination.parse() == Ok(to)).then_some(())?;
    let side = match (to, from) {
        (RED_FOREST, 3600) => Some("WL"),
        (RED_FOREST, 10125) => Some("EN"),
        _ => None,
    };
    Some(Crossing::Routine(Routine::Seeking {
        remember: side.map(|side| ("redforest_location".to_owned(), side.to_owned())),
    }))
}

/// The trinket (31 exits) and the rogue guild doors (9): the script itself,
/// or a call to it.
pub(super) fn called(script: &str) -> Option<Crossing> {
    const CALLS: [(&str, Routine); 3] = [
        (";e Map[7].wayto['3668'].call;", Routine::Trinket),
        (";e Map[7].wayto['3668'].call", Routine::Trinket),
        (
            ";e Map[12421].wayto['14089'].call; # rogue guild proc",
            Routine::GuildPassword,
        ),
    ];
    let routine = if script == TRINKET {
        Routine::Trinket
    } else if script == PASSWORD {
        Routine::GuildPassword
    } else {
        CALLS
            .iter()
            .find(|(call, _)| *call == script)
            .map(|(_, routine)| routine.clone())?
    };
    Some(Crossing::Routine(routine))
}
