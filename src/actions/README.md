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
10|  BYTE|  1|   Always changes together with 0x10 or 0x20 two bytes before. But sometimes changes between different games.<br> Is this the player location/ID?<br><br> Values that have been observed so far [u8] in 1v1: 3, 74, 120, 195<br>Values that have been observed so far [u8] additionally: 122
11| BYTE|  1| Most likely the unit identifier. But how/where is it assigned?
12-13| BYTE| 2 | Seems to provide more context for the action (see general observations)
14| BYTE|  1| Identifier for e.g. wargear, unit, global ability.<br><br>When building a power node this has been observed to be 107<br> When building a generator this has been observed to be 98<br>When using global abilities this has been observed to be 115, 123, 125, 124, 119
15-19| DWORD| 4| **STILL NOT CONFIRMED**<br>Identifier for the item (unit, upgrade, wargear)<br>See the item codes file for these values<br><br>Identifier for the canceled unit, upgrade, wargear<br>These are numbered in the order they are queued.<br>Units and Base Tiers share the same numbering.<br> Wargear is on a different ordering.<br> Upgrades have a different ordering for every unit. These actions are numbered: upgrades (0x30) and unknown (0x2E)


# General observations:

- Abilities that can be toggled are identical apart from the action counter. As far as the game is concerned you have
  used the same ability twice.

- Targettable abilities seem to also have location information stored in them (probably origin and target coordinates)

- Unit id (once unit is on the field) is not the same as the unit/item id when its purchased at the HQ

- Action data fields 12 + 13 seem to decode the following (amongst other stuff):
  - (1, 3) => Something to do with action type 48
  - (1, 5) => Capturing
  - (1, 13) => Movement
  - (3, 26) => Movement + Direction
  - (5, 4) => Purchase
  - (6, 17) => Movement
  - (18, 17) => Movement
  - (15, 35) => Single target, building (e.g. Webway Gate)
  - (25, 5) => Non-targettable, affects every target-type at once (e.g. Angels of Death, Blessing of the Omnissiah, Swift Movement) or is called in at base (e.g. Seer Council). Also applicable to unit abilities that are self-targetting
  - (26, 9) => Single target, unit or global ability (e.g. Scout grenades, Farseer Guide, For the Emperor, Crackshot)
  - (26, 19) => Single target with no secondary projectiles (e.g. Drop Pod, Terminators, Eldritch Storm)
  - (28, 59) => Multiple targets but no unit call-in (e.g. Orbital Bombardement)
  - (28, 71) => Multiple targets and unit call-in (e.g. Autarch)


# Global Abilities
## Space Marines (Techmarine, Apothecary, Force Commander)

Name | 1 | Action Type| Player location ID | Player ID |5|Action Counter I|Action Counter II|Action Source (u8)|9|10|11|Action Context I|Action Context II|Item ID|15|16|17|18|19|20
-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-
Blessing of the Omnissiah | 0| 85| 1| 233| 3| 0| 0| 0| 0| 3| 233| 25| 5| 115| 3| 0| 0
Drop Pod | 0| 85| 1| 233| 3| 1| 0| 0| 0| 3| 233| 26| 19| 123| 3| 0| 0| 1| 0| 2| 74| 75| 68| 193| 72|5| 210| 66| 126| 168| 37
Venerable Dreadnought | 0| 85| 1| 233| 3| 4| 0| 0| 0| 3| 233| 26| 19| 125| 3| 0| 0| 1| 0| 2| 90| 174| 28| 65| 51| 243| 209| 66| 100| 222| 42
Terminator Call-in |0| 85| 1| 233| 3| 5| 0| 0| 0| 3| 233| 26| 19| 124| 3| 0| 0| 1| 0| 2| 152| 31| 118| 64| 96| 111| 208| 66| 65| 114| 31
Orbital Bombardement | 0| 85| 1| 233| 3| 6| 0| 0| 0| 3| 233| 28| 59| 119| 3| 0| 0| 1| 0| 2| 12| 192| 220| 192| 58| 250| 209| 66| 229| 136| 254| 66| 2| 198| 116| 30| 65| 54| 139| 208| 66| 232| 162| 242| 66| 2| 128| 5| 29| 62| 113| 165| 203| 66| 228| 233| 217| 66| 2| 77| 154| 140| 193| 230| 2| 204| 66| 148| 187| 239| 66
Larramans Blessing | 0|85|1|233|3|2|0|0|0|3|233|25|5|121|3|0|0
Drop Pod | 0|85|1|233|3|3|0|0|0|3|233|26|19|123|3|0|0|1|0|2|248|94|99|193|16|29|210|66|206|62|38
Angels of Death | 0|85|1|233|3|4|0|0|0|3|233|25|5|114|3|0|0
Terminator Call-in | 0|85|1|233|3|5|0|0|0|3|233|26|19|124|3|0|0|1|0|2|42|238|65|65|50|243|209|66|67|252|40
Orbital Bombardement | 0|85|1|233|3|6|0|0|0|3|233|28|59|119|3|59|119|3|0|0|1|0|2|0|30|115|189|22|77|209|66|24|59|4|67|2|0|30|115|189|22|77|209|66|24|59|4|67|2|0|189|54|63|9|21|209|66|117|205|3|67|2|0|252|45|189|22|77|209|66|159|129|4|67
For the Emperor | 0|85|0|232|3|2|0|0|0|3|232|26|9|117|3|0|0|1|0|4|84
Drop Pod | 0|85|0|232|3|3|0|0|0|3|232|26|19|123|3|0|0|1|0|2|16|16|2|194|51|220|210|66|101|95|44
Assault Terminators | 0|85|0|232|3|4|0|0|0|3|232|26|19|122|3|0|0|1|0|2|174|243|136|193|41|220|210|66|39|120|41
Ranged Terminators | 0|85|0|232|3|21|0|0|0|3|232|26|19|124|3|0|0|1|0|2|52|152|37|193|0|0|180|66|214|222|150
Orbital Bombardement | 0|85|0|232|3|23|0|0|0|3|232|28|59|119|3|0|0|1|0|2|182|98|104|66|44|185|191|66|146|253|30|194|2|182|98|104|66|44|185|191|66|146|253|30|194|2|182|98|104|66|44|185|191|66|146|253|30|194|2|151|124|104|66|10|236|191|66|118|23|31|194

## Eldar (Warlock, WSE, Farseer)

Name | 1 | Action Type| Player location ID | Player ID |5|Action Counter I|Action Counter II|Action Source (u8)|9|10|11|Action Context I|Action Context II|Item ID|15|16|17|18|19|20
-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-
Webway Gate | 0|78|1|233|3|2|0|0|0|3|233|15|35|198|1|0|0|54|10|136|193|86|236|209|66|211|70|37|67|54|10|136|193|86|236|209|66|211|70|38|67|0|0|0|0|0|0
Swift Movement | 0|85|1|233|3|3|0|0|0|3|233|25|5|197|1|0|0
Distort Field | 0|85|1|233|3|4|0|0|0|3|233|26|9|199|1|0|0|1|0|4|86
Autarch Drop | 0|85|1|233|3|5|0|0|0|3|233|28|71|205|1|0|0|1|0|2|132|130|77|65|65|230|209|66|238|79|38|67|2|96|247|232|64|204|11|209|66|185|40|34|67|2|216|235|88|64|142|157|208|66|251|182|32|67|2|64|188|88|191|192|157|208|66|189|175|32|67|2|28|89|150|192|140|230|208|66|252|39|34
Eldritch Storm | 0|85|1|233|3|6|0|0|0|3|233|26|19|201|1|0|0|1|0|2|220|244|156|64|120|27|208|66|17|190|12
Webway Gate | 0|78|1|233|3|2|0|0|0|3|233|15|35|198|1|0|0|250|168|131|193|154|236|209|66|95|52|38|67|250|168|131|193|154|236|209|66|95|52|39|67|0|0|0|0|0|0
Crackshot | 0|85|1|233|3|3|0|0|0|3|233|26|9|198|1|0|0|1|0|4|87
Warpspider Call-in | 0|85|1|233|3|4|0|0|0|3|233|26|19|208|1|0|0|1|0|2|144|136|76|65|106|233|209|66|190|147|38
Autarch | 0|85|1|233|3|5|0|0|0|3|233|28|71|205|1|0|0|1|0|2|24|81|206|64|246|134|209|134|209|66|36|129|35|67|2|240|170|246|64|61|37|208|66|248|46|25|67|2|144|25|23|64|71|32|208|66|19|81|23|67|2|240|153|43|192|2|64|208|66|128|75|24|67|2|40|119|247|192|6|171|208|66|136|140|29
Eldritch Storm | 0|85|1|233|3|6|0|0|0|3|233|26|19|201|1|0|0|1|0|2|56|143|35|64|169|139|205|66|18|196|246
Webway Gate | 0|78|0|232|3|2|0|0|0|3|232|15|35|198|1|0|0|112|54|26|194|42|220|210|66|14|122|41|195|112|54|26|194|42|220|210|66|14|122|40|195|0|0|0|0|0|0
Farsight | 0|85|0|232|3|3|0|0|0|3|232|26|19|204|1|0|0|1|0|2|0|184|204|62|157|240|208|66|72|225|34
Autarch | 0|85|0|232|3|4|0|0|0|3|232|28|71|206|1|0|0|1|0|2|226|10|170|193|22|220|210|66|20|45|31|195|2|41|16|137|193|47|220|210|66|255|99|30|195|2|154|28|118|193|250|219|210|66|85|172|29|195|2|104|196|58|193|218|219|210|66|150|131|31|195|2|56|81|55|193|201|219|210|66|228|109|31
Seer Council | 0|85|0|232|3|5|0|0|0|3|232|25|5|207|1|0|0
Eldritch Storm | 0|85|0|232|3|6|0|0|0|3|232|26|19|201|1|0|0|1|0|2|248|246|73|193|55|132|207|66|226|131|6

# Unit Abilities
## Space Marines
Name | 1 | Action Type| Player location ID | Player ID |5|Action Counter I|Action Counter II|Action Source (u8)|9|10|11|Action Context I|Action Context II|Item ID|15|16|17|18|19|20
-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-
Tacs activate Kraken Rounds | 0|53|1|233|3|6|0|32|0|195|85|25|5|164|3|0|0
Scouts cloaking | 0|53|1|233|3|7|0|32|0|195|83|25|5|143|3|0|0
Scouts uncloaking | 0|53|1|233|3|8|0|32|0|195|83|25|5|143|3|0|0
Scouts throw grenade | 0|53|1|233|3|9|0|32|0|195|83|26|19|144|3|0|0|1|0|2|59|226|160|66|0|0|200|66|120|215|161
FC battlecry | 0|53|1|233|3|10|0|32|0|195|82|25|5|107|3|0|0
FC alacrity | 0|53|1|233|3|11|0|32|0|195|82|25|5|110|3|0|0
FC toggle on halo | 0|53|1|233|3|12|0|32|0|195|82|25|5|109|3|0|0
FC toggle off halo | 0|53|1|233|3|13|0|32|0|195|82|25|5|109|3|0|0

## Eldar
Name | 1 | Action Type| Player location ID | Player ID |5|Action Counter I|Action Counter II|Action Source (u8)|9|10|11|Action Context I|Action Context II|Item ID|15|16|17|18|19|20
-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-
Farseer Fleet of Foot | 0|53|1|233|3|8|0|32|0|195|86|25|5|173|1|0|0
Farseer Guide | 0|53|1|233|3|9|0|32|0|195|86|26|9|182|1|0|0|1|0|4|87

# Unit Purchases
## Eldar
Name | 1 | Action Type| Player location ID | Player ID |5|Action Counter I|Action Counter II|Action Source (u8)|9|10|11|Action Context I|Action Context II|Item ID|15|16|17|18|19|20
-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-
Dire Avengers | 0|3|1|233|3|0|0|16|0|120|180|5|4|141|0|0
Howling Banshees | 0|3|1|233|3|1|0|16|0|120|180|5|4|137|0|0
Rangers | 0|3|1|233|3|2|0|16|0|120|180|5|4|146|0|0
Shuriken Platform | 0|3|1|233|3|3|0|16|0|120|180|5|4|144|0|0
Dark Reapers | 0|3|1|233|3|5|0|16|0|120|180|5|4|138|0|0
Warp Spiders | 0|3|1|233|3|6|0|16|0|120|180|5|4|148|0|0
Fire Dragons | 0|3|1|233|3|7|0|16|0|120|180|5|4|140|0|0
Wraithlord | 0|3|1|233|3|8|0|16|0|120|180|5|4|153|0|0
Falcon | 0|3|0|232|3|1|0|16|0|120|173|5|4|151|0|0
Wraithguard | 0|3|0|232|3|2|0|16|0|120|173|5|4|149|0|0
Fire Prism | 0|3|0|232|3|4|0|16|0|120|173|5|4|152|0|0
D-Cannon | 0|3|0|232|3|5|0|16|0|120|173|5|4|143|0|0
Avatar | 0|3|0|232|3|6|0|16|0|120|173|5|4|145|0|0
Seer Council | 0|3|0|232|3|7|0|16|0|120|173|5|4|147|0|0

# Wargear Purchases
## Eldar
### Farseer
Name | 1 | Action Type| Player location ID | Player ID |5|Action Counter I|Action Counter II|Action Source (u8)|9|10|11|Action Context I|Action Context II|Item ID|15|16|17|18|19|20
-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-|-
Doombringer |0|50|1|233|3|2|0|32|0|195|86|5|4|192|0|0
Fortune Armor | 0|50|1|233|3|3|0|32|0|195|86|5|4|184|0|0
Spirit Stones | 0|50|1|233|3|4|0|32|0|195|86|5|4|175|0|0
Singing Spear | 0|50|1|233|3|5|0|32|0|195|86|5|4|193|0|0
Rune Armor | 0|50|1|233|3|6|0|32|0|195|86|5|4|185|0|0
Ghosthelm | 0|50|1|233|3|7|0|32|0|195|86|5|4|173|0|0
Gravity Blade | 0|50|1|233|3|10|0|32|0|195|86|5|4|194|0|0
Asuryan Armor | 0|50|1|233|3|11|0|32|0|195|86|5|4|183|0|0
Runes of Reaping | 0|50|1|233|3|12|0|32|0|195|86|5|4|174|0|0
