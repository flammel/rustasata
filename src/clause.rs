extern crate vec_map;

use self::vec_map::VecMap;

use literal::Literal;

use self::WatchedUpdate::*;

#[derive(Debug, Eq, PartialEq)]
pub enum WatchedUpdate {
    NowUnit(Literal),
    NewWatched(Literal),
    NoChange,
}

#[derive(Eq, PartialEq, Debug)]
pub struct Clause(Vec<Literal>);

impl Clause {
    pub fn new(mut literals: Vec<i64>) -> Clause {
        literals.sort_unstable();
        literals.dedup();
        Clause(literals.iter().map(Literal::new).collect())
    }

    pub fn from_literals(mut literals: Vec<Literal>) -> Clause {
        literals.sort_unstable();
        literals.dedup();
        Clause(literals)
    }

    pub fn watched_literals(&self) -> (Literal, Literal) {
        if self.0.len() == 1 {
            (self.0[0], self.0[0])
        } else {
            (self.0[0], self.0[1])
        }
    }

    pub fn literals(&self) -> &Vec<Literal> {
        &self.0
    }

    pub fn propagate(&mut self, literal: &Literal, assigns: &VecMap<bool>) -> WatchedUpdate {
        if !*literal == self.0[0] {
            self.check(0, 1, &assigns)
        } else {
            self.check(1, 0, &assigns)
        }
    }

    fn check(&mut self, idx: usize, other_idx: usize, assigns: &VecMap<bool>) -> WatchedUpdate {
        let lit = self.0[idx];
        let val = assigns.get(lit.var());

        if lit.satisfied_by(val) {
            return NoChange;
        }

        // Skipping the first two literals, return the index of the first literal that is not falsified under the current assignment.
        let swap_with = self
            .0
            .iter()
            .enumerate()
            .skip(2)
            .find(|(_, literal)| !literal.falsified_by(assigns.get(literal.var())))
            .map(|(idx, _)| idx);

        match swap_with {
            None => NowUnit(self.0[other_idx]),
            Some(swap_idx) => {
                self.0.swap(idx, swap_idx);
                NewWatched(self.0[idx])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watched_literals_returns_unit() {
        let literals = vec![-78634];
        let clause = Clause::new(literals);
        assert_eq!(Literal(-78634), clause.watched_literals().0);
        assert_eq!(Literal(-78634), clause.watched_literals().1);
    }

    #[test]
    fn watched_literals_returns_first_two() {
        let literals = vec![-1, 3, 7];
        let clause = Clause::new(literals);
        assert_eq!(Literal(-1), clause.watched_literals().0);
        assert_eq!(Literal(3), clause.watched_literals().1);
    }

    #[test]
    fn propagate_swaps_literals_and_returns_new_watched() {
        let literals = vec![-4, -2, 1, 3];
        let mut clause = Clause::new(literals);
        let assigns = VecMap::new();
        let result = clause.propagate(&Literal(2), &assigns);
        assert_eq!(
            vec![Literal(-4), Literal(1), Literal(-2), Literal(3)],
            clause.0
        );
        assert_eq!(NewWatched(Literal(1)), result);
    }
}
