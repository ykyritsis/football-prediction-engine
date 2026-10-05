//! Match statistics: the numbers behind a PES post-match screen.
//!
//! [`MatchStats`] is fed by the simulation as events happen (see
//! [`Simulation::stats`](crate::sim::Simulation::stats)): team totals
//! (possession, shots, passing, tackles, discipline, set pieces) and a line
//! per player who took part, with goals, assists and a 3–10 match rating.
//! Everything is derived from the event stream plus a possession tick, so a
//! game that runs its own simulation can feed it the same way.

use crate::pitch::{TeamSide, HALF_LENGTH};
use crate::sim::MatchEvent;
use crate::state::{MatchState, Period, PlayerId, RestartKind, ShotKind, PLAYER_COUNT};

/// A completed pass this many seconds before a shot is a key pass.
const KEY_PASS_WINDOW: f32 = 4.0;
/// A completed pass this many seconds before a goal is an assist.
const ASSIST_WINDOW: f32 = 10.0;

/// One team's totals.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TeamStats {
    pub goals: u32,
    /// Ticks in possession while the ball was in play.
    pub possession_ticks: u32,
    pub shots: u32,
    pub shots_on_target: u32,
    /// Summed pre-shot expected goals of every shot.
    pub xg: f32,
    pub passes: u32,
    pub passes_completed: u32,
    pub crosses: u32,
    pub through_balls: u32,
    pub long_balls: u32,
    pub tackles: u32,
    pub tackles_won: u32,
    pub interceptions: u32,
    pub clearances: u32,
    pub saves: u32,
    pub fouls: u32,
    pub yellow_cards: u32,
    pub red_cards: u32,
    pub corners: u32,
    pub offsides: u32,
    pub free_kicks: u32,
    pub penalties: u32,
    /// Times this team lost the ball to the other side.
    pub possessions_lost: u32,
    /// Possession won in the attacking third (the pressing team's reward).
    pub regains_final_third: u32,
}

impl TeamStats {
    pub fn pass_accuracy(&self) -> f32 {
        self.passes_completed as f32 / self.passes.max(1) as f32
    }
}

/// One player's appearance. A substitute gets his own line.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerLine {
    pub team: TeamSide,
    pub slot: u8,
    pub name: String,
    /// Match minute he came on (0 for starters) and went off.
    pub on: f32,
    pub off: Option<f32>,
    pub goals: u32,
    pub own_goals: u32,
    pub assists: u32,
    pub shots: u32,
    pub shots_on_target: u32,
    pub key_passes: u32,
    pub passes: u32,
    pub passes_completed: u32,
    pub tackles_won: u32,
    pub tackles_lost: u32,
    pub interceptions: u32,
    pub clearances: u32,
    pub dispossessed: u32,
    pub saves: u32,
    pub goals_conceded: u32,
    pub fouls: u32,
    pub fouled: u32,
    pub yellow_cards: u32,
    pub red_card: bool,
}

impl PlayerLine {
    fn new(team: TeamSide, slot: u8, name: String, on: f32) -> Self {
        PlayerLine {
            team,
            slot,
            name,
            on,
            off: None,
            goals: 0,
            own_goals: 0,
            assists: 0,
            shots: 0,
            shots_on_target: 0,
            key_passes: 0,
            passes: 0,
            passes_completed: 0,
            tackles_won: 0,
            tackles_lost: 0,
            interceptions: 0,
            clearances: 0,
            dispossessed: 0,
            saves: 0,
            goals_conceded: 0,
            fouls: 0,
            fouled: 0,
            yellow_cards: 0,
            red_card: false,
        }
    }

    /// PES-style match rating, 3.0–10.0 with 6.0 for an anonymous game.
    /// Contributions are weighted by involvement, so a substitute who plays
    /// ten minutes is not rated on one misplaced pass.
    pub fn rating(&self) -> f32 {
        // Each category saturates: one busy afternoon of duels should not
        // outweigh a goal.
        let capped = |n: u32, per: f32, cap: f32| (n as f32 * per).min(cap);
        let mut r = 6.0;
        r += 1.0 * self.goals as f32 - 0.8 * self.own_goals as f32;
        r += 0.6 * self.assists as f32 + capped(self.key_passes, 0.15, 0.6);
        r += capped(self.shots_on_target, 0.12, 0.5)
            - capped(self.shots - self.shots_on_target, 0.04, 0.3);
        if self.passes > 0 {
            let volume = (self.passes as f32 / 30.0).min(1.0);
            let accuracy = self.passes_completed as f32 / self.passes as f32;
            r += (accuracy - 0.75) * 1.2 * volume;
        }
        r += capped(self.tackles_won, 0.1, 0.8) - capped(self.tackles_lost, 0.03, 0.3);
        r += capped(self.interceptions, 0.08, 0.5) + capped(self.clearances, 0.03, 0.3);
        r -= capped(self.dispossessed, 0.05, 0.5);
        r += capped(self.saves, 0.3, 1.8) - 0.35 * self.goals_conceded as f32;
        r += capped(self.fouled, 0.05, 0.3) - capped(self.fouls, 0.05, 0.3);
        r -= 0.25 * self.yellow_cards as f32;
        if self.red_card {
            r -= 1.5;
        }
        r.clamp(3.0, 10.0)
    }
}

/// Team totals and player lines for one match.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchStats {
    pub teams: [TeamStats; 2],
    /// Every appearance, starters first, substitutes in order of coming on.
    pub players: Vec<PlayerLine>,
    /// Index into `players` of whoever currently fills each slot.
    current: [usize; PLAYER_COUNT],
    /// Last completed pass: (seconds, from, to).
    last_completed: Option<(f32, PlayerId, PlayerId)>,
    /// Last shooter, credited when the keeper saves.
    last_shot: Option<PlayerId>,
    /// Passer of the pass not yet resolved: it counts as completed if the
    /// next player to control the ball is a team-mate (as in Opta data).
    open_pass: Option<PlayerId>,
}

impl MatchStats {
    /// Starters' names by team, in slot order.
    pub fn new(names: [Vec<String>; 2]) -> Self {
        let mut players = Vec::with_capacity(PLAYER_COUNT + 6);
        let mut current = [0; PLAYER_COUNT];
        for side in TeamSide::BOTH {
            for (slot, name) in names[side.index()].iter().enumerate() {
                current[PlayerId::new(side, slot).index()] = players.len();
                players.push(PlayerLine::new(side, slot as u8, name.clone(), 0.0));
            }
        }
        MatchStats {
            teams: [TeamStats::default(); 2],
            players,
            current,
            last_completed: None,
            last_shot: None,
            open_pass: None,
        }
    }

    pub fn team(&self, side: TeamSide) -> &TeamStats {
        &self.teams[side.index()]
    }

    /// Possession share `0..1` of `side`.
    pub fn possession(&self, side: TeamSide) -> f32 {
        let total = self.teams[0].possession_ticks + self.teams[1].possession_ticks;
        if total == 0 {
            return 0.5;
        }
        self.teams[side.index()].possession_ticks as f32 / total as f32
    }

    /// The line of whoever is in `id`'s slot now.
    pub fn line(&self, id: PlayerId) -> &PlayerLine {
        &self.players[self.current[id.index()]]
    }

    fn line_mut(&mut self, id: PlayerId) -> &mut PlayerLine {
        &mut self.players[self.current[id.index()]]
    }

    /// Highest-rated player, ties to the earlier line.
    pub fn man_of_the_match(&self) -> Option<&PlayerLine> {
        self.players
            .iter()
            .fold(None, |best: Option<&PlayerLine>, p| match best {
                Some(b) if b.rating() >= p.rating() => Some(b),
                _ => Some(p),
            })
    }

    /// Count a tick of possession (only while the ball is in play).
    pub fn tick(&mut self, state: &MatchState) {
        if let Some(from) = self.open_pass {
            if state.restart.is_some() {
                self.open_pass = None;
            } else if let Some(o) = state.ball.owner {
                if o.team() == from.team() && o != from {
                    self.complete(from);
                }
                if o != from {
                    self.open_pass = None;
                }
            }
        }
        if state.restart.is_none() && matches!(state.period, Period::FirstHalf | Period::SecondHalf)
        {
            self.teams[state.possession.index()].possession_ticks += 1;
        }
    }

    /// A substitute named `name` took over `slot` at `minute`.
    pub fn substitution(&mut self, team: TeamSide, slot: usize, name: String, minute: f32) {
        let id = PlayerId::new(team, slot);
        self.line_mut(id).off = Some(minute);
        self.current[id.index()] = self.players.len();
        self.players
            .push(PlayerLine::new(team, slot as u8, name, minute));
    }

    fn complete(&mut self, from: PlayerId) {
        self.teams[from.team().index()].passes_completed += 1;
        self.line_mut(from).passes_completed += 1;
    }

    /// Record an event. `state` is the match right after it, `seconds` the
    /// match clock.
    pub fn record(&mut self, state: &MatchState, seconds: f32, e: &MatchEvent) {
        match *e {
            MatchEvent::Pass { from, kind, .. } => {
                let t = &mut self.teams[from.team().index()];
                t.passes += 1;
                match kind {
                    crate::passing::PassKind::Cross => t.crosses += 1,
                    crate::passing::PassKind::Through => t.through_balls += 1,
                    crate::passing::PassKind::Long => t.long_balls += 1,
                    crate::passing::PassKind::Short => {}
                }
                self.line_mut(from).passes += 1;
                self.open_pass = Some(from);
            }
            MatchEvent::PassCompleted { from, to } => {
                if self.open_pass == Some(from) {
                    self.complete(from);
                    self.open_pass = None;
                }
                self.last_completed = Some((seconds, from, to));
            }
            MatchEvent::Interception { by, .. } => {
                self.teams[by.team().index()].interceptions += 1;
                self.line_mut(by).interceptions += 1;
            }
            MatchEvent::Tackle { by, on, won } => {
                let t = &mut self.teams[by.team().index()];
                t.tackles += 1;
                if won {
                    t.tackles_won += 1;
                    self.line_mut(by).tackles_won += 1;
                    self.line_mut(on).dispossessed += 1;
                } else {
                    self.line_mut(by).tackles_lost += 1;
                }
            }
            MatchEvent::Dispossessed { by, on } => {
                self.line_mut(by).tackles_won += 1;
                self.line_mut(on).dispossessed += 1;
            }
            MatchEvent::Shot { by, xg, kind } => {
                let t = &mut self.teams[by.team().index()];
                t.shots += 1;
                t.xg += xg;
                if kind == ShotKind::Penalty {
                    t.penalties += 1;
                }
                self.line_mut(by).shots += 1;
                self.last_shot = Some(by);
                if let Some((at, from, to)) = self.last_completed {
                    if to == by && from != by && seconds - at <= KEY_PASS_WINDOW {
                        self.line_mut(from).key_passes += 1;
                    }
                }
            }
            MatchEvent::Save { by, .. } => {
                self.teams[by.team().index()].saves += 1;
                self.line_mut(by).saves += 1;
                if let Some(shot) = self.last_shot.take().filter(|s| s.team() != by.team()) {
                    self.teams[shot.team().index()].shots_on_target += 1;
                    self.line_mut(shot).shots_on_target += 1;
                }
            }
            MatchEvent::Goal { team, scorer } => {
                self.teams[team.index()].goals += 1;
                let keeper = state.goalkeeper(team.opponent()).id;
                self.line_mut(keeper).goals_conceded += 1;
                match scorer {
                    Some(s) if s.team() == team => {
                        self.teams[team.index()].shots_on_target += 1;
                        let line = self.line_mut(s);
                        line.goals += 1;
                        line.shots_on_target += 1;
                        if let Some((at, from, to)) = self.last_completed {
                            if to == s && from != s && seconds - at <= ASSIST_WINDOW {
                                self.line_mut(from).assists += 1;
                            }
                        }
                    }
                    Some(s) => self.line_mut(s).own_goals += 1,
                    None => {}
                }
                self.last_completed = None;
            }
            MatchEvent::Clearance { by } => {
                self.teams[by.team().index()].clearances += 1;
                self.line_mut(by).clearances += 1;
            }
            MatchEvent::Offside { player } => {
                self.teams[player.team().index()].offsides += 1;
            }
            MatchEvent::OutOfPlay { restart, team } => match restart {
                RestartKind::Corner => self.teams[team.index()].corners += 1,
                RestartKind::FreeKick | RestartKind::IndirectFreeKick => {
                    self.teams[team.index()].free_kicks += 1
                }
                _ => {}
            },
            MatchEvent::PossessionChange { team } => {
                self.teams[team.opponent().index()].possessions_lost += 1;
                if state.restart.is_none() && team.to_local(state.ball.pos).x > HALF_LENGTH / 3.0 {
                    self.teams[team.index()].regains_final_third += 1;
                }
            }
            MatchEvent::Foul { by, on, .. } => {
                self.teams[by.team().index()].fouls += 1;
                self.teams[on.team().index()].free_kicks += 1;
                self.line_mut(by).fouls += 1;
                self.line_mut(on).fouled += 1;
            }
            MatchEvent::YellowCard { player } => {
                self.teams[player.team().index()].yellow_cards += 1;
                self.line_mut(player).yellow_cards += 1;
            }
            MatchEvent::RedCard { player, .. } => {
                self.teams[player.team().index()].red_cards += 1;
                self.line_mut(player).red_card = true;
            }
            _ => {}
        }
    }
}

impl MatchStats {
    /// Plain-text post-match screen: team comparison, then players by rating.
    pub fn report(&self, team_names: [&str; 2]) -> String {
        let [h, a] = &self.teams;
        let mut out = format!(
            "{:>22} {:>3} - {:<3} {}\n",
            team_names[0], h.goals, a.goals, team_names[1]
        );
        let rows: [(&str, String, String); 15] = [
            (
                "Possession",
                pct(self.possession(TeamSide::Home)),
                pct(self.possession(TeamSide::Away)),
            ),
            ("Shots", h.shots.to_string(), a.shots.to_string()),
            (
                "On target",
                h.shots_on_target.to_string(),
                a.shots_on_target.to_string(),
            ),
            ("xG", format!("{:.2}", h.xg), format!("{:.2}", a.xg)),
            ("Passes", h.passes.to_string(), a.passes.to_string()),
            (
                "Pass accuracy",
                pct(h.pass_accuracy()),
                pct(a.pass_accuracy()),
            ),
            ("Crosses", h.crosses.to_string(), a.crosses.to_string()),
            (
                "Through balls",
                h.through_balls.to_string(),
                a.through_balls.to_string(),
            ),
            (
                "Tackles (won)",
                format!("{} ({})", h.tackles, h.tackles_won),
                format!("{} ({})", a.tackles, a.tackles_won),
            ),
            (
                "Interceptions",
                h.interceptions.to_string(),
                a.interceptions.to_string(),
            ),
            ("Saves", h.saves.to_string(), a.saves.to_string()),
            ("Fouls", h.fouls.to_string(), a.fouls.to_string()),
            (
                "Cards (Y/R)",
                format!("{}/{}", h.yellow_cards, h.red_cards),
                format!("{}/{}", a.yellow_cards, a.red_cards),
            ),
            ("Corners", h.corners.to_string(), a.corners.to_string()),
            ("Offsides", h.offsides.to_string(), a.offsides.to_string()),
        ];
        for (label, l, r) in rows {
            out += &format!("{l:>18}  {label:^15}  {r}\n");
        }
        let mut lines: Vec<&PlayerLine> = self.players.iter().collect();
        lines.sort_by(|x, y| crate::math::cmp_f32(y.rating(), x.rating()));
        out += "\nPlayer ratings\n";
        for p in lines {
            let mut notes = String::new();
            for (n, tag) in [(p.goals, "G"), (p.assists, "A"), (p.saves, "Sv")] {
                if n > 0 {
                    notes += &format!(" {n}{tag}");
                }
            }
            if p.on > 0.0 {
                notes += &format!(" (on {:.0}')", p.on);
            }
            out += &format!(
                "  {:>4.1}  {:<4} {}{}\n",
                p.rating(),
                team_names[p.team.index()]
                    .chars()
                    .take(4)
                    .collect::<String>(),
                p.name,
                notes
            );
        }
        out
    }
}

fn pct(x: f32) -> String {
    format!("{:.0}%", 100.0 * x)
}
