//! Crossings upstream wrote once, for one place. Each is matched against its
//! script **verbatim** (`src/upstream_scripts/`), and ported by hand beside
//! it: an edit upstream un-ports the exit, and the ratchet says so.

use cena_map::{Action, Cond, Crossing, Routine, Step};

use super::{always, holes, is_word};

pub(super) fn crossing(script: &str) -> Option<Crossing> {
    verbatim(script)
        .or_else(|| guild_door(script))
        .or_else(|| wandering_way(script))
}

fn when(action: Action, cond: Cond) -> Step {
    Step {
        action,
        when: Some(cond),
    }
}

fn put(command: &str) -> Step {
    always(Action::Put(command.to_owned()))
}

fn go(command: &str) -> Step {
    always(Action::Move(command.to_owned()))
}

fn not(cond: Cond) -> Cond {
    Cond::Not(Box::new(cond))
}

fn search_until(said: &str) -> Step {
    always(Action::PutUntil {
        command: "search".to_owned(),
        until: vec![said.to_owned()],
        tries: None,
    })
}

fn standing() -> Cond {
    Cond::Posture("standing".to_owned())
}

fn verbatim(script: &str) -> Option<Crossing> {
    const MIRROR: &str = include_str!("../upstream_scripts/mirror.rb");
    const RING_WEDGES: &str = include_str!("../upstream_scripts/ring_wedges.rb");
    const STAIN: &str = include_str!("../upstream_scripts/stain.rb");
    const TOMB: &str = include_str!("../upstream_scripts/tomb.rb");
    const LOCKPICK_SHED: &str = include_str!("../upstream_scripts/lockpick_shed.rb");
    const HEAVY_KEY: &str = include_str!("../upstream_scripts/heavy_key.rb");
    const TRAIL: &str = include_str!("../upstream_scripts/trail.rb");
    const CAB: &str = include_str!("../upstream_scripts/cab.rb");
    const ROPE_BRIDGE: &str = include_str!("../upstream_scripts/rope_bridge.rb");
    const IVY_BUILDING: &str = include_str!("../upstream_scripts/ivy_building.rb");
    let steps = match script {
        MIRROR => return Some(Crossing::Routine(Routine::Mirror)),
        RING_WEDGES => return Some(Crossing::Routine(Routine::RingWedges)),
        // Wait for the stain, take it, and crawl: down a slope that a search
        // finds, and through an opening that another does.
        STAIN => vec![
            always(Action::Await("dark stains".to_owned())),
            put("get stain"),
            when(Action::Put("lie".to_owned()), standing()),
            search_until("descending slope"),
            always(Action::TryMove("go slope".to_owned())),
            search_until("opening"),
            go("go opening"),
            when(Action::Put("stand".to_owned()), not(standing())),
        ],
        // Sleep in the tomb, wake somewhere else, and wait out the stun.
        TOMB => vec![
            put("lie tomb"),
            put("sleep"),
            always(Action::AwaitArrival),
            always(Action::WaitUntil(Cond::Otherwise(Box::new(Cond::Flag(
                "stunned".to_owned(),
            ))))),
            when(Action::Put("stand".to_owned()), not(standing())),
        ],
        LOCKPICK_SHED => vec![
            always(Action::TakeOut("lockpick".to_owned())),
            put("pick shed"),
            always(Action::PutBack),
            put("open shed"),
            go("go shed"),
        ],
        // Upstream tells a walker without the key to do the puzzle on the
        // table instead. Whether a key is in some container cannot be priced,
        // so here the crossing fails at its first step.
        HEAVY_KEY => vec![
            always(Action::TakeOut("heavy key".to_owned())),
            put("unlock spiked gate with my heavy key"),
            always(Action::PutBack),
            go("go spiked gate"),
        ],
        // The trail's way on changes; looking at it from the boulder says.
        TRAIL => vec![
            go("climb boulder"),
            always(Action::Ask(
                "look trail".to_owned(),
                "the trail heads off to the ".to_owned(),
            )),
            go("down"),
            go("{told}"),
        ],
        // The cab comes down the mountain once the dam is shut: about four
        // and a half minutes, which the exit's cost says.
        CAB => {
            let gone = || not(Cond::Sees("wooden cab".to_owned()));
            vec![
                always(Action::EmptyHands),
                when(Action::Put("close dam".to_owned()), gone()),
                when(Action::Await("riding on the trellis".to_owned()), gone()),
                put("open dam"),
                always(Action::FillHands),
                go("go cab"),
            ]
        }
        // The small folk jump for the rope and everyone else pulls it, until
        // the bridge is down. It may not land where the map says.
        ROPE_BRIDGE => {
            let small = || {
                Cond::Any(
                    ["Halfling", "Dwarf", "Gnome"]
                        .map(|race| Cond::Race(race.to_owned()))
                        .to_vec(),
                )
            };
            let until = [
                "give it a good yank!",
                "since it is underneath",
                "what were you referring to",
                "that was fun.",
            ]
            .map(str::to_owned)
            .to_vec();
            let work = |command: &str| Action::PutUntil {
                command: command.to_owned(),
                until: until.clone(),
                tries: None,
            };
            vec![
                when(work("jump"), small()),
                when(work("pull rope"), Cond::Otherwise(Box::new(small()))),
                go("go drawbridge"),
                always(Action::Replan),
            ]
        }
        IVY_BUILDING => vec![go(
            "go {item:ivy-covered grey stone building with a sturdy cross-gabled roof}",
        )],
        _ => return None,
    };
    Some(Crossing::Steps(steps))
}

/// The wizard guild's doors hear their password only in Guildspeak, and not
/// from someone unseen. 2 exits.
fn guild_door(script: &str) -> Option<Crossing> {
    let [door] = holes(
        script,
        &[
            ";e fput 'speak'; language = /You are currently speaking (.*?)\\./.match(get).captures\
             .first until language;; fput('speak wizard') unless language == 'Guildspeak'; \
             fput('unhide') if hidden? or invisible?; move 'say ::",
            " wizard'; fput('speak ' + language.to_s) unless language == 'Guildspeak'",
        ],
    )?[..] else {
        return None;
    };
    is_word(door).then_some(())?;
    Some(Crossing::Steps(vec![
        always(Action::Speak("wizard".to_owned())),
        when(
            Action::Put("unhide".to_owned()),
            Cond::Any(vec![
                Cond::Flag("hidden".to_owned()),
                Cond::Flag("invisible".to_owned()),
            ]),
        ),
        always(Action::Move(format!("say ::{door} wizard"))),
        always(Action::RestoreSpeech),
    ]))
}

/// A way out that turns up in one of many rooms: visit each until it shows.
/// 2 exits, each with its own list.
fn wandering_way(script: &str) -> Option<Crossing> {
    if let Some(found) = holes(
        script,
        &[
            ";e bleak_rooms = [",
            "]\n;bleak_rooms.map! { |r| Room[\"u#{r}\"].id }\n;bleak_rooms.each { |r|\n;  \
             options = {'force': true}\n;  Script.run('go2', \"#{r}\", options) unless \
             Room.current.id == r\n;  if GameObj.loot.any? { |o| o.name == 'rippling ethereal \
             green portal' }\n;    break\n;  end\n;}\n;move(\"go ethereal portal\")\n;$go2_restart \
             = true",
        ],
    ) {
        let [rooms] = found[..] else {
            return None;
        };
        return Some(Crossing::Routine(Routine::SearchRooms {
            rooms: numbers(rooms, ", ")?,
            by_uid: true,
            sees: "rippling ethereal green portal".to_owned(),
            enter: "go ethereal portal".to_owned(),
        }));
    }
    let [rooms] = holes(
        script,
        &[
            ";e %w(",
            ").each{|id| force_start_script('go2', [id]);wait_while{Script.running.count{|s| \
             s.name == 'go2'} == 2};break if GameObj.loot.collect{|i| \
             i.noun}.include?('doorframe')};move 'go doorframe'",
        ],
    )?[..] else {
        return None;
    };
    Some(Crossing::Routine(Routine::SearchRooms {
        rooms: numbers(rooms, " ")?,
        by_uid: false,
        sees: "doorframe".to_owned(),
        enter: "go doorframe".to_owned(),
    }))
}

fn numbers(list: &str, between: &str) -> Option<Vec<u32>> {
    list.split(between).map(|n| n.parse().ok()).collect()
}

#[cfg(test)]
mod tests {
    use cena_map::moves_whatever_is_known;

    use super::*;

    #[test]
    fn every_verbatim_script_is_ported_and_moves_the_walker() {
        for script in [
            include_str!("../upstream_scripts/stain.rb"),
            include_str!("../upstream_scripts/tomb.rb"),
            include_str!("../upstream_scripts/lockpick_shed.rb"),
            include_str!("../upstream_scripts/heavy_key.rb"),
            include_str!("../upstream_scripts/trail.rb"),
            include_str!("../upstream_scripts/cab.rb"),
            include_str!("../upstream_scripts/rope_bridge.rb"),
            include_str!("../upstream_scripts/ivy_building.rb"),
        ] {
            let Some(Crossing::Steps(steps)) = crossing(script) else {
                panic!("unported: {script}");
            };
            assert!(moves_whatever_is_known(&steps), "{script}");
            // One character's difference is a different script.
            assert_eq!(crossing(&format!("{script} ")), None);
        }
        assert_eq!(
            crossing(include_str!("../upstream_scripts/mirror.rb")),
            Some(Crossing::Routine(Routine::Mirror))
        );
    }

    #[test]
    fn the_wandering_ways_carry_their_own_room_lists() {
        let Some(Crossing::Routine(Routine::SearchRooms { rooms, by_uid, .. })) =
            crossing(include_str!("../upstream_scripts/bleak_portal.rb"))
        else {
            panic!("a search");
        };
        assert_eq!((rooms.len(), rooms[0], by_uid), (28, 474_204, true));
        let Some(Crossing::Routine(Routine::SearchRooms { rooms, by_uid, .. })) =
            crossing(include_str!("../upstream_scripts/doorframe.rb"))
        else {
            panic!("a search");
        };
        assert_eq!((rooms.len(), rooms[82], by_uid), (83, 21_386, false));
    }

    #[test]
    fn the_guild_door_is_spoken_to_in_its_own_tongue() {
        let script = ";e fput 'speak'; language = /You are currently speaking \
                      (.*?)\\./.match(get).captures.first until language;; fput('speak wizard') \
                      unless language == 'Guildspeak'; fput('unhide') if hidden? or invisible?; \
                      move 'say ::portal wizard'; fput('speak ' + language.to_s) unless language \
                      == 'Guildspeak'";
        let Some(Crossing::Steps(steps)) = crossing(script) else {
            panic!("steps");
        };
        assert_eq!(steps[0].action, Action::Speak("wizard".into()));
        assert_eq!(steps[2].action, Action::Move("say ::portal wizard".into()));
        assert_eq!(steps[3].action, Action::RestoreSpeech);
    }
}
