# CFECards Rules Engine

Deterministic rules engine for CFECards game state, formation resolution, turn flow, event replay, hidden information, and team-mode combat.

## Language

**Rules Engine**:
A pure Rust library that validates commands, advances deterministic game state, resolves formations, emits canonical events, and supports replay.

**Ruleset**:
A complete deterministic rule configuration made from the mandatory Base
Ruleset and zero or more Rule Modules.
_Avoid_: formation registry

**Base Ruleset**:
The mandatory official rules foundation used by every game. It is also a
complete Ruleset when no Rule Modules are enabled.
_Avoid_: optional base rules, team mode ruleset

**Rule Module**:
An optional deterministic addition layered on the Base Ruleset. Official Rule
Modules retain their published category while sharing one configuration and
execution model.
_Avoid_: Ruleset, game mode

**Advanced Rule Module (進階規則)**:
An official Rule Module categorized as 進階規則, such as the Star Rule Module.
_Avoid_: Optional Rule Module, Base Ruleset

**Optional Rule Module (選用規則)**:
An official Rule Module categorized as an optional or supplemental rule, such
as Discard Retrieval or Personal Deck.
_Avoid_: Advanced Rule Module, Base Ruleset

**Theme Rule Module (主題規則)**:
An official expansion Rule Module layered on the complete official-play
configuration: the Base Ruleset and all three Advanced Rule Modules.
_Avoid_: Optional Rule Module, Advanced Rule Module

**Spirit Rule Module (精靈規則)**:
The official Theme Rule Module that adds player-owned Spirits, Spirit Power,
Spirit Skills, five Spirit-summoning Formations, and Void Spirit-Shattering
Technique. It requires Star, Five Directions Legend, and Hero Schools.
_Avoid_: Spirit Ruleset, individual Spirit toggle

**Jianghu Rule Module (江湖規則)**:
The official Theme Rule Module that adds 獨行客 and the 劍客, 煉氣者, 尋墨客,
and 毒師 Profession systems. It also defines the 千鋒, 踏雪, and 中毒 Jianghu
States.
_Avoid_: Jianghu Ruleset, Hero Schools extension

**Echo Rule Module (迴響規則)**:
The official Theme Rule Module that adds seven shared Melody Formations, optional
Echo costs, and delayed repetition of Melody main effects. It requires all three
Advanced Rule Modules, while Personal Deck remains optional.
_Avoid_: Echo Ruleset, Personal Deck extension

**Melody Formation (曲調)**:
One of the seven Formations shared by every Player under the Echo Rule Module.
Each Melody defines a main effect and may repeat that effect at a later timing.
_Avoid_: song, Player ability

**Echo (迴響)**:
The one-time repetition of a Melody's main effect at its performing Player's
next Turn Start. It is not a new Formation Use and cannot trigger another Echo.
_Avoid_: Formation replay, recurring effect

**Echo Cost (迴響代價)**:
The specified-element Card that a Player may Discard after a Melody's main
effect resolves to schedule its Echo. 變徵‧淨火 schedules Echo without a cost.
_Avoid_: casting cost, Formation card

**Scheduled Echo**:
A committed one-time Echo due at its Player's next Turn Start. It retains the
Player, Melody, and timing, while targets and other choices resolve from the
then-current Game State.
_Avoid_: Pending Choice, copied result

**Confluence Generation Rule Module (匯流世代規則)**:
The official Theme Rule Module that adds the 調律師, 道法師, 晴風士, and
虛空追尋者 Profession systems. Some of its Formations and Profession Abilities
have limited uses that can be recovered or reset.
_Avoid_: Confluence Ruleset, Hero Schools extension

**Tuner Profession System (調律師系統)**:
The Confluence Generation Profession chain comprising 調律師, 易弦師, and
天響師, whose transitions, Formations, and abilities derive from the
Retrievable Discard's Residual Element and Residual Level.
_Avoid_: Confluence Generation system, 調律 ability

**Tuning Card Obligation (調律牌義務)**:
The turn-scoped requirement attached to the Card obtained through 調律. That
Card must participate in an allowed Profession Change or, while 易弦 applies,
one of the Player's currently available Profession Formations, including
inherited Formations. No other voluntary ability may move or consume that Card,
or consume other Cards in a way that removes every remaining completion path.
_Avoid_: optional Tuning bonus, generic Card-use restriction

**Pouch Rule Module (錦囊規則)**:
The official Theme Rule Module that adds player-owned Pouches, ten Secret
Strategies, and the Chain Formation. In this product it requires the Personal
Deck and Spirit Rule Modules, transitively requiring all three Advanced Rule
Modules.
_Avoid_: Pouch Ruleset, optional setup rule

**Pouch (錦囊)**:
One face-down Card owned by a Player and chosen from a Deck. Its owner may
reveal it before their action to trigger one eligible Secret Strategy.
_Avoid_: hand Card, Covered Passive, shared Pouch

**Pouch Owner**:
The Player who may inspect and trigger a Pouch. Pouch ownership does not change
the Card's immutable Card Origin when Chain gives a Card to a teammate.
_Avoid_: Card Origin, Pile Owner

**Player-Only Secret Protection**:
Golden Cicada's protection of its triggering Player from the listed
Player-facing effects and other Players' Secret Strategies. It makes Watch the
Fire ineffective against that Player's Turn, so their Formation's Attack damage
and Formation-caused HP changes resolve normally, but it does not
independently protect their Spirit, Team, Team Star, or the shared Environment.
_Avoid_: Team protection, Spirit protection, global Secret immunity

**Watch the Fire Protection (觀火保護)**:
The Secret Strategy effect that protects the Next Player's next Turn from
Attack damage and Formation-caused HP changes. Golden Cicada leaves this
effect present and visible until its ordinary expiry while making it
ineffective against the protected Player.
_Avoid_: Attack prohibition, hidden protection, removed by Golden Cicada

**Initial Pouch Selection**:
The pre-deal stage in which every Player independently and privately chooses
one starting Pouch from their unshuffled Personal Deck. The stage remains open
until every Player has chosen, after which the remaining Personal Decks are
shuffled before the initial hands are dealt.
_Avoid_: room configuration, starting hand choice

**Game Preparation**:
The pre-turn lifecycle in which enabled rules collect Player setup decisions,
trusted randomness, and the initial deal before the first Turn begins.
_Avoid_: Turn Phase, waiting room configuration

**Secret Strategy (秘計)**:
One of ten effects triggered by revealing a Pouch whose printed element or
level satisfies that strategy's condition.
_Avoid_: Formation, Spirit Skill, Pouch effect

**Secret Strategy Option**:
A state-derived pairing of one source Card, one eligible Secret Strategy, and
the immediately required input candidates for that strategy. It may support a
direct Pouch trigger or appear inside a Chain Pending Choice, so it is not
necessarily an independently committable Action.
_Avoid_: Secret Strategy Action, resolved Secret Strategy effect

**Chain Formation (連環)**:
The active Spell that searches a Deck for one or two Cards with different
elements and levels. One becomes a friendly Player's Pouch; when a second is
chosen, it triggers one eligible Secret Strategy immediately.
_Avoid_: Chain status, two-Card Formation

**Dark Glimmer Rule Module (黑暗微光規則)**:
The official Theme Rule Module that adds the 暗行者, 影戰士, and 魔靈師
Profession systems, Dark Formations, and the 惡精靈 and 死精靈. Its Spirits
follow and require the Spirit Rule Module.
_Avoid_: Dark Glimmer Ruleset, Spirit extension

**Tribulation Rule Module (天劫規則)**:
The official Theme Rule Module that adds the five Tribulations and Divine
Calculation, including their shared global effects and statuses.
_Avoid_: Tribulation Ruleset, individual Tribulation toggle

**Divine Calculation Status (神算狀態)**:
The exclusive Player-owned protection granted by Divine Calculation and
consumed by the next Tribulation any Player performs. Its protection is the
Status's own rule, not a repeated effect of the granting Formation.
_Avoid_: Team status, stackable protection, timed status

**Gale-Rain Status (烈風暴雨狀態)**:
A Player-owned, two-turn Tribulation effect that makes life-recovery effects of
Formations performed by that Player ineffective.
_Avoid_: Team healing prohibition, non-Formation healing prohibition

**Spirit (精靈)**:
A persistent elemental entity owned by one Player under the Spirit Rule Module.
A Player may own at most one Spirit, while different Players may own the same
kind.
_Avoid_: team Spirit, status effect

**Spirit Power (靈力)**:
The bounded resource held by a Spirit and spent to use its Spirit Skills. Its
range is zero through six.
_Avoid_: mana, Player resource

**Spirit Skill (精靈技能)**:
An ability granted by a Player's Spirit, normally used during the active-effect
timing by spending Spirit Power. Bloom may also trigger automatically.
_Avoid_: Formation, Profession Ability

**Spirit Summoning (召喚精靈)**:
The acquisition of a specified Spirit with two initial Spirit Power. It replaces
the summoning Player's existing Spirit, if any.
_Avoid_: Spirit transformation, team summon

**Spirit Breaking (破除精靈)**:
The removal of a Player's Spirit when it is replaced or its Spirit Power reaches
zero.
_Avoid_: voluntary dismissal, Spirit expiry

**Star Rule Module (星辰圖記規則)**:
The optional official Advanced Rule Module that adds Star Summoning, Stars, Star
Formations, Star Element Substitution, and Five-Star Alignment.
_Avoid_: Star Ruleset, star mode

**Five Directions Legend Rule Module (五方傳說規則)**:
The optional official Advanced Rule Module that adds Sacred Beasts, the shared
Environment, and Void Meridian-Severing Technique as one indivisible rules option.
_Avoid_: Five Directions Legend Ruleset, Field Ruleset, individual Sacred Beast toggle

**Hero Schools Rule Module (英雄學派規則)**:
The optional official Advanced Rule Module that adds Professions, Profession
Changes, Profession Abilities, Profession Formations, and Void Reversion Technique
as one indivisible rules option.
_Avoid_: Hero Schools Ruleset, individual Profession toggle

**Profession (職業)**:
A Player-owned role provided by an enabled Rule Module. A Player starts without
a Profession and may own at most one across all modules; a new Profession
replaces the previous one.
_Avoid_: class, hero, character

**Profession System (職業系統)**:
A published progression family whose Professions share transition or inherited
ability relationships. A Profession System does not give a Player another
Profession slot.
_Avoid_: separate Profession track, Rule Module

**Legendary Profession (傳說職業)**:
A published Profession classification that normally survives Void Reversion
Technique when its performing Cards are below level three.
_Avoid_: third-tier Profession, automatically strongest Profession

**Profession Change (轉職)**:
The transition by which a Player acquires or replaces their Profession after
meeting the requirements imposed by the action or effect causing it.
_Avoid_: class change, setup profession selection

**Direct Profession Change**:
A Profession Change caused by a rule effect without paying the Profession's
ordinary Card or prerequisite requirements. It remains subject to effects that
forbid Profession Change.
_Avoid_: Profession Change action, free normal Profession Change

**Profession Breaking (破除職業)**:
The removal of a Player's current Profession without replacing it, returning
that Player to having no Profession.
_Avoid_: Profession Change, profession expiry

**Automatic Profession Ability (普通能力)**:
A Profession Ability that applies automatically and continuously while the
Player owns the granting Profession.
_Avoid_: passive spell, Covered Passive, Status Effect

**Formation Proficiency (專精能力)**:
An automatic Profession Ability that gives its Player an alternative way to
match a specified Formation.
_Avoid_: Formation replacement, card mutation

**Formation Match Option**:
One legal interpretation of submitted Card Instances for a declared Formation,
including any role assignment that changes the Formation's result.
_Avoid_: automatic best match, separate Formation

**Action Card Selection**:
The exact set of physical Card Instances a Player is currently proposing for
one offered Active Effects Process Command. A non-empty selection matches only
offers that use every selected Card; an empty selection matches only offers
that require no selected physical Card.
_Avoid_: candidate filter, Formation Composition, Pending Choice

**Playable Action**:
A currently legal Active Effects Process Command offer whose physical Card
input exactly matches the Action Card Selection. Completing any declared Action Input
Requirement produces a command that is accepted while the Game State remains
unchanged.
_Avoid_: Formation Catalog entry, UI capability flag, predicted command result

**Formation Composition**:
The physical Card Instances and optional Virtual Formation Card accepted as the
components of one Formation Use.
_Avoid_: selected Cards, Discard list

**Formation Area (陣法區)**:
The Player-owned Card zone holding a Formation's physical Cards after they
leave that Player's hand and until they are Discarded. Attacks and active
Spells resolve face up; Covered Passives wait face down until their trigger
timing. Each Player's Formation Area holds at most one Formation.
_Avoid_: Formation Display, display zone, Covered Passive zone

**Virtual Formation Card (虛擬牌)**:
A non-physical Formation component created with a fixed source ability,
element, and level. It belongs to no Card zone and has no Card Instance or Card
Origin.
_Avoid_: token Card Instance, interpreted hand Card

**Formation Requirement**:
A turn-scoped commitment that a specified physical or Virtual Formation Card
must participate in an allowed Formation Use before the Player ends their
action opportunity.
_Avoid_: optional preparation, selected Cards

**Activated Profession Ability (發動能力)**:
A Profession Ability that its Player may deliberately use during the Active
Effects Process without consuming the action opportunity. It becomes activated
only after all required inputs are accepted, consuming the shared once-per-turn
activation allowance; uncommitted input selection does neither.
_Avoid_: Action Command, Formation Use, started input selection

**Prepared Profession Ability**:
The declared, turn-scoped result of an Activated Profession Ability that changes
how one specified physical Card Instance may be interpreted or used by the
Player's subsequent action.
_Avoid_: Virtual Formation Card, Card Definition mutation, hidden draft

**Printed Card Level (牌面等級)**:
The immutable one-through-five level belonging to a Card Definition. Only rules
that explicitly name the printed level read it instead of the effective level.
_Avoid_: base level, raw level

**Effective Card Level (有效等級)**:
The bounded one-through-five level exposed after every applicable Card
Interpretation Layer has composed. An unqualified reference to a Card's level
means its Effective Card Level.
_Avoid_: Printed Card Level, unbounded composed level

**Card Level Interpretation**:
A Card Interpretation Layer that either assigns an in-range level or adjusts
the currently composed level. Applicable level interpretations compose in
effect order before their final result becomes the Effective Card Level.
_Avoid_: Card Definition mutation, level counter

**Card Interpretation Layer**:
A rule-scoped change to one or more effective dimensions of a physical Card
Instance, such as element or level. Eligible layers compose in effect order
before their effective facts are exposed, while unrelated dimensions continue
to compose.
_Avoid_: Virtual Formation Card, Card Definition mutation

**Sacred Art Multiplicity (聖術視為兩張)**:
The Saint's ability to let one eligible physical Card fill two match slots when
forming a Base Ruleset four-Card Formation. It cannot use Star Element
Substitution, and the Card still moves and contributes to ordinary level sums
only once.
_Avoid_: cloned Card, independently interpreted slots

**Void Reversion Technique (虛空返璞術)**:
The Hero Schools Formation that costs its performing Player's Team 20 HP and
breaks every non-Legendary Profession, or every Profession when made from
level-three-or-higher Cards.
_Avoid_: Return to Origin, 歸元, Void Meridian-Severing Technique

**Sacred Beast (聖獸)**:
An 81-point elemental attack formed from five Cards of one element under the
Five Directions Legend Rule Module. It ignores other Formation effects and
changes the Environment to its element after the attack resolves.
_Avoid_: divine beast, guardian, uncounterable Formation

**Environment (環境)**:
The single shared elemental condition affecting all Players under the Five
Directions Legend Rule Module. A game starts without one Environment, and a new
Environment replaces the previous one.
_Avoid_: field, battlefield, player environment, Status Effect

**Environment Transfer (轉移環境)**:
The replacement of the current Environment with a specified elemental
Environment. A Sacred Beast transfers the Environment only after its attack has
resolved under the previously existing Environment.
_Avoid_: environment mutation, pre-attack environment change

**Environment Effect (環境效果)**:
A rule imposed by the current Environment that modifies elemental combat or
makes specified Formations ineffective for every Player.
_Avoid_: Status Effect, player buff, five-element interaction

**Environment-Element Card (環行牌)**:
A Card Instance whose printed element matches the current Environment when the
rule checks it. There is no Environment-Element Card while no Environment
exists.
_Avoid_: interpreted-element Card, Card matching the previous Environment

**Environment Clearing (破除環境)**:
The successful removal of the current Environment, returning the game to no
Environment. Void Meridian-Severing Technique clears only an Environment that
exists and then changes both Teams' HP simultaneously.
_Avoid_: environment expiry, Environment Transfer

**Void Meridian-Severing Technique (虛空斷脈術)**:
An Active Spell formed from three same-level Cards that clears the Environment.
A successful Environment Clearing reduces each Team's HP by 20 simultaneously.
_Avoid_: environment reset, player damage, Shield damage

**Discard Retrieval (棄牌回收)**:
An Optional Rule Module that lets the current Player pay HP during the
active-effect timing to return the Previous Player's Discarded Card from the
Previous Turn to the top of the current Player's Deck.
_Avoid_: discard recycling, turn-draw discard

**Discard Shuffle (洗棄牌)**:
The complete random reordering of an applicable Discard Pile when its Deck has
too few Cards for a required operation, followed by placing every shuffled Card
at the bottom of that same Deck.
_Avoid_: Discard Retrieval, Deck Shuffle, discard recycling

**Deck (牌堆)**:
The ordered hidden pile used for draws and top-or-bottom Card movement. An
unqualified rule reference to 牌堆 means the resolving Player's applicable Deck;
another Player's Deck must be identified explicitly.
_Avoid_: Deck List, Discard Pile, target Player's Deck by default

**Personal Deck (個人牌組)**:
An Optional Rule Module under which each Player prepares and draws from their
own 60-card Deck instead of all Players sharing one Deck.
_Avoid_: shared deck, team deck

**Deck Composition (牌組構成)**:
The rule-defined multiset of Card Definitions used to prepare a Deck before
shuffling. The shared Deck has one fixed official 90-card composition, while a
Personal Deck uses one valid 60-card Deck List.
_Avoid_: shuffled Deck order, Card Instance sequence

**Game Record**:
The canonical persisted game history made from setup and accepted setup, command, and automatic advancement decisions.
_Avoid_: save file, snapshot

**Game State**:
The current projection derived from a **Game Record**.
_Avoid_: record, save

**Game Event**:
A canonical fact emitted by accepted setup, command, or automatic advancement and used for replay.
_Avoid_: log message, notification

**Game Outcome (遊戲結果)**:
The terminal winner or Draw determined after all deltas in the applicable
simultaneous resolution have been applied.
_Avoid_: Game End Cause, score, inferred UI result

**Game Conclusion (遊戲結論)**:
The canonical terminal fact containing the Game Outcome and one or more Game
End Causes. It persists independently of Card zones and previous-Turn queries.
_Avoid_: final board, last Formation, result presentation

**Game End Cause (遊戲結束原因)**:
A typed reason the Game ended, pairing a terminal condition with the Formation
Use or Rule Effect that caused it. Simultaneous causes may produce a Draw.
_Avoid_: Formation Area snapshot, Previous-Turn Formation, inferred cause

**Validation Failure**:
A rejected command or setup because the submitted player/input data is illegal for the current rules and state.
_Avoid_: rule bug

**Rule Implementation Error**:
A failure caused by an incomplete or internally inconsistent rule module after input validation has succeeded.
_Avoid_: validation failure

**Engine Invariant Error**:
A failure caused by an impossible engine state or violated core invariant.
_Avoid_: player error

**Public View**:
A viewer-filtered representation of canonical game data for API or presentation use.
_Avoid_: replay source

**Public State View**:
A viewer-filtered snapshot of game state.
_Avoid_: canonical state

**Public Event Feed**:
A viewer-filtered event stream derived from canonical events.
_Avoid_: canonical event log

**Card Move Delta**:
An explicit recorded movement of a card instance from one zone to another.
_Avoid_: inferred movement

**Discard (捨棄)**:
The action of moving a used or unwanted Card Instance to the Discard Pile.
_Avoid_: 棄置

**Discard Pile (棄牌堆)**:
The public zone containing discarded Card Instances, owned by the shared game or
one Player according to the enabled Rule Modules. A card selected under the Turn
Draw discard rule is a **Discarded Card (棄牌)**.
_Avoid_: 捨棄區

**Card Instance**:
A movable individual card in a game zone.
_Avoid_: card definition

**Card Origin**:
The immutable source of a Card Instance: the shared deck or the Player whose
Personal Deck originally contained it. Moving or retrieving a card does not
change its Card Origin.
_Avoid_: current holder, current zone

**Pile Owner**:
The shared game or Player whose Deck or Discard Pile a zone represents. A pile
may temporarily contain a Card Instance with a different Card Origin.
_Avoid_: Card Origin, current holder

**Deck List**:
The exact 60 Card Definitions selected by one Player before a Personal Deck
game. It describes deck composition, not shuffled Card Instance order.
_Avoid_: prepared deck order, hand

**Preconstructed Deck List (預組牌組)**:
The built-in legal Deck List used when Personal Deck is enabled and a Player has
no valid custom Deck List. For each element it contains 3/2/3/2/2 cards of
levels 1/2/3/4/5 respectively, totaling 60 cards and 170 levels.
_Avoid_: shared deck, shuffled deck

**Locked Deck List**:
The validated Deck List captured for a non-owner when they become ready, or for
the room owner when they start a Personal Deck game. It remains that game's
setup input even if the Player later edits their account Deck List, and records
the display name actually selected after fallback.
_Avoid_: live account deck, prepared deck order

**Retrieved Card (回收牌)**:
A Retrievable Discard moved by Discard Retrieval into the current Player's
Deck. Under Personal Deck it is an Exposed Foreign Card when its Card Origin is
another Player.
_Avoid_: owned card, hidden card

**Exposed Foreign Card**:
A Card Instance placed in a Player's Personal Deck whose Card Origin is another
Player. It remains publicly revealed while in that Deck or hand and returns to
its origin Player's Discard Pile when used or discarded.
_Avoid_: owned card, hidden card

**Retrievable Discard**:
The Previous Player's Turn Draw Discarded Card from the immediately completed
Previous Turn. It is the sole card eligible for the current Player's Discard
Retrieval.
_Avoid_: top discarded card, formation cards

**Residual Element (餘行)**:
The turn-scoped fact fixed from the printed element of the Previous Player's
Turn Draw Discarded Card when that Card is Discarded. It remains unchanged
until the current Player reaches Turn Draw, even if the Card moves.
_Avoid_: Previous Formation element, interpreted element

**Residual Level (餘級)**:
The turn-scoped fact fixed from the printed level of the Previous Player's Turn
Draw Discarded Card when that Card is Discarded. It remains unchanged until the
current Player reaches Turn Draw, even if the Card moves.
_Avoid_: Previous Formation level, interpreted level

**Card Definition**:
Immutable printed-card data such as name, one five-element element, and level from 1 to 5.
_Avoid_: card instance

**Card Back**:
The viewer-safe appearance of a hidden Card Instance. It conveys no element, level, owner, or other game information and may vary cosmetically.
_Avoid_: hidden-card label, element mark

**Player**:
A seat participant in turn order.
_Avoid_: user, account

**Local Password Reset**:
A development-only password change available from a local Worker entry. It does
not verify email ownership, requires an explicit local-reset enablement flag,
reports whether the email is absent, has no resettable password credential, or
was reset, creates a new session while preserving existing sessions, and is
unavailable outside enabled local development. It will be replaced by an emailed
reset flow outside local development.
_Avoid_: production password reset, email password reset

**Team**:
The HP-owning side that one or more players belong to.
_Avoid_: player HP owner

**Affected Player Set (受影響玩家集合)**:
The Players to whom a resolved effect applies. A Player target contributes that
Player only, while a Team target such as 我方, 對方, or 雙方 contributes every
Player on the targeted Team even when HP is stored as one shared Team value.
_Avoid_: every Player sharing an HP delta, declared target text

**Star (星辰)**:
A persistent elemental power owned by a Team when the Star Rule Module is
enabled. A Team may own at most one Star, and each Star may be owned by at most
one Team.
_Avoid_: player star, status effect

**Star Summoning (召喚星辰)**:
The outcome of a qualifying use of 鍠金, 樸木, 洄水, 熾火, or 坱土 that grants
its associated Star to the attacking Player's Team and records that Player as
its summoner. Qualification uses Attack points before elemental interaction,
Counter Effects, Shields, or HP resolution change the outcome; Star Formations
and copied effects do not qualify.
_Avoid_: star pickup, team summon

**Star Formation (星辰技)**:
A Formation made available by the Star owned by the performing Player's Team.
It is distinct from a Base Ruleset Formation.
_Avoid_: base formation, skill

**Star Element Substitution (星辰變牌)**:
The Star-granted ability to treat one matching generating-element Card Instance
as the Star's element when forming a Base Ruleset Formation. It is an implicit
matching rule and does not change the Card Instance or its Card Definition.
_Avoid_: card mutation, universal element change

**Temporary Star Effect**:
A turn-scoped grant of one specified Star's Star Element Substitution and Star
Formations without owning or summoning that Star.
_Avoid_: temporary Star, Star Summoning, Five-Star Alignment progress

**Star-Element Card (星行牌)**:
A Card Instance whose printed element matches the Star currently owned by the
relevant Player's Team. There is no Star-Element Card while that Team owns no
Star.
_Avoid_: Star Element Substitution, interpreted-element Card

**Star Breaking (破除星辰)**:
The removal of a Team's currently owned Star.
_Avoid_: discard star, expire star

**Five-Star Alignment (五星連珠)**:
The Star Rule Module victory condition achieved when one Player has personally
summoned all five kinds of Star. After the Formation Use fully resolves, this
condition makes that Player's Team the winner before HP-based outcome evaluation.
_Avoid_: team star collection

**Shield (防護罩)**:
A player-owned persistent value that takes damage in place of the owning Player.
Each Player may have at most one Shield.
_Avoid_: 護盾, team shield

**Turn Order**:
The circular player sequence used to decide the current player, previous player, next player, and passive trigger relationships.
_Avoid_: team order

**Turn (回合)**:
One Player's complete turn-flow from Turn Start through Turn End. For a
time-limited effect measured in Turns, one Turn elapses only when the affected
Player reaches Turn End; other Players' Turns do not decrement it.
_Avoid_: global turn number, table rotation

**Round (輪)**:
One affected Player's Turn Start boundary for a time-limited effect measured in
Rounds. It is distinct from a Turn because it elapses at that Player's Turn
Start, not Turn End, and is not a count of every Player taking a Turn.
_Avoid_: full-table rotation, global round counter

**Turn Start (回合開始)**:
The opening timing of a Player's turn, when due expirations resolve before
delayed rule effects. All Turn Start effects finish before the Active Effects
Process.
_Avoid_: start of Active Effects Process

**Previous Player**:
The player immediately before the current player in turn order.
_Avoid_: opponent, enemy

**Previous Turn**:
The immediately completed Player turn before the current turn. It is not the previous round or the Previous Player's older history.
_Avoid_: previous round, previous formation history

**Previous-Turn Formation**:
The Formation performed by the Previous Player during the immediately completed Previous Turn. It does not exist when that Player passed or performed no Formation during that turn. A Passive Spell belongs to the turn when it was covered, not the later turn when it is revealed. Triggering an existing Formation's effect, such as Echo or Plant Earth at Turn Start, does not perform that Formation again.
_Avoid_: latest formation, last known formation, formation history

**Previous-Turn Elemental Attack**:
The Previous-Turn Formation when that Formation is a Five-Element Attack. It does not exist when the Previous-Turn Formation is absent or belongs to another category.
_Avoid_: latest elemental attack, last known element, elemental history

**Formation**:
A declared combination of card instances that matches a formation pattern and resolves through an effect definition.
_Avoid_: combo, hand pattern

**Formation Catalog**:
The complete set of Formations contributed by the Base Ruleset and enabled Rule
Modules, independent of whether a Player can currently perform them.
_Avoid_: playable actions, current hand matches

**Rule Consequence**:
A closed typed Player-visible fact that can change the Player's understanding
of one rule offer or choice, such as a concrete cost, effect, follow-up, timing,
current limit, or material exception. It excludes Action identity, universal
rules, vague caveats, engine-lifecycle distinctions, arbitrary prose, Game
Events, and predictions of the final Game State.
_Avoid_: summary string, Game Event, simulated outcome, engine diagnostic

**Player-Facing Action Detail**:
An optional Player-scoped, state-specific supplement containing only the Rule
Consequences needed beyond an offered Action's accessible label and visible
decision context. The Rules Engine owns which supplemental facts hold; the Web
owns concise localized composition that states each Player-visible fact once,
uses official game terms, and orders related clauses by dependency and timing.
_Avoid_: raw rule_text, tooltip copy, choice payload, repeated Action identity

**Action Input Requirement**:
A typed description of the Player input still required to turn one offered
Action into a complete command. It is pre-commit offer data and does not create
a Pending Choice or canonical waiting state.
_Avoid_: Pending Choice, Rule Consequence, command draft

**Formation Use**:
The accepted use of a Formation Composition, including its semantic resolution
and explicit movement of its physical Cards.
_Avoid_: implicit card discard

**Formation Use Commitment (陣法施展承諾)**:
The irreversible boundary reached when a complete Formation command passes
validation and its physical Cards enter the performing Player's Formation Area.
Later prevention or ineffectiveness changes only the resolution outcome.
_Avoid_: Action Card Selection, Pending Choice, effect completion

**Ineffective Formation (陣法效果無效)**:
An accepted Formation Use whose Formation effects do not execute. It still
consumes its action and Cards and satisfies rules based only on performing it.
_Avoid_: Validation Failure, unperformed Formation

**No-Effect Ground (無效依據)**:
A rule fact independently sufficient to make an otherwise applicable resolving
effect have no effect. Multiple grounds may hold simultaneously without
creating repeated outcomes or imposing a rule priority among those grounds.
_Avoid_: first cause, cancellation order, Validation Failure

**Formation Category**:
The official top-level kind of a formation: attack or spell.
_Avoid_: elemental attack category, active spell category, passive spell category

**Effect Plan**:
The execution plan linked from a formation effect definition, such as attack resolution, immediate spell resolution, or covered passive resolution.
_Avoid_: formation behavior

**Attack Plan**:
The attack execution detail that distinguishes elemental, physical, and special attacks.
_Avoid_: attack category

**Attack**:
A formation category that targets the previous player first and resolves HP impact through that player's team.
_Avoid_: damage spell

**Attack Resolution (攻擊解析)**:
The simultaneous result of an Attack's damage and every attached effect that
has no separately specified timing. Its consequences have no internal
before-and-after order.
_Avoid_: damage step, ordered attack effects, Attack completion

**Spell**:
A formation category whose effect may resolve immediately or be covered as a passive.
_Avoid_: effect, skill

**Spell Type**:
The active or passive procedure used to perform a Spell. Spell Type determines
whether submitted cards are shown and resolved immediately or placed face down;
it is distinct from Formation Category and is not copied by class change.
_Avoid_: spell category

**Covered Passive**:
A passive Spell waiting face down in the Formation Area until the next Player's
action start. If neutralized before that timing, it remains hidden until its
ordinary reveal and Discard.
_Avoid_: trap, secret

**Counter Effect**:
A delayed defensive Formation effect that checks and may modify the next Player's
action. Passive Spells create hidden Counter Effects through Covered Passives;
class change may create the copied Counter Effect publicly without covered cards.
_Avoid_: covered passive effect

**Resolved Formation Effect**:
The Formation Category and effect behavior actually adopted by a Formation Use,
stored separately from the performed Formation's identity. Class change keeps its
own Formation identity while copying a previous Resolved Formation Effect.
_Avoid_: displayed formation, formation id

**Status Effect**:
A rule-recognized ongoing effect attached to a player or team.
_Avoid_: arbitrary tag

**Timed Formation Effect (時效性陣法效果)**:
A Formation effect whose remaining duration or layer count can expire and can
be shortened by rules such as 變徵‧淨火.
_Avoid_: every Status Effect, permanent effect

**Flow State (流水狀態)**:
A stackable Player-owned Timed Formation Effect that may spend at most one layer
per Turn Draw to increase a draw that would otherwise not fill the hand.
_Avoid_: permanent draw bonus, per-draw trigger

**Status Kind**:
The rule-recognized type of a status effect.
_Avoid_: string metadata

**Jianghu State (江湖狀態)**:
The official collective term used only for the 千鋒, 踏雪, and 中毒 ongoing
Formation effects defined by the Jianghu Rule Module. Similar ongoing effects
from other rules are not Jianghu States.
_Avoid_: generic Status Effect, all ongoing Formation effects

**Limited Use (次數限制)**:
A bounded allowance attached to a specified Formation or Profession Ability
under the Confluence Generation Rule Module. Exhausted uses remain unavailable
until the published recovery or Profession reacquisition condition resets them.
_Avoid_: per-turn allowance, cooldown

**Dark Formation (暗黑陣法)**:
One of 暗黑光芒, 暗黑氣壁, 暗黑歸元, 暗黑震暴, 暗黑混沌, or 暗行輪迴 under
the Dark Glimmer Rule Module.
_Avoid_: every Dark Glimmer Formation, ordinary elemental Formation

**Persistent Spirit Skill (常駐技能)**:
A Spirit Skill that remains continuously in effect while its granting Spirit
is owned and does not require active use.
_Avoid_: activated Spirit Skill, Automatic Profession Ability

**Cannot Act**:
A status kind that prevents a player from taking an action command.
_Avoid_: stunned, disabled

**Active Effects Process (主動效果流程)**:
The fixed Turn process after Turn Start and before Action in which the current
Player may use zero or more Active-Effect Commands. The first accepted Action
Command ends this process without a separate completion Command.
_Avoid_: Main Phase, active window

**Action Process (行動流程)**:
The fixed Turn process after active-effect timing and before Turn Draw in which
exactly one Action is committed and completed. Choices required by a Formation
Use pause inside this process rather than creating another Turn phase.
_Avoid_: Formation Resolution phase, Pending Choice phase

**Turn Draw (回合抽牌)**:
The mandatory draw step in the turn flow. Drawn Cards enter the Turn Draw Pool,
one is Discarded, and only the remaining Cards then enter the Player's hand.
_Avoid_: 抽牌選擇

**Turn Draw Pool (回合抽牌區)**:
The transient Card zone holding Cards removed from a Deck during Turn Draw
until one is Discarded and all remaining Cards enter the Player's hand. The
Game has at most one Turn Draw Pool because only the current Turn Draw may be
unresolved.
_Avoid_: temporary hand, drawn hand, Pending Choice

**Active-Effect Command**:
A non-Formation Player ability Command that may be used during the Active
Effects Process without committing the Player's Action.
_Avoid_: formation action

**Action Command**:
A Command that ends the Active Effects Process and commits the Player's one
Action for the Turn.
_Avoid_: active-effect command

**Action Pass**:
An Action Command that completes the Action Process without another action when
the Rules Engine permits it. It remains an explicit Command whether a Player
submits it to decline optional Active-Effect Commands or the Rules Engine
selects it as the sole Playable Action, and its Playable Action carries the
authoritative Pass reason.
_Avoid_: Skip button, Automatic Decision

**Pending Choice**:
A serialized waiting state requiring one closed typed Player answer before
deterministic resolution can continue. Initial Pouch Selection remains a Game
Preparation stage rather than a Pending Choice.
_Avoid_: prompt, callback

**Choice ID**:
The stable canonical identity of one Pending Choice. A Player answer references
the Choice ID so it cannot answer a later choice with the same visible shape.
_Avoid_: UI key, payload hash

**Choice Continuation**:
The typed canonical instruction attached to a Pending Choice that tells the
Rules Engine which rule flow resumes after the Player's answer is validated.
_Avoid_: effect ID and continuation string pair, application callback

**Pending Randomness**:
A serialized waiting state requiring a trusted application adapter to supply a
rule-authorized random result before deterministic resolution can continue.
_Avoid_: Pending Choice, client-provided shuffle

**Randomness Continuation**:
The typed canonical instruction attached to Pending Randomness that tells the
Rules Engine how to validate the supplied random result and resume deterministic
resolution after the trusted application adapter answers.
_Avoid_: continuation string, application callback

**Choice Requested**:
A Game Event that creates one Pending Choice, including its Choice ID and
Choice Continuation, for one Player.
_Avoid_: UI prompt

**Choice Made**:
A Game Event that records one Player's closed typed answer to a Pending Choice
and clears that waiting state. Rule consequences remain separate Game Events.
_Avoid_: callback response

## Relationships

- A **Player** belongs to exactly one **Team**
- A **Player** starts without a **Profession** and owns at most one
- A successful **Profession Change** replaces the Player's previous **Profession**
- **Profession Breaking** removes a Player's current **Profession** without replacement
- A **Profession** grants Automatic Profession Abilities, Formation
  Proficiencies, and Activated Profession Abilities
- A successful **Activated Profession Ability** use consumes that Player's
  shared once-per-turn activation allowance
- A **Prepared Profession Ability** expires after the Player's action or at the
  end of that turn
- A **Formation Requirement** permits non-action-ending effects beforehand but
  prevents the Player from ending their action opportunity without an allowed
  **Formation Use**
- A **Virtual Formation Card** contributes its fixed element and level to one
  **Formation Composition** but never moves between Card zones
- A **Team** owns HP for one or more **Players**
- A **Player** owns at most one **Shield**
- Every **Card Instance** has exactly one immutable **Card Origin**
- A **Card Instance's** current zone does not change its **Card Origin**
- A **Pouch** has exactly one **Pouch Owner**, which may differ from its Card
  Origin
- Every Deck and **Discard Pile** has exactly one **Pile Owner**
- A **Deck List** contains exactly 60 Card Definitions with total level at most
  170 and official per-definition copy limits
- **Personal Deck** uses a Player's valid custom **Deck List**, or automatically
  substitutes the **Preconstructed Deck List** when the custom list is absent or
  invalid
- Each Player in a ready Personal Deck room has one **Locked Deck List**
- A **Locked Deck List** is private to its Player; other Players see readiness,
  not its name, contents, or whether it came from a custom or Preconstructed
  Deck List
- Under **Personal Deck**, every Player's Deck count and Discard Pile contents
  are public, while Deck order and ordinary opposing hands remain hidden
- An **Exposed Foreign Card** remains fully public even while in a Deck or
  opposing hand, including its position among otherwise hidden Deck cards
- Changing the room's enabled **Rule Modules** invalidates non-owner readiness
  and all waiting-room **Locked Deck Lists**
- New official game configurations enable every available **Rule Module** by
  default; the **Base Ruleset** remains mandatory
- A used or discarded **Exposed Foreign Card** returns to its origin Player's
  **Discard Pile**
- **Discard Retrieval** derives exactly one **Retrievable Discard** from
  canonical turn history rather than a Player-submitted card choice
- Residual Element and Residual Level remain available for Tuner transitions
  and Formations even after their source Card stops being a Retrievable Discard;
  調律 and 天響 additionally require that physical Card to remain retrievable
- 天響 moves the Retrievable Discard into its Player's hand without creating a
  Tuning Card Obligation or changing the active Residual Element and Residual
  Level; the Card may be freely kept or used
- 調律 and 天響 are both Activated Profession Abilities and compete for the
  same once-per-turn Activated Profession Ability allowance
- 天響 has one use per acquisition of 天響師; reacquiring 天響師 resets that
  use, while Turn changes and shuffles do not
- Residual Element and Residual Level expire when their Player reaches Turn
  Draw; if that Turn Draw produces no Turn Draw Discard, the Next Player
  receives no new residual facts
- A **Tuning Card Obligation** permits non-action-ending abilities beforehand,
  but no Action may complete without consuming the Tuning Card through an
  allowed Action
- 調律 is available when at least one legal sequence of the Player's remaining
  non-action-ending abilities can lead to an action that fulfills the resulting
  Tuning Card Obligation; immediate post-調律 legality is not required
- While a Tuning Card Obligation is active, a voluntary non-action-ending
  option is available only when its projected result retains at least one legal
  completion path; consuming one of several alternatives remains legal
- The Rules Engine, not the UI, authoritatively rejects any command that would
  remove every completion path for an active Tuning Card Obligation
- A Tuning Card Obligation is fulfilled when the Card participates in an
  accepted allowed action, even if that Profession Change or Profession
  Formation is later ineffective or cancelled
- The Tuner Profession System's Residual-Element transition values 3, 6, and 9
  are minimum total levels, not exact totals
- Residual Level is fixed from the Retrievable Discard's printed level, while a
  submitted Card satisfies a Residual-Level-Card slot using its effective level
  after every applicable Card Interpretation Layer
- 調律 compares its Discarded cost Card's effective level against the printed
  Residual Level
- A physical Card fills only one Formation match slot unless a rule explicitly
  grants multiplicity; the element-Card and Residual-Level-Card slots of a
  five-resonance Formation must therefore use different Card Instances
- 千鳴 normally resolves its Residual-Element resonance and selected resonance
  simultaneously; when a choice prevents simultaneous handling, the complete
  Residual-Element resonance continuation finishes before the selected
  resonance begins
- Game Outcome evaluation waits until every 千鳴 resonance and pending choice
  has completed; a lethal earlier resonance does not cancel the later resonance
- 萬鳴 applies its deterministic HP, Shield, and Turn Draw effects before
  entering the 鏡鳴 hand-inspection and Discard choice continuation
- A lethal deterministic 萬鳴 effect does not finalize Game Outcome or block
  commands until its 鏡鳴 continuation has completed
- 煌鳴 and every composite resonance that includes it affect only the Previous
  Player; that Player's teammates share the Team HP loss but do not join the
  Affected Player Set
- 森鳴 and 萬鳴 recover their performing Player's Team HP, but Formation
  recovery restrictions are evaluated only on that performing Player; an
  affected teammate does not block the recovery
- An **Echo Cost** is an ordinary **Discard**, not a Turn Draw Discard, and
  therefore neither charges a Spirit nor becomes a **Retrievable Discard**
- An **Echo Cost** reads a Card's printed element unless the granting rule
  explicitly extends a **Card Interpretation Layer** beyond Formation matching
- Under **Personal Deck**, effects that return cards to the top of a Deck use
  the performing Player's Deck and create **Exposed Foreign Cards** when origins
  differ
- 商調‧鳴金 searches and shuffles the performing Player's current Deck under
  **Personal Deck**, or the shared Deck otherwise, regardless of **Card Origin**
- If that Deck is empty, 商調‧鳴金 performs the ordinary pile-scoped Discard
  recycling before searching it
- A **Game Record** uses exactly one **Ruleset**
- Every **Ruleset** includes the **Base Ruleset**
- A **Ruleset** may enable zero or more **Rule Modules**
- **Advanced Rule Modules** and **Optional Rule Modules** share one Ruleset
  configuration and execution model
- The **Base Ruleset** supports both two-player and team-mode setup shapes
- The **Star Rule Module** adds Star-specific rules without replacing the Base Ruleset
- The **Hero Schools Rule Module** adds Profession-specific rules without replacing the Base Ruleset
- The **Five Directions Legend Rule Module** adds five **Sacred Beasts**, one
  shared **Environment**, and **Void Meridian-Severing Technique**
- A game has at most one **Environment**, shared by every **Player**
- Every **Sacred Beast** completes an **Environment Transfer** after its Attack resolves
- **Environment Clearing** changes each **Team's** HP once, regardless of Player count
- A **Team** owns at most one **Star**
- A **Star** is owned by at most one **Team**
- **Star Summoning** belongs to one **Player** and grants the Star to that Player's **Team**
- A **Star Formation** is available only through the performing Player's Team-owned **Star**
- An accepted three-card **Star Formation** breaks its enabling **Star** and grants
  its Turn Draw bonus even when its damage is prevented
- **Star Element Substitution** applies only while matching a Base Ruleset Formation
- **Five-Star Alignment** uses one Player's Star Summoning history, not the Team's combined history
- **Star Breaking** removes the affected Team's currently owned **Star**
- **Turn Order** is player-based, not team-based
- The **Previous Player** is derived from **Turn Order**
- A **Formation** has exactly one **Formation Category**
- A **Formation** resolves through exactly one **Effect Plan**
- A Formation declaration with multiple result-changing interpretations
  requires one explicit **Formation Match Option**
- A **Spell** has one **Spell Type** that controls its performance procedure
- A **Formation Use** retains its Formation identity separately from its **Resolved Formation Effect**
- An **Echo** may be scheduled only after its Melody main effect executes; a
  no-change result remains eligible, while an **Ineffective Formation** does not
- **Scheduled Echo** and 變宮‧植土's fixed next-Turn-Start schedule have no
  reducible duration or layer count and are not shortened by 變徵‧淨火
- **Echo** and 變宮‧植土 execute Melody main effects during **Turn Start**
  without creating a **Formation Use** or triggering a **Covered Passive**
- **Echo** and 變宮‧植土 do not update the previous **Formation Use** or satisfy
  rules that require performing a Formation
- 宮調‧裂土 makes only a matching **Formation Use** ineffective; it does not
  suppress the same Melody main effect when executed by **Echo** or 變宮‧植土
- A validated **Formation Use** reaches **Formation Use Commitment** before its
  effects resolve and moves its physical **Card Instances** from hand to its
  Player's **Formation Area**
- Each **Player** owns one **Formation Area**, which contains at most one
  **Formation**
- An ordinarily completed face-up **Formation Use** moves its physical **Card
  Instances** from the **Formation Area** to the applicable **Discard Piles**
- A **Covered Passive** is a face-down **Formation** state in its Player's
  **Formation Area**, not another Card zone
- A **Card Move Delta** moves one **Card Instance**
- **Discard** moves a **Card Instance** to the **Discard Pile**
- A **Card Instance** refers to one **Card Definition**
- A **Card Definition** has exactly one element: metal, wood, water, fire, or earth
- A **Card Definition** has a level from 1 to 5
- An **Attack Plan** may be elemental, physical, or special without changing the **Formation Category**
- An **Attack** targets a **Previous Player** before resolving HP impact to that player's **Team**
- An **Attack Resolution** contains the **Attack** damage and every attached
  effect without another specified timing, all without an internal order
- The **Active Effects Process** may accept multiple **Active-Effect Commands**
  before one **Action Command** begins the **Action Process**
- Every legal Active Effects Process option is classified by the Rules Engine as either an
  **Active-Effect Command** or an **Action Command**; presentation may group
  those options but does not reclassify them
- **Playable Actions** include every legal Active Effects Process Command contributed by
  the Base Ruleset and enabled Rule Modules, independent of where presentation
  places the corresponding controls
- Trusted-randomness command variants continue an already chosen Formation or
  Spirit Skill through **Pending Randomness**; they are not separate
  **Playable Actions**
- Public presentation derives Active Effects Process controls only from **Playable
  Actions** and carries no parallel capability flags for the same legality
- An initial, refreshed, or reconnected Active Effects Process view uses an empty **Action
  Card Selection**; non-empty selections remain uncommitted Player-local input
- An Active Effects Process option that uses selected physical Cards must use exactly the
  current **Action Card Selection**; selection-independent options appear only
  for an empty Action Card Selection
- A legal **Action Pass** is a **Playable Action** only for an empty
  **Action Card Selection**
- An **Action Pass** may coexist with optional **Active-Effect Commands** but
  never with another **Action Command**
- An Active Effects Process whose sole **Playable Action** is an **Action Pass** contains no
  Player decision and advances by applying that explicit Command; every sole
  non-pass Playable Action remains a Player decision
- The sole Action Pass condition is derived from the same empty-selection
  **Playable Actions** used for Player presentation, never from a parallel
  capability predicate
- A successful **Action Command** ends the **Active Effects Process** and begins
  the **Action Process**
- A **Covered Passive** belongs to one **Player** and flips at the next player's action start
- A **Counter Effect** may be hidden behind a **Covered Passive** or publicly established by class change
- A **Status Effect** has one **Status Kind**
- **Cannot Act** allows an action pass for the affected **Player**
- A **Choice Requested** creates one **Pending Choice**
- A **Choice Made** answers one **Pending Choice**
- A **Pending Choice** has exactly one **Choice ID**
- A **Pending Choice** has exactly one **Choice Continuation**
- A **Game Record** projects **Game Events** into **Game State**
- A **Turn Draw** moves drawn **Card Instances** from a Deck into the one
  game-scoped **Turn Draw Pool** before any of them enter a hand
- Resolving a **Turn Draw** first moves the selected **Card Instance** from the
  **Turn Draw Pool** to its applicable **Discard Pile**, then moves all remaining
  Cards from the Pool to the current Player's hand
- The terminal **Game Event** records one independent **Game Conclusion** with
  at least one explicit **Game End Cause**
- A **Game Conclusion** is not inferred from the **Formation Area** or a
  previous-Turn **Formation Use**
- A **Public View** is derived from canonical data and is not used for replay
- A **Public Event Feed** is derived from **Game Events**
- A **Public State View** is derived from **Game State**
- A **Rule Implementation Error** emits no **Game Events** and does not mutate **Game State**
- A **Validation Failure** emits no **Game Events** and does not mutate **Game State**

## Example Dialogue

> **Dev:** "When a player performs an attack, do they choose an opponent?"
> **Domain expert:** "No. The attack targets the **Previous Player** from **Turn Order**. HP damage then applies to that player's **Team**."

## Flagged Ambiguities

- "opponent" is too vague in team mode; use **Previous Player** when the rule follows seating order, and **Team** only when describing HP ownership.
- Elemental, physical, special, immediate, and passive are execution details, not **Formation Category** values; keep **Formation Category** to **Attack** or **Spell**.
- Active-effect commands are not Formation Actions; do not treat every Command
  accepted during the **Active Effects Process** as an **Action Command**.
- The first version of the base formation engine may define **Active-Effect Command** as a future extension point without implementing concrete commands.
- `used_cards` identifies physical Card Instances used by a **Formation Use**;
  a **Formation Composition** identifies any Virtual Formation Card separately,
  and zone changes still require explicit replayable deltas.
- Turn draw discard and effect-generated selections are both **Pending Choices**, even when events keep more specific semantic names.
- `CannotAct` is the implementation spelling of **Cannot Act**, a canonical **Status Kind**, not arbitrary metadata. It blocks Formation, Profession Change, and activated Profession Ability Commands; the affected Player uses the canonical status-specific Pass instead. Player-Only Secret Protection may make it ineffective only for its triggering Player.
- Public event data is a **Public Event Feed**, not a replayable **Game Event** log.
- A known formation with a legal declaration but missing resolver is a **Rule Implementation Error**, not a **Validation Failure**.
- "card" is ambiguous; use **Card Instance** for zone membership and **Card Definition** for immutable printed-card data.
- Team mode is a setup shape supported by the **Base Ruleset**, not a separate **Ruleset** unless future rule behavior diverges.
