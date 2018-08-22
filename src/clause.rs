use std::fmt;

use literal::Literal;
use solver::Variables;

use self::WatchedUpdate::*;

#[derive(Debug)]
pub enum WatchedUpdate {
    NowUnit(Literal),
    NewWatched(Literal),
    AlreadySat,
    AlreadyOk,
    Unsat,
}

#[derive(Eq, PartialEq)]
pub struct Clause {
    pub watched: (usize, usize),
    pub literals: Vec<Literal>,
}

impl fmt::Debug for Clause {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Clause({} {}, {:?})",
            self.watched.0,
            self.watched.1,
            self.literals
                .iter()
                .map(|l| l.as_num())
                .collect::<Vec<i64>>()
        )
    }
}

impl Clause {
    pub fn new(literals: &mut Vec<i64>) -> Clause {
        literals.sort_unstable();
        literals.dedup();
        Clause {
            watched: (0, if literals.len() > 1 { 1 } else { 0 }),
            literals: literals.iter().map(Literal::new).collect(),
        }
    }

    pub fn update_watched(&mut self, variables: &Variables) -> WatchedUpdate {
        let fst_lit = self.literals[self.watched.0];

        if self.watched.0 == self.watched.1 {
            return NowUnit(fst_lit);
        }

        let snd_lit = self.literals[self.watched.1];
        let fst_val = variables.get(fst_lit.0).unwrap().state;
        let snd_val = variables.get(snd_lit.0).unwrap().state;

        if fst_lit.satisfied_by(fst_val) || snd_lit.satisfied_by(snd_val) {
            return AlreadySat;
        }

        if !fst_lit.falsified_by(fst_val) && !snd_lit.falsified_by(snd_val) {
            return AlreadyOk;
        }

        match self.next_unwatched(variables) {
            None => {
                if fst_lit.falsified_by(fst_val) {
                    if snd_lit.falsified_by(snd_val) {
                        Unsat
                    } else {
                        NowUnit(snd_lit)
                    }
                } else {
                    NowUnit(fst_lit)
                }
            }
            Some((idx, lit)) => {
                if fst_lit.falsified_by(fst_val) {
                    self.watched.0 = idx
                } else {
                    self.watched.1 = idx
                }
                NewWatched(lit)
            }
        }
    }

    fn next_unwatched(&self, variables: &Variables) -> Option<(usize, Literal)> {
        for (idx, literal) in self.literals.iter().enumerate() {
            if self.watched.0 == idx || self.watched.1 == idx {
                continue;
            }
            if !literal.falsified_by(variables.get(literal.0).unwrap().state) {
                return Some((idx, *literal));
            } else {
                debug!(
                    "{:?} is falsified by {:?}",
                    literal,
                    variables.get(literal.0).unwrap().state
                );
            }
        }
        None
    }
}
