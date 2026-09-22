;e script = Script.self
hand = sword = sword_container = nil;
close_containers = [];
find_all_containers_var = nil;
find_all_containers = proc {
	if find_all_containers_var.nil?
		restore_want_downstream = script.want_downstream
		restore_want_downstream_xml = script.want_downstream_xml
		script.want_downstream = false
		script.want_downstream_xml = true
		clear
		put 'inventory containers'
		timeout = Time.now + 10
		container_ids = []
		while (Time.now < timeout and line = get)
			if (line =~ /^You are wearing/)
				container_ids = line.scan(/<a exist="([^"]+)"/).flatten
				break
			end
		end
		script.want_downstream = restore_want_downstream
		script.want_downstream_xml = restore_want_downstream_xml
		clear
		find_all_containers_var = container_ids.collect { |id| GameObj[id] }
	end
	find_all_containers_var
}
ds_to_bag = proc { |item_id, container_id|
	restore_want_downstream = script.want_downstream
	restore_want_downstream_xml = script.want_downstream_xml
	script.want_downstream = false
	script.want_downstream_xml = true
	result = dothistimeout "_drag ##{item_id} ##{container_id}", 3, /<(?:right|left)>Empty<|^You can't .+ It's closed!|I could not find what you were referring to|Your .*? won't fit in/
	script.want_downstream = restore_want_downstream
	script.want_downstream_xml = restore_want_downstream_xml
	result
}
ds_to_hand = proc { |item_id,hand|
	if hand.nil?;
		hand = 'left' if GameObj.left_hand.id.nil?;
		hand = 'right' if GameObj.right_hand.id.nil?;
	end
	restore_want_downstream = script.want_downstream
	restore_want_downstream_xml = script.want_downstream_xml
	script.want_downstream = false
	script.want_downstream_xml = true
	result = dothistimeout "_drag ##{item_id} #{hand}", 3, /<(?:right|left) exist="#{item_id}"/
	script.want_downstream = restore_want_downstream
	script.want_downstream_xml = restore_want_downstream_xml
	result
}

stow_sword = proc {
	if sword.nil?
		if (GameObj.right_hand.noun == 'sword')
			sword = GameObj.right_hand
		elsif (GameObj.left_hand.noun == 'sword')
			sword = GameObj.left_hand
		end
	end
	if sword
		if sword_container
			ds_to_bag.call(sword.id, sword_container.id)
		else
  		ds_to_bag.call(sword.id, XMLData.stow_container_id)
			if (GameObj.right_hand.id == sword.id or GameObj.left_hand.id == sword.id)
				for container in find_all_containers.call
					next if (container.noun == 'locket')
					ds_to_bag.call(sword.id, container.id)
					break unless (GameObj.right_hand.id == sword.id or GameObj.left_hand.id == sword.id)
				end
			end
		end
	end
}
empty_hands
GameObj.containers.each{|k,v| if sword = v.find {|i| i.name == 'short sword'};sword_container = GameObj[k];break;end};
if sword.nil?
	for container in find_all_containers.call
		if container.contents.nil?
			result = dothistimeout "open ##{container.id}", 3, /^You open|^That is already open|^There doesn't seem to be any way to do that|^What were you referring to|^I could not find what you were referring to/
			if (container.contents.nil? and result =~ /^That is already open/)
				dothistimeout "look in ##{container.id}", 3, /^In the|^There is nothing|^That is closed|^What were you referring to|^I could not find what you were referring to/
			elsif result =~ /^You open/
				close_containers.push(container)
			end
		end
		if (sword = container.contents.find { |obj| obj.name == 'short sword' })
			sword_container = container
			break
		end
	end
end
if sword
	ds_to_hand.call(sword.id, 'right')
else
	if $go2_get_silvers
		this_room = Room.current
		if (Script.running.find_all { |s| s.name == 'go2' }.length > 1)
			echo 'Too many instances of go2 running. Killing other instances.'
			for s in Script.running
				s.kill if s.name == 'go2' and s != Script.self
			end
			exit
		end
		force_start_script 'go2', [ 'bank', '--disable-confirm' ]
		wait_while { Script.running.find_all { |s| s.name == 'go2' }.length > 1 }
		fput 'unhide' if hidden? or invisible?
		fput 'withdraw 200'
		force_start_script 'go2', [ 'weaponshop', '--disable-confirm' ]
		wait_while { Script.running.find_all { |s| s.name == 'go2' }.length > 1 }
		fput 'unhide' if hidden? or invisible?
		fput 'order 3'
		fput 'buy'
		force_start_script 'go2', [ 'bank', '--disable-confirm' ]
		wait_while { Script.running.find_all { |s| s.name == 'go2' }.length > 1 }
		fput 'unhide' if hidden? or invisible?
		fput 'deposit all'
		force_start_script 'go2', [ this_room.id.to_s ]
		wait_while { Script.running.find_all { |s| s.name == 'go2' }.length > 1 }
	else
		echo 'You have no short sword! Use	 ;go2 getsilvers=on	 if you want this script to buy one for you.'
		close_containers.each { |c| fput "close ##{c.id}" }
		fill_hands
		exit
	end
end
if (resolve = Spell[9704] and resolve.known? and resolve.affordable? and !resolve.active?)
	resolve.cast
end
killed_disarm = Script.kill('disarmed')
before_dying { Script.start('disarm') if killed_disarm }
recovering_item = false
loop do
	move 'go pool'
	fput 'swim down'
	fput 'kneel'
	sleep 0.5
	unless recovering_item
		line = dothistimeout 'pry gap', 3, /you are spit out|tumbles out of your hand|good enough leverage/
		if line =~ /tumbles out of your hand/
			recovering_item = true
			sword = nil
		end
		waitrt?
		break if Room.current.id != 22229
	end
	if recovering_item
		line = dothistimeout 'recover item', 3, /nothing recoverable|and recover it/
		recovering_item = false if line =~ /and recover it/
		waitrt?
	end
	fput 'swim up'
	fput 'go shore'
end
fput 'stand' until standing?
waitrt?
stow_sword.call
close_containers.each { |c| fput "close ##{c.id}" }
fill_hands
Script.start('disarm') if killed_disarm
undo_before_dying
$go2_restart = true;
