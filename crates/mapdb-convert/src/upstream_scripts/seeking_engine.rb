;e need_set_roomname_off = false

if last_roomdesc = $_SERVERBUFFER_.reverse.find { |line| line =~ /<resource picture="\d+"\/><style id="roomName" ?\/\>\[[^\]]+\]/ || line =~ /You will no longer see room names./ }
  if last_roomdesc =~ /You will no longer see room names./
    fput "set roomname on"
    need_set_roomname_off = true
  end
end

save_want_downstream = Script.current.want_downstream
save_want_downstream_xml = Script.current.want_downstream_xml
Script.current.want_downstream = false
Script.current.want_downstream_xml = true

destination = Map[$mapdb_seeking_destination]
roomname_pattern = /<style id="roomName" \/>(.*?)$/
failed_to_find_destination = false
first_roomname = nil
seeking_destination = nil

20.times do
  waitcastrt?
  put "symbol of seeking"
  matchtimeout(6, "Your vision is pulled away from you...")

  result = matchtimeout(6, roomname_pattern)
  next unless result

  seeking_destination = result.match(roomname_pattern)&.captures&.first
  next unless seeking_destination

  if seeking_destination[0] == "<"
    need_set_roomname_off = true
    fput "flag roomname on"
    next
  end

  # Remove trailing numeric room ID in parentheses, if any
  if seeking_destination =~ /( \(\d+\))$/
    seeking_destination.slice!($1)
  end
  
  if first_roomname == seeking_destination
    failed_to_find_destination = true
    break
  end

  first_roomname ||= seeking_destination
  break if destination.title.include?(seeking_destination)
end

Script.current.want_downstream_xml = save_want_downstream_xml
Script.current.want_downstream = save_want_downstream
fput "set roomname off" if need_set_roomname_off

if failed_to_find_destination
  echo "** seeking destination #{$mapdb_seeking_destination} #{destination.title.first.inspect} not found."
  $restart_go2 = false
  Script.current.kill
end

if $mapdb_seeking_destination == 24715
  UserVars.mapdb_redforest_location = "WL" if Map.current.id == 3600
  UserVars.mapdb_redforest_location = "EN" if Map.current.id == 10125
end

dothistimeout("symbol of seeking confirm", 6, /^Your surroundings blur into a white fog/)
