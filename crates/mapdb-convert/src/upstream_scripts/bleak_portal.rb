;e bleak_rooms = [474204, 474205, 474206, 474207, 474208, 474209, 474210, 474211, 474212, 474213, 474214, 474215, 474216, 474218, 474219, 474220, 474221, 474222, 474224, 474225, 474226, 474227, 474228, 474229, 474200, 474201, 474202, 474203]
;bleak_rooms.map! { |r| Room["u#{r}"].id }
;bleak_rooms.each { |r|
;  options = {'force': true}
;  Script.run('go2', "#{r}", options) unless Room.current.id == r
;  if GameObj.loot.any? { |o| o.name == 'rippling ethereal green portal' }
;    break
;  end
;}
;move("go ethereal portal")
;$go2_restart = true