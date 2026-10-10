//! Lobby-to-player-slot assignment, as done by Dawn of War II when a match starts.
//!
//! The command stream identifies a player by a sender slot `n` (runtime player
//! ID `1000 + n`), while the replay header lists players by lobby block. The
//! engine maps blocks to slots with a seeded shuffle, reverse-engineered from
//! `GameSetup_AssignPlayersToMapSeats` @ 0x0079df40 in DOW2.exe and the
//! `Random` class (Numerical Recipes `psdes`) in Util.dll:
//!
//! 1. Members are the blocks of kind 0, 1, 3 or 4 (human or AI), in block
//!    order. Teams are the sorted distinct team values of the members, and the
//!    map's team `k` owns engine slots `k * per .. (k + 1) * per`.
//! 2. A `Random` seeded with the match seed shuffles the order of the map
//!    teams when locations are random.
//! 3. Each team then draws from its map team's slots, shuffled when locations
//!    are random. A member takes the slot at its block offset within the team
//!    (`block - 4 * team`) when every member sits in its team's block range,
//!    otherwise the slot at its index among the team's members.
//!
//! Assumption: the map's players form contiguous groups per team. This is only
//! verified for full lobbies, so the mapping is refused (`None`) unless the
//! member count equals the map's player count.

use std::convert::TryFrom;

const C1: [u32; 4] = [0xbaa96887, 0x1e17d32c, 0x03bcdc3c, 0x0f33d1b2];
const C2: [u32; 4] = [0x4b0f3b58, 0xe874f0c3, 0x6955c5a6, 0x55a7ca46];

/// Util.dll's deterministic generator: a counter run through `rounds` of the
/// `psdes` pseudo-DES hash, keyed by the seed.
pub struct Random {
    seed: u32,
    index: u32,
    rounds: u32,
}

impl Random {
    pub fn new(rounds: u32) -> Random {
        Random {
            seed: 0,
            index: 0,
            rounds,
        }
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
        self.index = 0;
    }

    pub fn hash(&mut self) -> u32 {
        let mut l = self.seed;
        let mut r = self.index;
        self.index = self.index.wrapping_add(1);
        for i in 0..self.rounds as usize {
            let t = r ^ C1[i];
            let lo = t & 0xffff;
            let hi = t >> 16;
            let b = (!hi.wrapping_mul(hi)).wrapping_add(lo.wrapping_mul(lo));
            let b = b.rotate_left(16);
            let new_r = (b ^ C2[i]).wrapping_add(hi.wrapping_mul(lo)) ^ l;
            l = r;
            r = new_r;
        }
        r
    }

    pub fn get_max(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            self.hash() % n
        }
    }
}

pub fn random_shuffle<T>(a: &mut [T], rng: &mut Random) {
    for i in 1..a.len() {
        let j = rng.get_max(i as u32 + 1) as usize;
        a.swap(i, j);
    }
}

/// One lobby block from the replay header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyBlock {
    pub block: u8,
    pub kind: u32,
    pub team: u32,
}

/// Returns `(block, slot)` for every member, or `None` when the mapping can't
/// be determined exactly.
pub fn assign_slots(
    blocks: &[LobbyBlock],
    seed: u32,
    random_locations: bool,
    max_players: u32,
) -> Option<Vec<(u8, u8)>> {
    let mut members: Vec<LobbyBlock> = blocks
        .iter()
        .copied()
        .filter(|b| matches!(b.kind, 0 | 1 | 3 | 4))
        .collect();
    members.sort_by_key(|b| b.block);
    if members.is_empty() || members.len() as u32 != max_players {
        return None;
    }

    let mut teams: Vec<u32> = members.iter().map(|b| b.team).collect();
    teams.sort_unstable();
    teams.dedup();
    if members.len() % teams.len() != 0 {
        return None;
    }
    let per = members.len() / teams.len();
    let map_teams: Vec<Vec<usize>> = (0..teams.len())
        .map(|k| (k * per..(k + 1) * per).collect())
        .collect();

    let mut rng = Random::new(4);
    rng.set_seed(seed);
    let mut order: Vec<usize> = (0..teams.len()).collect();
    if random_locations {
        random_shuffle(&mut order, &mut rng);
    }

    let team_pos = |team: u32| teams.iter().position(|&t| t == team).unwrap();
    let by_slot = members.iter().all(|m| {
        let tp = team_pos(m.team);
        let start = 4 * tp;
        let block = m.block as usize;
        start <= block && block < start + map_teams[order[tp]].len()
    });

    let mut assigned = Vec::with_capacity(members.len());
    for (tp, &team) in teams.iter().enumerate() {
        let mut list = map_teams[order[tp]].clone();
        if random_locations {
            random_shuffle(&mut list, &mut rng);
        }
        for (k, m) in members.iter().filter(|m| m.team == team).enumerate() {
            let index = if by_slot { m.block as usize - 4 * tp } else { k };
            let slot = *list.get(index)?;
            assigned.push((m.block, u8::try_from(slot).ok()?));
        }
    }
    Some(assigned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hashes(seed: u32) -> Vec<u32> {
        let mut rng = Random::new(4);
        rng.set_seed(seed);
        (0..4).map(|_| rng.hash()).collect()
    }

    fn block(block: u8, kind: u32, team: u32) -> LobbyBlock {
        LobbyBlock { block, kind, team }
    }

    fn sorted(mut v: Vec<(u8, u8)>) -> Vec<(u8, u8)> {
        v.sort();
        v
    }

    #[test]
    fn hashes_match_reference_values() {
        assert_eq!(hashes(0), vec![0x5b730b80, 0x1146dd47, 0x1a47fc4f, 0x7806d363]);
        assert_eq!(hashes(1), vec![0x02e4d829, 0x509c0c23, 0x161a6415, 0x5293d392]);
        assert_eq!(
            hashes(0xffffffff),
            vec![0xb20f9a00, 0x9ffc1706, 0xf3f918dc, 0xf466bf39]
        );
    }

    #[test]
    fn get_max_is_bounded_hash() {
        let mut rng = Random::new(4);
        rng.set_seed(1445643909);
        let got: Vec<u32> = [2, 3, 2, 3, 3].iter().map(|&n| rng.get_max(n)).collect();
        assert_eq!(got, vec![0, 0, 0, 2, 2]);
        assert_eq!(rng.get_max(0), 0);
    }

    #[test]
    fn shuffle_matches_reference() {
        let mut rng = Random::new(4);
        rng.set_seed(42);
        let mut a = [0, 1, 2, 3, 4, 5];
        random_shuffle(&mut a, &mut rng);
        assert_eq!(a, [4, 3, 1, 2, 0, 5]);
    }

    #[test]
    fn assigns_random_3v3() {
        let blocks = [
            block(0, 3, 0),
            block(1, 3, 0),
            block(2, 3, 0),
            block(3, 5, 0),
            block(4, 0, 1),
            block(5, 3, 1),
            block(6, 3, 1),
            block(7, 5, 1),
        ];
        let got = assign_slots(&blocks, 1445643909, true, 6).unwrap();
        assert_eq!(
            sorted(got),
            vec![(0, 4), (1, 5), (2, 3), (4, 0), (5, 1), (6, 2)]
        );
    }

    #[test]
    fn assigns_random_1v1_with_empty_blocks() {
        let mut blocks = vec![block(0, 0, 0), block(4, 3, 1)];
        for b in [1, 2, 3, 5, 6, 7] {
            blocks.push(block(b, 7, 0));
        }
        let got = assign_slots(&blocks, 1497041141, true, 2).unwrap();
        assert_eq!(sorted(got), vec![(0, 0), (4, 1)]);
    }

    #[test]
    fn fixed_locations_use_identity_order() {
        let blocks = [
            block(0, 3, 0),
            block(1, 3, 0),
            block(2, 3, 0),
            block(4, 3, 1),
            block(5, 3, 1),
            block(6, 3, 1),
        ];
        let got = assign_slots(&blocks, 123, false, 6).unwrap();
        assert_eq!(
            sorted(got),
            vec![(0, 0), (1, 1), (2, 2), (4, 3), (5, 4), (6, 5)]
        );
    }

    #[test]
    fn fixed_locations_with_gapped_team_use_member_order() {
        let blocks = [
            block(0, 3, 0),
            block(1, 3, 0),
            block(2, 7, 0),
            block(3, 3, 0),
            block(4, 3, 1),
            block(5, 3, 1),
            block(6, 3, 1),
        ];
        let got = assign_slots(&blocks, 123, false, 6).unwrap();
        assert_eq!(
            sorted(got),
            vec![(0, 0), (1, 1), (3, 2), (4, 3), (5, 4), (6, 5)]
        );
    }

    #[test]
    fn refuses_undeterminable_lobbies() {
        let blocks = [block(0, 0, 0), block(4, 3, 1)];
        assert_eq!(assign_slots(&blocks, 1, true, 4), None);
        assert_eq!(assign_slots(&[block(0, 7, 0)], 1, true, 0), None);
        assert_eq!(assign_slots(&[], 1, true, 2), None);
    }
}
