;e 
flip_tags = !Script.current.want_downstream_xml;
status_tags if flip_tags;
go_result = dothistimeout 'go bridge',3,/which is presently pulled open|<nav rm/;
status_tags if flip_tags;
if go_result =~ /which is presently pulled open/;
  climb_platform = false;
  refill_hands = false;
  move('go opening');
  while climb_platform == false;
    waitrt?;
    result = dothistimeout "climb platform", 2, /You decide to climb back down|you were able to pull yourself up|I could not find what you were referring to|You figure freeing up both hands might help./;
    climb_platform = true if result =~ /you were able to pull yourself up|I could not find what you were referring to/;
    (refill_hands = true;empty_hands) if result =~ /You figure freeing up both hands might help./;
  end;
  fill_hands if refill_hands == true;
  buffed = false;
  1.times{
    turn_result = dothistimeout "turn wheel",3,/The wheel begins to move|you just don't have enough strength to budge it|already been turned and a locking bar is holding it in place./
    if turn_result =~ /The wheel begins to move|already been turned and a locking bar is holding it in place./
      move("down")
      move("out")
      move("go bridge")
    elsif buffed == false
      buffs = [606,509,9605]
      buffs.each{|num| Spell[num].cast if Spell[num].known? and Spell[num].affordable?}
      buffed = true
      redo
    else;
      echo "You don't have the strength to turn the wheel on your own.  Pausing here until you can get some help!"
      echo "`;u go2` to unpause the script once the wheel has been turned."
      pause_script
    end
  }
end
