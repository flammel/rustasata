extern crate vec_map;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use self::vec_map::VecMap;

use clause::{Clause, WatchedUpdate};
use decision_provider::DecisionProvider;
use literal::Literal;
use parser::Dimacs;

type ClauseRef = Rc<RefCell<Clause>>;
pub type VariableName = usize;

#[derive(Debug, Eq, PartialEq)]
pub enum SolverResult {
    Sat,
    Unsat,
}

struct StatTime(Instant, Duration, usize);

impl fmt::Debug for StatTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} for {:?} calls", self.1, self.2)
    }
}

impl StatTime {
    fn new() -> StatTime {
        StatTime(Instant::now(), Duration::new(0, 0), 0)
    }

    fn start(&mut self) {
        self.0 = Instant::now();
        self.2 += 1;
    }

    fn end(&mut self) {
        self.1 += self.0.elapsed();
    }
}

#[derive(Debug)]
struct SolverStats {
    clauses: usize,
    literals: usize,
    decisions: usize,
    propagations: usize,
    learned_clauses: usize,
    learned_literals: usize,
    conflicts: usize,
    restarts: usize,
    init_time: StatTime,
    solve_time: StatTime,
    bcp_time: StatTime,
    sto_time: StatTime,
    ana_time: StatTime,
    bkt_time: StatTime,
}

impl SolverStats {
    fn new() -> SolverStats {
        SolverStats {
            clauses: 0,
            literals: 0,
            decisions: 0,
            propagations: 0,
            learned_clauses: 0,
            learned_literals: 0,
            conflicts: 0,
            restarts: 0,
            init_time: StatTime::new(),
            solve_time: StatTime::new(),
            bcp_time: StatTime::new(),
            sto_time: StatTime::new(),
            ana_time: StatTime::new(),
            bkt_time: StatTime::new(),
        }
    }
}

#[derive(Debug)]
struct Conflict(ClauseRef);

#[derive(Debug)]
pub struct Solver {
    trivially_unsat: bool,
    stats: SolverStats,
    learned_clauses: Vec<ClauseRef>,
    bcp_queue: VecDeque<Literal>,
    decision_provider: DecisionProvider,
    restart: (usize, usize, usize),

    // for each variable, its value
    assigns: VecMap<bool>,
    // for each variable, the clause that implied the variable's value
    reason: VecMap<ClauseRef>,
    // for each variable, the decision level it was assigned at
    level: VecMap<usize>,

    // assigned literals in chronological order
    trail: Vec<Literal>,
    // indices of trail separating the decision levels
    trail_lim: Vec<usize>,

    // indexed by literal.index(), a list of clauses that watch that literal
    watches: VecMap<Vec<ClauseRef>>,
}

impl Solver {
    //
    // Initialisation
    //

    fn new() -> Solver {
        Solver {
            trivially_unsat: false,
            stats: SolverStats::new(),
            learned_clauses: Vec::new(),
            bcp_queue: VecDeque::new(),
            decision_provider: DecisionProvider::new(),
            restart: (100, 100, 100),

            assigns: VecMap::new(),
            reason: VecMap::new(),
            level: VecMap::new(),

            trail: Vec::new(),
            trail_lim: Vec::new(),

            watches: VecMap::new(),
        }
    }

    pub fn from_dimacs(dimacs: &Dimacs) -> Solver {
        let mut solver = Solver::new();
        solver.stats.init_time.start();
        for literals in dimacs.clauses.iter() {
            solver.add_clause(literals.clone());
        }
        solver.stats.init_time.end();
        solver
    }

    fn add_clause(&mut self, literals: Vec<i64>) {
        self.stats.clauses += 1;

        if self.trivially_unsat {
            return;
        }

        if literals.is_empty() {
            self.trivially_unsat = true;
            return;
        }

        let clause = Clause::new(literals);
        self.decision_provider.new_clause(&clause.literals());
        let (wl1, wl2) = clause.watched_literals();
        let clauseref = Rc::new(RefCell::new(clause));
        self.watches
            .entry(wl1.index())
            .or_insert(Vec::new())
            .push(clauseref.clone());
        if wl1 != wl2 {
            self.watches
                .entry(wl2.index())
                .or_insert(Vec::new())
                .push(clauseref.clone());
        } else if self.store_consequence(wl1, clauseref.clone()).is_err() {
            self.trivially_unsat = true;
        }
    }

    //
    // Main loop
    //

    pub fn solve(&mut self) -> SolverResult {
        self.stats.solve_time.start();
        let result = self.internal_solve();
        self.stats.solve_time.end();
        info!("{:?}", self.stats);
        result
    }

    fn internal_solve(&mut self) -> SolverResult {
        if self.trivially_unsat {
            debug!("Trivially unsat");
            return SolverResult::Unsat;
        }

        if self.unit_propagate().is_some() {
            debug!("Unsat by initial bcp");
            return SolverResult::Unsat;
        }

        while let Some(decision) = self.decision_provider.get_next() {
            if self.should_restart() {
                self.restart();
                continue;
            }
            self.store_decision(decision)
                .expect("Decision lead to conflict");
            while let Some(conflict) = self.unit_propagate() {
                self.stats.conflicts += 1;
                if let Some((clause, unit, level)) = self.analyse_conflict(conflict) {
                    self.backtrack(level);
                    self.add_learned_clause(clause, unit).expect("Could not learn clause");
                } else {
                    return SolverResult::Unsat;
                }
            }
        }

        SolverResult::Sat
    }

    //
    // Decisions
    //

    fn store_decision(&mut self, literal: Literal) -> Result<(), ()> {
        debug!("Store decision {:?}", literal);
        self.stats.decisions += 1;
        self.store_assignment(literal, None)
    }

    //
    // Unit Propagation
    //

    fn unit_propagate(&mut self) -> Option<Conflict> {
        trace!("BCP");
        self.stats.bcp_time.start();
        let mut result = None;
        while let Some(unit) = self.bcp_queue.pop_front() {
            self.stats.propagations += 1;
            if let Some(clauses) = self.watches.get_mut((!unit).index()).map(|cs| cs.clone()) {
                let mut to_remove = Vec::new();
                for clause in clauses.iter() {
                    debug!("propagate {:?} to {:?}", unit, clause);
                    let update_result = clause.borrow_mut().propagate(&unit, &self.assigns);
                    match update_result {
                        WatchedUpdate::NoChange => {}
                        WatchedUpdate::NowUnit(unit) => {
                            let stored = self.store_consequence(unit, clause.clone());
                            if stored.is_err() {
                                self.bcp_queue.clear();
                                result = Some(Conflict(clause.clone()));
                                break;
                            }
                        }
                        WatchedUpdate::NewWatched(watched) => {
                            self.watches
                                .entry(watched.index())
                                .or_insert(Vec::new())
                                .push(clause.clone());
                            to_remove.push(clause);
                        }
                    }
                }
                let mut clauses = self
                    .watches
                    .get_mut((!unit).index())
                    .expect("Cannot get watching clauses to remove clauses");
                for clause in to_remove {
                    vec_remove(clauses, clause);
                }
            }
        }
        self.stats.bcp_time.end();
        trace!("BCP done");
        result
    }

    fn store_consequence(&mut self, literal: Literal, clause: ClauseRef) -> Result<(), ()> {
        debug!("Store consequence {:?}", literal);
        self.store_assignment(literal, Some(clause))
    }

    //
    // Conflict Analysis
    //

    fn analyse_conflict(&mut self, conflict: Conflict) -> Option<(Clause, Literal, usize)> {
        debug!("analyse {:?}", conflict);
        if self.trail_lim.len() == 0 {
            return None;
        }
        self.stats.ana_time.start();
        let (clause, unit) = self.get_clause_to_learn(conflict);
        let lvl = self.get_backtrack_level(&clause);
        let result = Some((clause, unit, lvl));
        self.stats.ana_time.end();
        result
    }

    fn get_clause_to_learn(&mut self, conflict: Conflict) -> (Clause, Literal) {
        let mut learned_literals: Vec<Literal> = conflict.0.borrow().literals().clone();
        let current_literals = self.trail.split_at(*self.trail_lim.last().expect("No current literals")).1;
        loop {
            match self.select_resolution_literal(&learned_literals, current_literals) {
                Err(non_unique) => {
                    let antecedent = self
                        .reason
                        .get(non_unique.var())
                        .expect("Cannot get reason of var for conflict analysis")
                        .clone();
                    Solver::resolve(
                        &mut learned_literals,
                        &antecedent.borrow().literals(),
                        non_unique,
                    );
                }
                Ok(unique) => {
                    return (Clause::from_literals(learned_literals), unique);
                }
            }
        }
    }

    fn resolve(alits: &mut Vec<Literal>, blits: &Vec<Literal>, literal: Literal) {
        alits.retain(|l| l.var() != literal.var());
        for x in blits.iter() {
            if x.var() != literal.var() {
                alits.push(x.clone())
            }
        }
        alits.sort_unstable();
        alits.dedup();
    }

    fn select_resolution_literal(
        &self,
        learned_literals: &Vec<Literal>,
        current_literals: &[Literal],
    ) -> Result<Literal, Literal> {
        let mut result = None;
        for curlit in current_literals.iter().rev() {
            for lealit in learned_literals.iter() {
                if lealit.var() == curlit.var() {
                    match result {
                        None => result = Some(lealit),
                        Some(rlit) => if self.reason.get(rlit.var()).is_none() {
                            return Err(*lealit);
                        } else {
                            return Err(*rlit);
                        },
                    }
                }
            }
        }
        if let Some(literal) = result {
            return Ok(*literal);
        } else {
            panic!("Clause does not contain any of the given variables");
        }
    }

    fn get_backtrack_level(&self, clause: &Clause) -> usize {
        let current_dl = self.trail_lim.len();
        let literals = clause.literals();
        let mut dl = 0;
        for literal in literals {
            let vardl = *self.level.get(literal.var()).expect("No level for literal in learned clause");
            if vardl == current_dl {
                continue;
            }
            dl = dl.max(vardl);
        }
        dl
    }

    fn add_learned_clause(&mut self, clause: Clause, unit: Literal) -> Result<(), ()> {
        debug!("learning {:?} with unit {:?}", clause, unit);
        self.stats.learned_clauses += 1;
        self.stats.learned_literals += clause.literals().len();
        self.decision_provider.new_clause(&clause.literals());
        let (wl1, wl2) = clause.watched_literals();
        let clauseref = Rc::new(RefCell::new(clause));
        self.watches
            .entry(wl1.index())
            .or_insert(Vec::new())
            .push(clauseref.clone());
        if wl1 != wl2 {
            self.watches
                .entry(wl2.index())
                .or_insert(Vec::new())
                .push(clauseref.clone());
        }
        self.learned_clauses.push(clauseref.clone());
        self.store_consequence(unit, clauseref)
    }

    //
    // Backtracking
    //

    fn backtrack(&mut self, to_level: usize) {
        debug!(
            "Backtrack to level {:?} of {:?}",
            to_level,
            self.trail_lim.len()
        );
        self.stats.bkt_time.start();
        let unset_list = self.trail.split_off(self.trail_lim[to_level]);
        self.trail_lim.split_off(to_level);
        for unset in unset_list {
            self.assigns.remove(unset.var());
            self.level.remove(unset.var());
            self.reason.remove(unset.var());
            self.decision_provider.unset(unset.var());
        }
        self.stats.bkt_time.end();
    }

    //
    // Restarts
    //

    /// https://pdfs.semanticscholar.org/7ea4/cdd0003234f9e98ff5a080d9191c398e26c2.pdf
    fn should_restart(&mut self) -> bool {
        if self.stats.conflicts > self.restart.2 && self.trail_lim.len() > 0 {
            true
        } else {
            false
        }
    }

    fn restart(&mut self) {
        trace!("{:?}", self.stats);
        if self.restart.0 >= self.restart.1 {
            self.restart.1 = (self.restart.1 as f64 * 1.1) as usize;
            self.restart.0 = 100;
        } else {
            self.restart.0 = (self.restart.0 as f64 * 1.1) as usize;
        }
        self.restart.2 = self.stats.conflicts + self.restart.0;
        self.stats.restarts += 1;
        self.backtrack(0);
        // let split = (self.learned_clauses.len() / 2) as usize;
        // for clause in self.learned_clauses.split_off(split) {
        //     let (l1, l2) = clause.borrow().watched_literals();
        //     vec_remove(self.watches.get_mut(l1.index()).expect("No watchlist for clause removal"), &clause);
        //     vec_remove(self.watches.get_mut(l2.index()).expect("No watchlist for clause removal"), &clause);
        // }
    }

    //
    // Utilities
    //

    fn store_assignment(&mut self, literal: Literal, clause: Option<ClauseRef>) -> Result<(), ()> {
        self.stats.sto_time.start();
        if let Some(current) = self.assigns.get(literal.var()) {
            if *current != literal.sign() {
                self.stats.sto_time.end();
                return Err(());
            } else {
                self.stats.sto_time.end();
                return Ok(());
            }
        }
        if let Some(clause) = clause {
            self.reason.insert(literal.var(), clause);
        } else {
            self.trail_lim.push(self.trail.len());
        }
        self.trail.push(literal);
        self.assigns.insert(literal.var(), literal.sign());
        self.level.insert(literal.var(), self.trail_lim.len());
        self.decision_provider.set(literal.var());
        self.bcp_queue.push_back(literal);

        self.stats.sto_time.end();

        Ok(())
    }
}

fn vec_remove<T: PartialEq>(vec: &mut Vec<T>, item: &T) {
    if let Some(pos) = vec.iter().position(|x| *x == *item) {
        vec.remove(pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backtrack() -> Result<(), ()> {
        let mut solver = Solver::new();

        let clause = Rc::new(RefCell::new(Clause::new(vec![1, 2, 3])));
        solver.store_decision(Literal(1))?;
        solver.store_consequence(Literal(-2), clause.clone())?;
        solver.store_consequence(Literal(3), clause.clone())?;
        solver.store_decision(Literal(10))?;
        solver.store_consequence(Literal(-20), clause.clone())?;
        solver.store_consequence(Literal(30), clause.clone())?;
        solver.store_decision(Literal(100))?;
        solver.store_consequence(Literal(-200), clause.clone())?;
        solver.store_consequence(Literal(300), clause.clone())?;

        assert_eq!(
            vec![
                Literal(1),
                Literal(-2),
                Literal(3),
                Literal(10),
                Literal(-20),
                Literal(30),
                Literal(100),
                Literal(-200),
                Literal(300),
            ],
            solver.trail
        );
        assert_eq!(vec![0, 3, 6], solver.trail_lim);
        assert_eq!(Some(&1), solver.level.get(1));
        assert_eq!(Some(&2), solver.level.get(10));
        assert_eq!(Some(&3), solver.level.get(100));

        solver.backtrack(1);

        assert_eq!(vec![Literal(1), Literal(-2), Literal(3)], solver.trail);
        assert_eq!(vec![0], solver.trail_lim);
        Ok(())
    }
}
