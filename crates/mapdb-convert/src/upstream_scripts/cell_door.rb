;e if !GameObj.loot.find{|o| o.name =~ /ruined cell door/}.nil?;
  move ('go door');
elsif GameObj.right_hand.id.nil?;
  echo 'You need a weapon to BATTER the DOOR';
else
  echo 'BATTERing the DOOR with a valid weapon (even if not a warrior) until it is destroyed and becomes a ruined cell door';
  batter_result = nil;
  until batter_result =~ /ineffective|destroyed/;
    batter_result = dothistimeout 'batter door', 3, /You bash at the|The cell door has already been destroyed.|Your .*? is ineffective against the door|going to do much good/;
    break if batter_result =~ /ineffective/;
  end;
  move ('go door') if batter_result =~ /destroyed/;
  echo '  You need a different weapon to BATTER the DOOR' if batter_result =~ /ineffective|going to do much good/;
end;
