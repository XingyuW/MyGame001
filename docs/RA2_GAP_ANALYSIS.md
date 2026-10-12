# Differences between MyGame001 and Red Alert 2

Prepared 11 October 2026 against the working tree at `8230a95` plus its uncommitted changes (the
unified 72 item arsenal, multiple enemies, game speed, Super Me and map work). The reference is the
original Red Alert 2 without Yuri's Revenge. Network play is out of scope, so lobby, ladder,
online and LAN features are not compared.

## 1. Method and conventions

Game values were read from `assets/data/*.ron` and from the simulation code in `crates/sim`.
Red Alert 2 values were read from a vanilla `rules.ini` and `art.ini` (section 16); the file
carries no patch number, so its version is assumed to be 1.006 but not proven. Behaviour that the
ini does not encode was taken from the C&C Wiki, ModEnc and GameFAQs guides, and anything only one
weak source supports is marked *unverified*. In the tables **G** is this game and **R** is Red Alert 2.

Three conversions are needed because the engines count differently. This game runs 20 simulation
ticks per second at 1x speed, so reloads and build times are given in seconds at 1x. Red Alert 2
gives ROF in frames and build and recharge times in game minutes of 900 frames, which its own
`rules.ini` comment equates to one minute at 15 frames per second; "R nominal" seconds below use
that convention and will be shorter at faster game speeds. Movement speeds use incompatible units,
so each unit's speed is shown relative to the GI of the same game, which makes the two columns
directly comparable as proportions. In the tables `tech center` is the game's id for the Battle Lab,
`(Sov)` marks the Soviet version of an RA2 building, and "any radar" means Airforce Command or Radar
Tower, as RA2's `RADAR` token does.

## 2. Highest impact differences

The items below change how a match plays rather than its numbers, and are ordered by how far they
move the game from Red Alert 2.

| # | Area | This game | Red Alert 2 | Impact |
|---|---|---|---|---|
| 1 | Sides and countries | The human gets one unified Allied plus Soviet tech tree (72 RA2 items plus 12 original legacy items); no country is chosen | Each player picks one of nine countries (five Allied, four Soviet), gets only that side's tree plus one national unit or power | High |
| 2 | Opponent armies | Computer players use the original Vanguard and Meridian rosters (rifleman, scout, light tank, artillery, gun turret and so on), none of which exist in RA2, and never build RA2 units, aircraft, ships or superweapons | The AI plays the same countries, units and superweapons as the human | High |
| 3 | Veterancy | Absent | Veteran at kills worth 3x own cost, elite at a further 3x; +10% damage, +20% speed, +50% armour, 40% faster fire, elite self heal and elite weapons | High |
| 4 | Vision | Shroud plus fog of war: explored cells darken and hide enemies once out of sight | Shroud only (`FogOfWar=no`); once revealed, a cell stays fully visible, except under Gap Generators or after a spy resets radar | High |
| 5 | Weapons per unit | At most one weapon per unit, no elite weapon | Primary and secondary weapons (GI deploy, Tanya C4, Flak Track and Apocalypse anti air, Destroyer anti sub, Yuri psi wave, Desolator eruption), plus elite variants | High |
| 6 | Armour model | 7 classes (infantry, light, heavy, structure, air, naval, submerged) and 13 warheads | 11 armour types (none, flak, plate, light, medium, heavy, wood, steel, concrete, special 1, special 2) with per warhead percentages, splash spread and falloff | High |
| 7 | Ore economy | Finite ore, no regrowth or spreading, no gems, 1 credit per ore unit | Ore worth 25 and gems 50 per bail; ore grows and spreads (`GrowthRate=5` minutes) | High |
| 8 | Production speed | A fixed time per item, unrelated to cost; no bonus for extra factories; one serial queue per producer type, Buildings and Armory sharing the Construction Yard | Time proportional to cost (`BuildSpeed=.7` minutes per 1000 credits); each extra factory multiplies time by 0.8; the Construction Yard has separate main and armoury queues | High |
| 9 | Win condition | A player is defeated when it owns no building | Short game: no buildings and no MCV (`BaseUnit=AMCV,SMCV`); long game: everything destroyed | Medium |
| 10 | Start of match | Fixed 5000 credits; a deployed Construction Yard plus a fixed legacy unit group | Credits 5000 to 10000 (default 10000); an MCV plus 0 to 10 starting units (default 10) | Medium |
| 11 | Garrisons, tech buildings, crates | None | GIs and Conscripts garrison civilian buildings; capturable Oil Derrick, Hospital, Airport, Outpost; eight crate types | Medium |
| 12 | Low power | Production at half speed and every armed building stops, including Pillbox and Sentry Gun | Production slows between 0.5x and 0.8x by deficit; only buildings that draw power stop (Pillbox and Sentry Gun have `Power=0` and keep firing); radar switches off | Medium |
| 13 | Radar | Minimap is always available | Minimap needs Airforce Command or Radar Tower with power | Medium |
| 14 | Infantry crush | Absent | Tanks with `Crusher=yes` kill crushable infantry by driving over them | Medium |

## 3. Player settings

| Setting | This game | Red Alert 2 |
|---|---|---|
| Players | 2 to 10 (you plus 1 to 9 computer players) on every map; each extra player enlarges the map by one eighth per side up to 256 x 256 | Fixed by the map's start positions; maps keep their size |
| Country or side | Starting force choice (Vanguard or Meridian) changes only the initial units | One of nine countries, or random (*random unverified*) |
| Enemy force | Vanguard or Meridian, shared by all computer players | Each computer player has its own country |
| Teams | Computer players always form one team against you (`allied_ai: true` in `sim_host.rs`) | Patch 1.004 added free for all to skirmish; earlier versions used "Battle", every AI against the human (*version detail unverified*) |
| Difficulty | easy, normal, hard, one value for all enemies | Easy, Medium, Brutal per AI slot |
| Starting credits | 5000, not adjustable | Slider 5000 to 10000, default 10000 |
| Starting units | Fixed per force (for example 3 light tanks, 2 scouts, 4 riflemen) | Unit count 0 to 10, default 10 |
| Tech level | STANDARD or ALL UNLOCKED (human only, waives prerequisites) | Tech level 10 by default; no slider in RA2 skirmish (*unverified*) |
| Superweapons | Always on | Can be switched off |
| Crates | Not implemented | On by default |
| Short game | Not selectable; rule is "no buildings" | Checkbox, on by default |
| MCV repacks | Not implemented: an MCV deploys but a Construction Yard cannot pack up | On by default (`MCVRedeploys=yes`) |
| Multi engineer | Not applicable; one Engineer captures | Off (`MultiEngineer=no`); one Engineer captures |
| Bridge destruction | Bridges are indestructible terrain | Bridges can be destroyed and repaired at a repair hut |
| Game speed | 0.5x, 1x, 1.5x, 2x, 3x, 4x of 20 ticks per second, changeable in match | 7 settings with frame caps of 10, 12, 15, 20, 30, 60 and unlimited in single player |
| Colour | 10 colours | Lobby colours (*count unverified*) |
| Additions with no RA2 equivalent | Super Me (1 credit items, 10x build speed, up to 4x unit speed for the human) | None |

## 4. Factions and technology tree

Red Alert 2 splits its arsenal into two sides and nine countries, so a single player can never own
both the Prism Tank and the Apocalypse without capturing an enemy Construction Yard; this game
gives the human all 72 RA2 items at once, plus twelve original items (rifleman, rocket soldier,
grenadier, scout, raider, light tank, heavy tank, artillery, rocket launcher, harvester, gun turret,
rocket tower) that RA2 does not have. Country specials are therefore not exclusive here: Black
Eagle (Korea), Grand Cannon (France), Tank Destroyer (Germany), Sniper (Great Britain), Demolition
Truck (Libya), Desolator (Iraq), Terrorist (Cuba) and Tesla Tank (Russia) are all buildable together,
and America's paradrop does not exist. RA2 also has units this game lacks: the Navy SEAL (in the
file but at tech level -1, so unbuildable in skirmish), and the four units a spy unlocks by entering
a Battle Lab (Chrono Commando, Psi Commando, Chrono Ivan, Yuri Prime).

Where the prerequisite trees differ, the game is usually simpler or substitutes one building.
Rocketeer needs Airforce Command only (RA2: barracks and any radar); Flak Trooper needs nothing
(RA2: Radar Tower); Tesla Trooper needs the Radar Tower (RA2: barracks only); Psychic Sensor needs
the Radar Tower (RA2: Soviet Battle Lab); Tank Destroyer needs nothing (RA2: radar); Demolition
Truck needs the Battle Lab (RA2: radar); Dolphin needs nothing (RA2: Battle Lab); Aegis and Sea
Scorpion need nothing (RA2: radar); Chronosphere and Weather Control also need Airforce Command;
Cloning Vats also need a Barracks; Ore Purifier does not need a refinery; War Factory needs only a
refinery (RA2: refinery and barracks); Battle Lab needs only a War Factory (RA2: War Factory and
radar). RA2 limits the Ore Purifier, Cloning Vats and every superweapon to one per player
(`BuildLimit=1`), and the game has no build limits at all.

## 5. Economy and production

| Rule | This game | Red Alert 2 |
|---|---|---|
| Ore value | 1 credit per ore unit | Ore 25, gems 50 per bail |
| Miner capacity | War Miner 1000, Chrono Miner 500, legacy harvester 700 credits | War Miner 40 bails (1000), Chrono Miner 20 bails (500); gems double that |
| Ore growth | None; fields deplete permanently | Grows and spreads every 5 game minutes |
| Free miner with refinery | The legacy `harvester`, for every player | Chrono Miner (Allied) or War Miner (Soviet) |
| Chrono Miner | Teleports to a refinery dock when full | Same, also self heals (`SelfHealing`) |
| Ore Purifier | +25% while powered | +25% (`PurifierBonus=.25`), one per player |
| Build time | Fixed per item; 10 to 33 s per 1000 credits at 1x | 0.7 game minutes per 1000 credits for every item (42 nominal s) |
| Extra factories | No effect on speed | Each extra factory of a type multiplies time by 0.8 |
| Queue | 16 items per producer type, one item at a time; Buildings and Armory share one Construction Yard queue | Up to 29 queued objects; Construction Yard runs a main and an armoury queue in parallel |
| Low power | Production advances every other tick (0.5x) | Between 0.5x and 0.8x depending on the deficit |
| Sell | 50% of cost | 50% (`RefundPercent`); walls and captured tech buildings unsellable |
| Building repair | Wrench (K); 20 s from zero, full repair costs 25% of price | Wrench (K); full repair costs 15% of price (`RepairPercent`), 8 hit points per step |
| Engineer repair | Restores a friendly building to full and is consumed | Same |
| Service Depot | Repairs nearby vehicles | Repairs one docked vehicle for credits, removes Terror Drones |
| Placement | Within 4 cells of any own building; Shipyard within 32 cells | `Adjacent=2` cells of own buildings |
| Power output | Construction Yard +20, Power Plant +100 | Construction Yard 0, Allied Power Plant +200 |
| Crew | Destroyed vehicles and sold or destroyed buildings release nobody | Survivors and crew escape (`CrewEscape=50%`) |

## 6. Combat model

Beyond the armour and veterancy gaps in section 2, the following rules differ.

| Rule | This game | Red Alert 2 |
|---|---|---|
| Hit model | Every projectile hits its target if the target still exists | Inaccurate ballistic projectiles scatter up to `BallisticScatter=1.0` cell |
| Splash | Ordinary weapons hit one target; splash only for units with the `splash` role, applied as a separate blast | Each warhead has `CellSpread` and `PercentAtMax`, so most explosive weapons splash |
| Area damage and armour | Superweapon, storm, suicide and bomb blasts ignore armour and fall off linearly | Area damage goes through the warhead's armour table |
| Intercepting missiles | V3 and Dreadnought fire ordinary projectiles | V3 rockets and Dreadnought missiles are spawned air units that Patriots, Flak and Aegis can shoot down |
| Damage states | No yellow and red states that change behaviour | `ConditionYellow=50%`, `ConditionRed=25%`; garrisons are forced out when heavily damaged |
| Infantry in cells | One unit per cell | Several infantry share a cell |
| Low power defences | Every armed building stops | Only buildings with negative `Power` stop |
| Aircraft ammunition | Strike, then a timed rearm near Airforce Command; no pad limit | Harrier and Black Eagle carry `Ammo=1`; each Airforce Command has 4 docks |
| Carrier | 2 recoverable aircraft | 3 Hornets that regenerate |
| Transports | Capacity counts units (IFV 1, Flak Track 5, NightHawk 5, Amphibious 12) | Same counts, with size limits (`SizeLimit` 1, 2, 2, 6) so tanks need the amphibious transport |
| Sub detection | No separate sonar layer; submerged armour class only | Submarines and Dolphin are cloaked underwater and need detection |

## 7. Unit properties

Costs match RA2 for every unit except the Black Eagle (1400 against 1200). Hit points, sight,
weapons, relative speed and build time mostly do not match: every arsenal unit except the Sniper
sees 8 cells (RA2 ranges from 4 to 12), about half the infantry and several vehicles and ships have
more hit points than in RA2 (Tanya 250 against 125, Apocalypse 1000 against 800, Amphibious
Transport 1000 against 300), the Harrier and Black Eagle are about twice as fast relative to
infantry, the Kirov less than half as fast, and the Terror Drone almost twice as fast. Weapon columns give damage,
range in cells and reload; RA2 V3 and Dreadnought damage come from `V3RocketDamage` and
`DMislDamage`, and RA2 armour names are the ini names.

| Unit | Cost G/R | Build s G / R nominal | HP G/R | Armour G / R | Speed rel. GI G/R | Sight G/R | Weapon G (dmg / range / reload) | Weapon R (dmg / range / ROF frames) | Prereq G (producer) | Prereq R |
|---|---|---|---|---|---|---|---|---|---|---|
| gi | 200/200 | 4 / 8 | 125/125 | infantry / none | 1.00/1.00 | 8/5 | rifle 14 / 4c / 0.75s | M60 15 / 4c / 20f | none (barracks) | barracks |
| conscript | 100/100 | 5 / 4 | 120/125 | infantry / flak | 1.00/1.00 | 6/5 | carbine 12 / 3.5c / 0.7s | M1Carbine 15 / 4c / 25f | none (barracks) | barracks(Sov) |
| engineer | 500/500 | 8.3 / 21 | 100/75 | infantry / none | 1.00/1.00 | 8/4 | - | - | none (barracks) | any barracks |
| attack dog | 200/200 | 4 / 8 | 100/100 | infantry / none | 2.57/2.00 | 8/9 | dog_bite 200 / 1.5c / 0.6s | GoodTeeth 30 / 1.5c / 30f | none (barracks) | any barracks |
| rocketeer | 600/600 | 10 / 25 | 180/125 | air / none | 2.14/2.25 | 8/8 | air_rifle 25 / 5c / 0.75s | 20mm 25 / 5c / 30f | airforce command (barracks) | barracks, any radar |
| spy | 1000/1000 | 16.65 / 42 | 100/100 | infantry / flak | 1.14/1.00 | 8/9 | - | - | tech center (barracks) | barracks, tech center |
| tanya | 1000/1000 | 16.65 / 42 | 250/125 | infantry / flak | 1.57/1.25 | 8/8 | commando_gun 150 / 6c / 0.6s | DoublePistols 125 / 6c / 10f | tech center (barracks) | barracks, tech center |
| chrono legionnaire | 1500/1500 | 25 / 63 | 150/125 | infantry / none | 1.00/1.25 | 8/8 | chrono_beam 60 / 5c / 1.5s | NeutronRifle 8 / 5c / 120f | tech center (barracks) | barracks, any battle lab |
| sniper | 600/600 | 10 / 25 | 100/125 | infantry / none | 0.86/1.00 | 12/8 | sniper_rifle 250 / 12c / 3.5s | AWP 125 / 14c / 150f | radar tower (barracks) | barracks, any radar |
| flak trooper | 300/300 | 5 / 13 | 100/100 | infantry / none | 0.86/1.00 | 8/5 | flak_weapon 30 / 6c / 1s | FlakGuyGun 20 / 5c / 20f | none (barracks) | barracks(Sov), radar tower |
| tesla trooper | 500/500 | 8.3 / 21 | 200/130 | infantry / Plate | 0.71/1.00 | 8/6 | tesla_weapon 100 / 5c / 2s | ElectricBolt 50 / 3c / 60f | radar tower (barracks) | barracks(Sov) |
| crazy ivan | 600/600 | 10 / 25 | 150/125 | infantry / none | 1.14/1.00 | 8/6 | bomb_trigger 1 / 1.5c / 6s | IvanBomber 400 / 1.5c / 50f | radar tower (barracks) | barracks(Sov), radar tower |
| yuri | 1200/1200 | 20 / 50 | 100/100 | infantry / none | 0.86/1.00 | 8/12 | control_beam 1 / 6c / 5s | MindControl 10 / 7c / 200f | tech center (barracks) | barracks(Sov), tech center(Sov) |
| desolator | 600/600 | 10 / 25 | 200/150 | infantry / plate | 0.71/1.00 | 8/6 | radiation_weapon 50 / 4c / 1.5s | RadBeamWeapon 125 / 6c / 50f | radar tower (barracks) | barracks(Sov), any radar |
| terrorist | 200/200 | 4 / 8 | 100/75 | infantry / flak | 1.57/1.50 | 8/9 | suicide_trigger 1 / 1.5c / 5s | TerrorBomb 225 / 1.5c / 10f | none (barracks) | barracks(Sov), any radar |
| chrono miner | 1400/1400 | 23.3 / 59 | 1000/1000 | heavy / medium | 1.43/1.00 | 8/4 | - | - | refinery (war factory) | war factory, any refinery |
| war miner | 1400/1400 | 23.3 / 59 | 1000/1000 | heavy / medium | 1.14/1.00 | 8/4 | scout_gun 9 / 4c / 0.4s | 20mmRapid 30 / 5.5c / 20f | refinery (war factory) | war factory(Sov), any refinery |
| grizzly | 700/700 | 11.65 / 29 | 300/300 | heavy / heavy | 2.00/1.75 | 8/8 | tank_cannon 40 / 5c / 1.5s | 105mm 65 / 5c / 60f | none (war factory) | war factory |
| rhino | 900/900 | 15 / 38 | 500/400 | heavy / heavy | 1.43/1.50 | 8/8 | heavy_cannon 60 / 5c / 1.8s | 120mm 90 / 5.75c / 65f | none (war factory) | war factory(Sov) |
| ifv | 600/600 | 10 / 25 | 200/200 | light / light | 2.86/2.50 | 8/8 | flak_weapon 30 / 6c / 1s | HoverMissile 25x2 / 6c / 50f | none (war factory) | war factory |
| flak track | 500/500 | 8.3 / 21 | 250/180 | light / heavy | 2.29/2.00 | 8/8 | flak_weapon 30 / 6c / 1s | FlakTrackGun 25 / 5c / 40f | none (war factory) | war factory(Sov) |
| terror drone | 500/500 | 8.3 / 21 | 100/100 | light / special_1 | 4.57/2.50 | 8/4 | drone_claw 30 / 1.5c / 1s | DroneJump 50 / 1.83c / 60f | none (war factory) | war factory(Sov) |
| v3 launcher | 800/800 | 13.3 / 34 | 180/150 | light / light | 1.14/1.00 | 8/7 | v3_missile 160 / 15c / 7s | V3Launcher 200 / 18c / 150f | radar tower (war factory) | war factory(Sov), radar tower |
| mcv | 3000/3000 | 50 / 126 | 1000/1000 | heavy / heavy | 0.86/1.00 | 8/6 | - | - | service depot (war factory) | war factory, service depot |
| mirage tank | 1000/1000 | 16.65 / 42 | 250/200 | heavy / light | 2.00/1.75 | 8/9 | mirage_gun 100 / 7c / 1.75s | MirageGun 100 / 7c / 70f | tech center (war factory) | war factory, tech center |
| prism tank | 1200/1200 | 20 / 50 | 200/150 | light / light | 1.43/1.00 | 8/8 | prism_beam 100 / 10c / 3.25s | Comet 100 / 10c / 100f | tech center (war factory) | war factory, tech center |
| apocalypse | 1750/1750 | 29.15 / 74 | 1000/800 | heavy / heavy | 0.86/1.00 | 8/6 | apocalypse_gun 120 / 7c / 2.25s | 120mmx 100x2 / 5.75c / 80f | tech center (war factory) | war factory(Sov), tech center(Sov) |
| tank destroyer | 900/900 | 15 / 38 | 400/400 | heavy / heavy | 1.43/1.25 | 8/8 | destroyer_gun 140 / 6c / 2s | SABOT 150 / 5c / 70f | none (war factory) | war factory, any radar |
| tesla tank | 1200/1200 | 20 / 50 | 400/300 | heavy / heavy | 1.57/1.50 | 8/8 | tesla_weapon 100 / 5c / 2s | TankBolt 135 / 4c / 75f | radar tower (war factory) | war factory(Sov), radar tower |
| demolition truck | 1500/1500 | 25 / 63 | 150/150 | light / light | 1.71/1.25 | 8/5 | suicide_trigger 1 / 1.5c / 5s | Demobomb 300 / 1c / 80f | tech center (war factory) | war factory(Sov), any radar |
| harrier | 1200/1200 | 20 / 50 | 200/150 | air / light | 6.86/3.50 | 8/8 | air_bomb 180 / 6c / 3s | Maverick 150x2 / 6c / 10f | none (airforce command) | any radar |
| black eagle | 1400/1200 | 23.3 / 50 | 300/200 | air / light | 7.86/3.50 | 8/8 | eagle_bomb 260 / 6c / 3s | Maverick2 200x2 / 6c / 10f | none (airforce command) | any radar |
| nighthawk | 1000/1000 | 16.65 / 42 | 300/175 | air / light | 4.00/3.50 | 8/7 | air_rifle 25 / 5c / 0.75s | BlackHawkCannon 35 / 6c / 40f | none (airforce command) | war factory |
| kirov | 2000/2000 | 33.3 / 84 | 2000/2000 | air / light | 0.57/1.25 | 8/8 | kirov_bomb 400 / 2c / 2.75s | BlimpBomb 250 / 1.5c / 50f | tech center (airforce command) | war factory(Sov), tech center(Sov) |
| amphibious transport | 900/900 | 15 / 38 | 1000/300 | naval / light | 2.00/1.50 | 8/6 | - | - | none (naval shipyard) | naval shipyard |
| destroyer | 1000/1000 | 16.65 / 42 | 600/600 | naval / heavy | 1.71/1.50 | 8/7 | naval_gun 80 / 7c / 2s | 155mm 60 / 8c / 110f | none (naval shipyard) | naval shipyard |
| aegis cruiser | 1200/1200 | 20 / 50 | 800/800 | naval / light | 1.43/1.00 | 8/8 | aegis_missile 100 / 10c / 1s | Medusa 100 / 12c / 15f | none (naval shipyard) | naval shipyard, any radar |
| dolphin | 500/500 | 8.3 / 21 | 200/200 | submerged / light | 2.86/2.00 | 8/4 | sonic 100 / 6c / 1.5s | SonicZap 4 / 6c / 120f | none (naval shipyard) | naval shipyard, tech center |
| aircraft carrier | 2000/2000 | 33.3 / 84 | 1000/800 | naval / heavy | 1.14/1.00 | 8/7 | carrier_strike 250 / 15c / 8s | HornetLauncher 1 / 25c / 150f | tech center (naval shipyard) | naval shipyard, any battle lab |
| typhoon | 1000/1000 | 16.65 / 42 | 600/600 | submerged / heavy | 1.71/1.00 | 8/4 | torpedo 120 / 7c / 2.5s | SubTorpedo 100 / 7c / 120f | none (naval shipyard) | naval shipyard(Sov) |
| sea scorpion | 600/600 | 10 / 25 | 400/400 | naval / heavy | 2.57/2.00 | 8/8 | flak_weapon 30 / 6c / 1s | FlakTrackGun 25 / 5c / 40f | none (naval shipyard) | naval shipyard(Sov), radar tower |
| giant squid | 1000/1000 | 16.65 / 42 | 300/200 | submerged / light | 2.29/2.00 | 8/5 | squid_grip 50 / 1.5c / 1.5s | SquidGrab 15 / 1.5c / 99f | tech center (naval shipyard) | naval shipyard(Sov), tech center(Sov) |
| dreadnought | 2000/2000 | 33.3 / 84 | 1000/800 | naval / heavy | 1.00/1.00 | 8/7 | dread_missile 300 / 16c / 9s | DredLauncher 300x2 / 25c / 50f | tech center (naval shipyard) | naval shipyard(Sov), tech center(Sov) |

## 8. Building properties

Most building costs match, except Power Plant (300 against 800), Barracks (300 against 500),
Battle Lab (1500 against 2000), Nuclear Reactor (1500 against 1000), Chronosphere and Iron Curtain
(3000 against 2500). Footprints are smaller than RA2 for most structures; defensive towers have 33 to
75% of RA2 hit points, while the Power Plant, Refinery and War Factory have less and most late
support buildings have more; the Pillbox and Sentry Gun deal under a third of RA2 damage per shot
and the Tesla Coil half. The Patriot and Flak Cannon both use the
Aegis missile in this game.

| Building | Cost G/R | Build s G / R nominal | HP G/R | Power G / R | Footprint G / R | Weapon G | Weapon R | Prereq G | Prereq R |
|---|---|---|---|---|---|---|---|---|---|
| construction yard | 3000/3000 | 60 / 126 | 1100/1000 | +20 / 0 | 3x3 / 4x4 | - | - | none | none |
| power plant | 300/800 | 10 / 34 | 450/750 | +100 / 200 | 2x2 / 2x2 | - | - | none | none |
| tesla reactor | 600/600 | 9 / 25 | 1000/750 | +150 / 150 | 2x2 / 3x2 | - | - | none | none |
| refinery | 2000/2000 | 20 / 84 | 750/1000 | -40 / -50 | 3x3 / 4x3 | - | - | power plant | any power |
| ore purifier | 2500/2500 | 35 / 105 | 1000/900 | -150 / -200 | 2x2 / 3x3 | - | - | tech center | tech center, any refinery |
| nuclear reactor | 1500/1000 | 35 / 42 | 1000/1000 | +2000 / 2000 | 2x2 / 4x4 | - | - | tech center | tech center(Sov) |
| barracks | 300/500 | 12 / 21 | 450/500 | -20 / -10 | 2x2 / 3x2 | - | - | power plant | any power |
| war factory | 2000/2000 | 25 / 84 | 900/1000 | -30 / -25 | 3x3 / 5x3 | - | - | refinery | any refinery, barracks |
| naval shipyard | 1000/1000 | 20 / 42 | 1000/1500 | -30 / -25 | 3x3 / 4x4 | - | - | refinery | any refinery, any power |
| airforce command | 1000/1000 | 20 / 42 | 1000/600 | -50 / -50 | 3x3 / 3x2 | - | - | refinery | refinery |
| service depot | 800/800 | 15 / 34 | 1000/1200 | -30 / -25 | 2x2 / 3x3 | - | - | war factory | war factory |
| tech center | 1500/2000 | 25 / 84 | 650/500 | -60 / -100 | 2x2 / 3x2 | - | - | war factory | war factory, any radar |
| radar tower | 1000/1000 | 17.5 / 42 | 1000/1000 | -50 / -50 | 2x2 / 2x2 | - | - | refinery | refinery(Sov) |
| cloning vats | 2500/2500 | 40 / 105 | 1000/1000 | -100 / -200 | 2x2 / 2x2 | - | - | tech center, barracks | tech center(Sov) |
| fortress walls | 100/100 | 2 / 4 | 300/300 | +0 / 0 | 1x1 / 1x1 | - | - | none | barracks |
| pillbox | 500/500 | 9 / 21 | 300/400 | +0 / 0 | 1x1 / 1x1 | rifle 14 / 4c / 0.75s | Vulcan2 50 / 5.5c / 26f | barracks | any barracks |
| sentry gun | 500/500 | 9 / 21 | 300/400 | +0 / 0 | 1x1 / 1x1 | scout_gun 9 / 4c / 0.4s | Vulcan 50 / 5.5c / 26f | barracks | any barracks |
| patriot | 1000/1000 | 15 / 42 | 300/900 | -50 / -50 | 1x1 / 1x1 | aegis_missile 100 / 10c / 1s | RedEye2 75 / 12c / 55f | barracks | any barracks |
| flak cannon | 1000/1000 | 15 / 42 | 300/900 | -50 / -50 | 1x1 / 1x1 | aegis_missile 100 / 10c / 1s | FlakWeapon 40 / 12c / 20f | barracks | any barracks |
| prism tower | 1500/1500 | 22.5 / 63 | 300/600 | -75 / -75 | 1x1 / 1x1 | prism_beam 100 / 10c / 3.25s | PrismShot 120 / 8c / 60f | radar tower | any power, any radar |
| tesla coil | 1500/1500 | 22.5 / 63 | 300/600 | -75 / -75 | 1x1 / 1x1 | tesla_weapon 100 / 5c / 2s | CoilBolt 200 / 7c / 120f | radar tower | any power, any radar |
| grand cannon | 2000/2000 | 32.5 / 84 | 1000/900 | -100 / -100 | 2x2 / 2x2 | grand_shell 200 / 14c / 4s | GrandCannonWeapon 150 / 15c / 120f | tech center | any radar |
| gap generator | 1000/1000 | 20 / 42 | 1000/600 | -100 / -100 | 2x2 / 1x1 | - | - | tech center | tech center |
| spy satellite | 1500/1500 | 25 / 63 | 1000/1000 | -100 / -100 | 2x2 / 2x2 | - | - | tech center | tech center |
| psychic sensor | 1000/1000 | 17.5 / 42 | 1000/750 | -50 / -50 | 2x2 / 2x2 | - | - | radar tower | tech center(Sov) |
| chronosphere | 3000/2500 | 80 / 105 | 1000/750 | -200 / -200 | 2x2 / 4x3 | - | - | tech center, airforce command | tech center |
| weather control | 5000/5000 | 100 / 210 | 1000/1000 | -200 / -200 | 3x3 / 3x3 | - | - | tech center, airforce command | tech center |
| iron curtain | 3000/2500 | 80 / 105 | 1000/750 | -200 / -200 | 2x2 / 3x3 | - | - | tech center | tech center(Sov) |
| nuclear silo | 5000/5000 | 100 / 210 | 1000/1000 | -200 / -200 | 3x3 / 3x3 | - | - | tech center, nuclear reactor | tech center(Sov) |

## 9. Specialist abilities and superweapons

| Item | This game | Red Alert 2 |
|---|---|---|
| GI | No deploy | Deploys behind sandbags with a faster firing, longer ranged weapon (`Para`, ROF 15, range 5) |
| Engineer | Captures or fully repairs a building, consumed | Same; also repairs bridges at a repair hut and defuses Ivan bombs |
| Attack Dog | 200 damage anti infantry bite; reveals spies within 6 cells | Leaps on and kills infantry; detects disguised spies (`DetectDisguise`) |
| Spy | Concealed until within 2 cells of an enemy (6 of a dog or powered sensor), no disguise | Disguises as an enemy soldier; dogs see through it |
| Spy infiltration | Refinery: steals half the credits. Power plant: 30 s outage. Superweapon: charge reset. Anything else: disabled 30 s | Refinery: steals half (`SpyMoneyStealPercent=.5`). Power: blackout of 1000 frames. Superweapon: reset. Barracks: veteran infantry. War Factory: veteran vehicles. Radar: resets shroud. Battle Lab: unlocks a special unit |
| Tanya | 150 damage gun and 200 extra damage to buildings | 125 damage pistols; C4 (`Sapper`, 2500 damage) destroys buildings outright |
| Chrono Legionnaire | Teleports; beam deals damage and suppresses | Erases targets over time; target is frozen and invulnerable while being erased |
| Tesla Trooper | Ordinary electrical weapon | Also charges Tesla Coils so they fire without power or overcharged |
| Prism Tower | Single tower beam with splash | Neighbouring towers feed their beam, up to 8 supporters |
| Mirage Tank | Concealed when idle | Disguised as a tree when idle |
| Crazy Ivan | Timed bomb on target | Timed bomb (`IvanDamage=400`, `IvanTimedDelay=450` frames) |
| Yuri | Controls one unit at a time | Same; also a psi wave secondary that kills nearby infantry |
| Desolator | D creates a radiation zone | Same, with radiation levels that decay; rad beam primary does 125 |
| Terror Drone and Squid | Timed suppression of the target | Drone climbs inside a vehicle and destroys it; Squid grabs a ship until a Dolphin frees it |
| IFV | Takes its passenger's weapon, Engineer gives repair | Same idea with crew specific turrets for every infantry type |
| Nuclear Reactor | No death effect | Explodes with a nuclear payload when destroyed |
| Chronosphere | Teleports a grouped selection of your own Vehicles category units | Teleports every unit in a 3 x 3 area, including enemy units; infantry die, ground units placed in water sink |
| Iron Curtain | 10 s invulnerability to your Vehicles category units near the target | 750 frames (50 nominal s) on vehicles and buildings, kills infantry |
| Weather Control | One damage pulse per second for 10 s within 5 cells, hitting everyone, ignoring armour | Random lightning bolts (250 damage each) over a wide area for 180 frames, after a warning delay |
| Nuclear Missile | Immediate 2500 damage blast within 6 cells, no fallout | Missile flight then 600 damage payload with radiation level 500 over 10 cells |
| Recharge at 1x | Chronosphere 180 s, Iron Curtain 180 s, Weather and Nuke 300 s | Chronosphere 7, Iron Curtain 5, Weather 10, Nuke 10 game minutes |
| Warning to other players | None | EVA announces superweapon construction and launch to all players |
| Paradrop | Absent | America's Airforce Command drops 8 GIs every 4 minutes; a captured Airport gives a paradrop |

## 10. Map, terrain and vision

| Rule | This game | Red Alert 2 |
|---|---|---|
| Elevation | Flat; cliffs are blocked terraces | Height levels, slopes and cliffs that units climb by ramps |
| Bridges | Indestructible | Destructible and repairable |
| Civilian buildings | Destructible scenery only | Destructible and garrisonable |
| Map growth | Maps grow with the player count and invent missing start positions | Maps are fixed |
| Gap Generator | Shrouds a 6 cell radius for enemies; local sight counters it | Re-shrouds a 10 cell radius continuously |
| Spy Satellite | Reveals the whole map while powered | Same; also nullifies radar infiltration |
| Psychic Sensor | Detects concealed units within 6 cells | Detects within 15 cells (`PsychicDetectionRadius=15`) and shows enemy movement orders |

## 11. Interface and controls

| Function | This game | Red Alert 2 |
|---|---|---|
| Sidebar tabs | Buildings, Armory, Infantry, Vehicles; Tab cycles | Buildings, defences and support powers, infantry, vehicles (*exact labels unverified*); Q, W, E, R |
| Repair and sell | K repair, X or the Sell button sell mode | K repair, L sell |
| Attack move | A | No vanilla attack move key (*Ctrl+Shift unverified*) |
| Guard, scatter, follow | Absent | G, X, F |
| Waypoint mode | Absent | Z |
| Force fire and force move | Absent | Ctrl click; Alt click (*Alt unverified*) |
| Type select | Double click | T (all of type), P (all combat units) |
| Unit cycling, bookmarks, home | Absent | N, M, Ctrl+F1 to F4, H |
| Control groups | Ctrl or Cmd + digit, digit, Shift + digit | Ctrl + digit, digit, Shift + digit, Alt + digit centres |
| Alliance toggle | Absent | A |
| Voices | Synthesised sound effects, text notices for defeats | EVA announcements (unit ready, base under attack, low power, superweapon detected) and unit voices |
| Rally points | Per building and global, R key | Per factory |

## 12. Computer opponent

The game's AI builds a fixed order of legacy buildings and an army from its faction's legacy
roster, and has no naval, air, specialist, defence beyond its turret type, or superweapon play. It
knows only what its vision shows, and the three difficulties change only think interval, harvester
count, army cap and wave timing. The RA2 AI builds the full side and country arsenal and uses
superweapons, and according to ModEnc it ignores ownership and prerequisite limits and sees
through shroud and cloaking at every difficulty; its difficulty scales firepower, rate of fire,
build time, repair delay, bonus money and team frequency. All computer players here form one team
against you, while RA2 skirmish offers free for all since patch 1.004 (*version detail unverified*).

## 13. Features this game has that RA2 lacks

Super Me, ALL UNLOCKED, unit speed for the human, maps that grow with up to ten players and new
terrain types (ice, marsh, lava, ash, crops), fog of war, an attack move key, per building rally
points, a configurable edge scroll and in match game speed change. These are deliberate additions
and are listed only so that a faithful mode can switch them off.

## 14. Suggested order for closing the gaps

1. Add sides and countries with exclusive trees and national specials, and let the AI use them.
2. Add veterancy, secondary and elite weapons, and the 11 armour type warhead table with splash.
3. Make build time proportional to cost with the multiple factory bonus, add the parallel armoury
   queue, build limits and the graded low power penalty.
4. Add ore regrowth, gems, crates, garrisons, tech buildings, crushing and the MCV win rule.
5. Switch to shroud only vision, gate the minimap on powered radar, and align superweapon effects.
6. Add the RA2 skirmish options (credits, unit count, short game, superweapons, crates, MCV repacks).
7. Align the property tables in sections 7 and 8 last, because they depend on items 2 and 3.

## 15. Not verified

The rules.ini patch version; RA2's maximum player count per map, colour count and random country
option; whether RA2 skirmish exposes a tech level slider; the Alt force move and Ctrl+Shift attack
move keys; and the exact AI difficulty numbers. RA2 display names come from `ra2.csf`, which was
not read, so the internal ini names in sections 7 and 8 were mapped to display names by role.

## 16. Sources

* Vanilla RA2 `rules.ini` and `art.ini`: github.com/otherdeniz/CnC_RulesEditor,
  `Deniz.TiberiumSunEditor.Gui/Resources/RA2rules.ini` and `RA2art.ini` at commit `b7278f2`.
* C&C Wiki: https://cnc.fandom.com/wiki/Veterancy, /Spy_(Red_Alert_2), /Infiltration, /Crate,
  /Garrisoning, /Tech_building, /Iron_Curtain_(Red_Alert_2), /Chronosphere_(Red_Alert_2),
  /Soviet_construction_yard_(Red_Alert_2), /Sidebar, /Fog_of_war.
* ModEnc: https://modenc.renegadeprojects.com/MultiplayerDialogSettings, /Game_Speed, /ShortGame,
  /BaseUnit, /MCVRedeploys, /FogOfWar, /Powerups, /AI, /Difficulty_Settings, /Adjacent.
* GameFAQs SRN strategy guide (v1.006):
  https://gamefaqs.gamespot.com/pc/914165-command-and-conquer-red-alert-2/faqs/9899
* Patch history 1.001 to 1.006:
  https://cncnz.com/features/technical-support-help-guides/red-alert-2-v1001-v1006-patch-history/
* https://en.wikipedia.org/wiki/Command_%26_Conquer:_Red_Alert_2
