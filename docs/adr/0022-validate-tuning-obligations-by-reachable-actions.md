# Validate Tuning Obligations By Reachable Actions

The Rules Engine authoritatively offers 調律 only when some legal sequence of
remaining non-action-ending abilities can reach a Profession Change or allowed
Profession Formation that uses the retrieved Card. While that obligation is
active, both action queries and command validation reject any voluntary choice
whose projected result removes every remaining completion path. This
reachability rule preserves legal post-調律 ability sequences while preventing
UI bypasses, stale clients, or direct commands from creating an unfinishable
turn.
