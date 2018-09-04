use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use clause::{Clause, WatchedUpdate};
use decision_provider::DecisionProvider;
use literal::Literal;
use parser::Dimacs;
use variable::{Variable, Variables};

type ClauseRef = Rc<RefCell<Clause>>;

#[derive(Debug, Eq, PartialEq)]
pub enum SolverResult {
    Sat,
    Unsat,
}

#[derive(Debug)]
pub struct Solver {
    variables: Variables,
    clauses: Vec<ClauseRef>,
    assignments: Vec<Vec<Literal>>,
    trivially_unsat: bool,
    bcp_queue: VecDeque<(Literal, ClauseRef)>,
    stats: SolverStats,
    decision_provider: DecisionProvider,
    restart: (usize, usize, usize),
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
    init_time: Duration,
    solve_time: Duration,
    misc_time: Duration,
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
            init_time: Duration::new(0, 0),
            solve_time: Duration::new(0, 0),
            misc_time: Duration::new(0, 0),
        }
    }
}

#[derive(Debug)]
struct Conflict(ClauseRef);

impl Solver {
    //
    // Initialisation
    //

    pub fn from_dimacs(mut dimacs: Dimacs) -> Solver {
        let start = Instant::now();
        let mut solver = Solver {
            variables: Variables::new(),
            clauses: Vec::new(),
            assignments: vec![vec![]],
            trivially_unsat: false,
            bcp_queue: VecDeque::new(),
            stats: SolverStats::new(),
            decision_provider: DecisionProvider::new(),
            restart: (100, 100, 100),
        };
        for mut literals in dimacs.clauses.iter_mut() {
            solver.add_clause(&mut literals);
        }
        solver.decision_provider.init(&solver.variables);
        solver.stats.init_time += start.elapsed();
        solver
    }

    fn add_clause(&mut self, mut literals: &mut Vec<i64>) {
        self.stats.clauses += 1;
        if self.trivially_unsat {
            return;
        }
        if literals.is_empty() {
            self.trivially_unsat = true;
            return;
        }
        let clause = Rc::new(RefCell::new(Clause::new(&mut literals)));
        self.add_clause_variables(&clause);
        self.check_initial_unit(&clause);
    }

    fn add_clause_variables(&mut self, clauseref: &ClauseRef) {
        let clause = clauseref.borrow();
        for (idx, literal) in clause.literals.iter().enumerate() {
            self.stats.literals += 1;
            let variable = self
                .variables
                .entry(literal.0)
                .or_insert(Variable::new(literal.0));
            if clause.watched.0 == idx || clause.watched.1 == idx {
                variable.watch(literal.1, clauseref.clone());
            }
            variable.occurences += 1;
            if literal.1 {
                variable.occurences_positive += 1;
            } else {
                variable.occurences_negative += 1;
            }
        }
    }

    fn check_initial_unit(&mut self, clauseref: &ClauseRef) {
        let literals = &clauseref.borrow().literals;
        if literals.len() == 1 {
            self.bcp_queue.push_back((literals[0], clauseref.clone()));
        }
    }

    //
    // Main loop
    //

    pub fn solve(&mut self) -> SolverResult {
        let start = Instant::now();
        let result = self.internal_solve();
        self.stats.solve_time += start.elapsed();
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
                .expect("Storing new decision lead to conflict");
            while let Some(conflict) = self.unit_propagate() {
                self.stats.conflicts += 1;
                if let Some(level) = self.analyse_conflict(conflict) {
                    self.backtrack(level);
                } else {
                    return SolverResult::Unsat;
                }
            }
        }

        SolverResult::Sat
    }

    //
    // Restarts
    //

    /// https://pdfs.semanticscholar.org/7ea4/cdd0003234f9e98ff5a080d9191c398e26c2.pdf
    fn should_restart(&mut self) -> bool {
        if self.stats.conflicts > self.restart.2 {
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
        self.backtrack(1);
        // let split = (self.clauses.len() / 2) as usize;
        // for clause in self.clauses.split_off(split) {
        //     let (l1, l2) = clause.borrow().watched_literals();
        //     self.variables.get_mut(l1).unwatch(l1.1, &clause);
        //     self.variables.get_mut(l2).unwatch(l2.1, &clause);
        //     self.stats.learned_clauses -= 1;
        //     self.stats.learned_literals -= clause.borrow().literals.len();
        // }
    }

    //
    // Unit Propagation
    //

    fn unit_propagate(&mut self) -> Option<Conflict> {
        trace!("BCP");
        while let Some((literal, clause)) = self.bcp_queue.pop_front() {
            trace!("Propagate {:?}", literal);
            self.stats.propagations += 1;
            if self.store_consequence(literal, clause.clone()).is_err() {
                self.bcp_queue.clear();
                return Some(Conflict(clause));
            }
        }
        None
    }

    //
    // Conflict Analysis
    //

    fn analyse_conflict(&mut self, conflict: Conflict) -> Option<usize> {
        debug!("analyze {:?}", conflict);
        let (clause, unique) = self.get_clause_to_learn(conflict);
        let result = self.get_backtrack_level(&clause);
        self.add_learned_clause(clause, unique);
        result
    }

    fn get_clause_to_learn(&mut self, conflict: Conflict) -> (ClauseRef, Literal) {
        let current_literals = self
            .assignments
            .last()
            .expect("Cannot analyse conflict without decision levels");
        let mut clause = conflict.0.clone();
        loop {
            let uniqueness = clause.borrow().unique(&current_literals);
            match uniqueness {
                Err(non_unique) => {
                    let antecedent = self
                        .variables
                        .get(non_unique)
                        .antecedent
                        .clone()
                        .expect("Cannot get antecedent of var for conflict analysis");
                    let new_clause = clause.borrow().resolution(&antecedent.borrow(), non_unique);
                    clause = Rc::new(RefCell::new(new_clause));
                }
                Ok(unique) => {
                    return (clause, unique);
                }
            }
        }
    }

    fn get_backtrack_level(&self, clause: &ClauseRef) -> Option<usize> {
        let current_dl = Some(self.assignments.len());
        let literals = &clause.borrow().literals;
        let mut dl = None;
        for literal in literals {
            let vardl = self.variables.get(*literal).decision_level;
            if vardl == current_dl {
                continue;
            }
            dl = dl.max(vardl);
        }
        dl.or_else(|| {
            if self.assignments.len() > 1 {
                Some(self.assignments.len() - 1)
            } else {
                None
            }
        })
    }

    fn add_learned_clause(&mut self, clause: ClauseRef, unique: Literal) {
        debug!("learning {:?}", clause);
        self.stats.learned_clauses += 1;
        self.stats.learned_literals += clause.borrow().literals.len();

        let (l1, l2) = clause.borrow().watched_literals();
        self.variables.get_mut(l1).watch(l1.1, clause.clone());
        self.variables.get_mut(l2).watch(l2.1, clause.clone());
        self.bcp_queue.push_back((unique, clause.clone()));
        self.clauses.push(clause);
    }

    //
    // Backtracking
    //

    fn backtrack(&mut self, to_level: usize) {
        debug!(
            "Backtrack to level {:?} of {:?}",
            to_level,
            self.assignments.len()
        );
        let to_undo = self.assignments.split_off(to_level);
        for assignments in to_undo {
            for assignment in assignments {
                debug!("unset {:?}", assignment);
                self.unset(assignment);
            }
        }
    }

    fn unset(&mut self, to_unset: Literal) {
        self.decision_provider.unset(to_unset);
        self.variables.get_mut(to_unset).unset();
    }

    //
    // Utilities
    //

    fn store_decision(&mut self, literal: Literal) -> Result<(), ()> {
        debug!("Store decision {:?}", literal);
        self.stats.decisions += 1;
        self.assignments.push(vec![]);
        self.store_assignment(literal, None)
    }

    fn store_consequence(&mut self, literal: Literal, antecedent: ClauseRef) -> Result<(), ()> {
        debug!("Store consequence {:?} of {:?}", literal, antecedent);
        self.store_assignment(literal, Some(antecedent))
    }

    fn store_assignment(
        &mut self,
        literal: Literal,
        antecedent: Option<ClauseRef>,
    ) -> Result<(), ()> {
        let updated = {
            let variable = self.variables.get_mut(literal);
            variable.set(literal.1, antecedent, self.assignments.len())
        };

        match updated {
            None => Err(()),
            Some(clauses_to_update) => {
                self.decision_provider.set(literal);
                self.assignments
                    .last_mut()
                    .expect("Cannot store assignment, empty assignment stack")
                    .push(literal);
                for clause in clauses_to_update {
                    let update_result = clause.borrow_mut().update_watched(&self.variables);
                    match update_result {
                        WatchedUpdate::NoChange => {}
                        WatchedUpdate::NowUnit(unit) => {
                            trace!("literal {:?} is now unit in {:?}", unit, clause.borrow());
                            // TODO check whether this has a positive or negative performance impact
                            if unit.falsified_by(self.variables.get(unit).state) {
                                self.bcp_queue.push_front((unit, clause.clone()));
                            } else {
                                self.bcp_queue.push_back((unit, clause.clone()));
                            }
                        }
                        WatchedUpdate::NewWatched(watched) => {
                            let variable = self.variables.get_mut(watched);
                            variable.unwatch(watched.1, &clause);
                            variable.watch(watched.1, clause.clone());
                        }
                    }
                }
                Ok(())
            }
        }
    }
}
