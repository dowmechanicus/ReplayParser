# The action data block has the following format:

Nr.  | TYPE  | LENGTH  | Description
-----| ----- | ------- | -----------
1    | BYTE  | 1       | Unknown. Is always 0 though.
2 |  BYTE|  1|    Action Type (confirmed)
3|  BYTE|  1| Not sure. It seems half an id and half an action identifier. By building generators the higher 4 bit are 0x8 lower 4 bit: 1 means this action is executed by player with ID 1, does not always correspond to the order of players in the header. I think this might be the position of the player base. So with fixed positioning this corresponds to the player ID, that's why that only works in fixed games.<br>Observation on lower bits: Seem to indicate player base location. <br>Observation on higher bits: Values that have been observed so far [u8]: 0, 1, 2, 3, 4, 5, 128, 129, 130, 131, 132, 133
4|  BYTE|  1|    Player ID (confirmed)
5|  BYTE|  1|    Unknown<br>Values that have been observed so far [u8]: 3
6-7|  BYTE|  2|  A counter for the actions performed by this player. (confirmed)<br> Starts at 0. This means there is a limit of 65536.<br> Best read as u16 LittleEndian
8|  BYTE|  1| 0x10 = build units at HQ, tier upgrade (T2, T3), building upgrade (e.g. Turret -> Missile Turret)<br> 0x20 = unit upgrades, movement<br>0x42 = was spotted with action code 56<br>0x44 = was spotted with action code 56<br>0x0  = seems related to power nodes and placeable entities (Turret, Mines)<br>(seems like any action performed by HQ like building or setting rally point is 10 while every action performed by a unit is 20<br>Values that have been observed so far [u8] (hex): 0 (0x0), 16 (0x10), 32 (0x20), 66 (0x42), 68 (0x44)
9|  BYTE|  1|    Unknown<br> Values that have been observed so far [u8]: 0, 93
10|  BYTE|  1|   Always changes together with 0x10 or 0x20 two bytes before. But sometimes changes between different games.<br> Is this the player location/ID?<br><br> Values that have been observed so far [u8] in 1v1: 3, 74, 195<br>Values that have been observed so far [u8] additionally: 122
11-12| BYTE|  2| Most likely the unit identifier. But how/where is it assigned?<br>When building power nodes and generators this has been observed to be (232, 15)
13-14| BYTE|  2| Always the same? It seems so.<br>When building a power node this has been observed to be (35, 107)<br> When building a generator this has been observed to be (35, 98)
15-19| DWORD| 4| Identifier for the item (unit, upgrade, wargear)<br>See the item codes file for these values<br><br>Identifier for the canceled unit, upgrade, wargear<br>These are numbered in the order they are queued.<br>Units and Base Tiers share the same numbering.<br> Wargear is on a different ordering.<br> Upgrades have a different ordering for every unit. These actions are numbered: upgrades (0x30) and unknown (0x2E)


# General observations:

- Abilities that can be toggled are identical apart from the action counter. As far as the game is concerned you have
  used the same ability twice.

- Targettable abilities seem to also have location information stored in them (probably origin and target coordinates)

- Unit id (once unit is on the field) is not the same as the unit/item id when its purchased at the HQ


