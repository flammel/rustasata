use std::fmt;

use literal::Literal;
use solver::Variables;

use self::WatchedUpdate::*;

#[derive(Debug)]
pub enum WatchedUpdate {
    NowUnit(Literal),
    NewWatched(Literal),
    NoChange,
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

    pub fn watched_literals(&self) -> (Literal, Literal) {
        (self.literals[self.watched.0], self.literals[self.watched.1])
    }

    /// ok -> unique
    /// err -> not unique
    pub fn unique(&self, alits: &Vec<Literal>) -> Result<Literal, Literal> {
        let mut result = None;
        for alit in alits {
            for literal in self.literals.iter() {
                if *literal == !*alit {
                    match result {
                        None => result = Some(*literal),
                        Some(rlit) => return Err(rlit),
                    }
                }
            }
        }
        if let Some(literal) = result {
            return Ok(literal);
        } else {
            panic!("Clause does not contain any of the given variables");
        }
    }

    pub fn resolution(&self, other: &Clause, literal: Literal) -> Clause {
        let mut literals = Vec::with_capacity(self.literals.len() + other.literals.len());
        for x in self.literals.iter() {
            if x.0 != literal.0 {
                literals.push(x.as_num())
            }
        }
        for x in other.literals.iter() {
            if x.0 != literal.0 {
                literals.push(x.as_num())
            }
        }
        Clause::new(&mut literals)
    }

    pub fn update_watched(&mut self, variables: &Variables) -> WatchedUpdate {
        let fst_lit = self.literals[self.watched.0];

        if self.watched.0 == self.watched.1 {
            return NowUnit(fst_lit);
        }

        let snd_lit = self.literals[self.watched.1];
        let fst_val = variables.get(fst_lit).state;
        let snd_val = variables.get(snd_lit).state;

        if fst_lit.satisfied_by(fst_val) || snd_lit.satisfied_by(snd_val) {
            return NoChange;
        }

        if !fst_lit.falsified_by(fst_val) && !snd_lit.falsified_by(snd_val) {
            return NoChange;
        }

        match self.next_unwatched(variables) {
            None => {
                if fst_lit.falsified_by(fst_val) {
                    NowUnit(snd_lit)
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
            if !literal.falsified_by(variables.get(*literal).state) {
                return Some((idx, *literal));
            }
        }
        None
    }
}
