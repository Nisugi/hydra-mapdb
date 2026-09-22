;e 
touch = ["gold leaf","red leaf","blighted leaf","outstretched hand"];
fail_pattern = /^You shouldn't mess with that while (\w+) is working on it./;
success_pattern = /^You touch the/;
retry_pattern = /^The shimmering leaves on the wall begin to fade, blending once again into the mural.|form is quickly lost in a bright shroud of light as/;
patterns = Regexp.union(success_pattern,fail_pattern);
result = nil;
until success_pattern.match(result);
  result = dothistimeout "touch green leaf", 3, patterns;
  if fail_pattern.match(result);
    waitforpc = $1;
    echo "waiting for #{waitforpc} to finish";
    timeout = Time.now + 15;
    until (!retry_pattern.match(get) or Time.now > timeout);end;
  else;
    echo "unknown result trying to enter the Labyrinth";
    break;
  end;
end;
touch.each {|t| fput "touch #{t}"};
fput "recite Listen to the wind in the forest,;That which whispers the words of the lady of the tower.;She of old who is called Maaghara.";
timeout = Time.now + 20;
wait_while { Room.current.id == 9767 and Time.now < timeout}
