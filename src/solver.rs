extern crate priority_queue;
extern crate vec_map;

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::fmt;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use self::priority_queue::PriorityQueue;
use self::vec_map::{VecMap, Entry, Values};

use clause::{Clause, WatchedUpdate};
use literal::Literal;
use parser::Dimacs;
use variable::{Variable, VariableName, VariableState};

use self::AssignmentType::*;

#[derive(Debug, Eq, PartialEq)]
pub enum SolverResult {
    Sat,
    Unsat,
}

#[derive(Debug, Eq, PartialEq)]
enum AssignmentType {
    InitialUnit,
    Decision,
    NegatedDecision,
    Consequence,
}

struct Assignment(Literal, AssignmentType);

impl fmt::Debug for Assignment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}({:?})", self.1, self.0)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct VariablePriority(usize, bool);

impl PartialOrd for VariablePriority {
    fn partial_cmp(&self, other: &VariablePriority) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VariablePriority {
    fn cmp(&self, other: &VariablePriority) -> Ordering {
        if self.1 && !other.1 {
            Ordering::Less
        } else if !self.1 && other.1 {
            Ordering::Greater
        } else {
            self.0.cmp(&other.0)
        }
    }
}

#[derive(Debug)]
pub struct Variables {
    variables: VecMap<Variable>,
}

impl Variables {
    fn new() -> Variables {
        Variables { variables: VecMap::new() }
    }

    fn entry(&mut self, key: usize) -> Entry<Variable> {
        self.variables.entry(key)
    }

    fn values(&self) -> Values<Variable> {
        self.variables.values()
    }

    fn len(&self) -> usize {
        self.variables.len()
    }

    pub fn get(&self, literal: Literal) -> &Variable {
        self.variables.get(literal.0).expect("Could not get variable")
    }

    fn get_mut(&mut self, literal: Literal) -> &mut Variable {
        self.variables.get_mut(literal.0).expect("Could not get_mut variable")
    }
}

#[derive(Debug)]
pub struct Solver {
    variables: Variables,
    clauses: Vec<Rc<RefCell<Clause>>>,
    assignments: Vec<Assignment>,
    trivially_unsat: bool,
    bcp_queue: VecDeque<Literal>,
    stats: SolverStats,
    variable_queue: PriorityQueue<VariableName, VariablePriority>,
}

#[derive(Debug)]
struct SolverStats {
    clauses: u64,
    literals: u64,
    decisions: u64,
    propagations: u64,
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
            init_time: Duration::new(0, 0),
            solve_time: Duration::new(0, 0),
            misc_time: Duration::new(0, 0),
        }
    }
}

impl Solver {
    //
    // Initialisation
    //

    pub fn from_dimacs(mut dimacs: Dimacs) -> Solver {
        let start = Instant::now();
        let mut solver = Solver {
            variables: Variables::new(),
            clauses: Vec::new(),
            assignments: vec![],
            trivially_unsat: false,
            bcp_queue: VecDeque::new(),
            stats: SolverStats::new(),
            variable_queue: PriorityQueue::new(),
        };
        for mut literals in dimacs.clauses.iter_mut() {
            solver.add_clause(&mut literals);
        }
        solver.build_variable_queue();
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
        self.clauses.push(clause);
    }

    fn add_clause_variables(&mut self, clauseref: &Rc<RefCell<Clause>>) {
        let clause = clauseref.borrow();
        for (idx, literal) in clause.literals.iter().enumerate() {
            self.stats.literals += 1;
            let variable = self
                .variables
                .entry(literal.0)
                .or_insert(Variable::new(literal));
            if clause.watched.0 == idx || clause.watched.1 == idx {
                variable.watch(literal.1, clauseref.clone());
                variable.occurences = variable.occurences + 1;
            }
        }
    }

    fn check_initial_unit(&mut self, clauseref: &Rc<RefCell<Clause>>) {
        let literals = &clauseref.borrow().literals;
        if literals.len() == 1 {
            if self.store_assignment(literals[0], InitialUnit).is_err() {
                self.trivially_unsat = true;
            }
        }
    }

    fn build_variable_queue(&mut self) {
        self.variable_queue = self
            .variables
            .values()
            .map(|var| {
                (
                    var.name,
                    VariablePriority(var.occurences, var.state != VariableState::Open),
                )
            })
            .collect();
    }

    //
    // Main loop
    //

    pub fn solve(&mut self) -> SolverResult {
        let start = Instant::now();
        let result = self.internal_solve();
        self.stats.solve_time += start.elapsed();
        println!("{:?}", self.stats);
        result
    }

    fn internal_solve(&mut self) -> SolverResult {
        if self.trivially_unsat {
            debug!("Trivially unsat");
            return SolverResult::Unsat;
        }
        if !self.unit_propagate() {
            debug!("Unsat by initial bcp");
            return SolverResult::Unsat;
        }
        debug!("Start loop");
        while !self.done() {
            debug!("Not done");
            if !self.unit_propagate() {
                debug!("BCP caused conflict");
                if !self.backtrack() {
                    return SolverResult::Unsat;
                }
            } else {
                debug!("BCP yielded sat");
                if let Some(var_name) = self.unassigned_var() {
                    self.stats.decisions += 1;
                    self.store_assignment(Literal(var_name, true), Decision)
                        .expect("Storing new decision lead to conflict");
                }
            }
        }
        debug!("Formula is sat");
        SolverResult::Sat
    }

    fn done(&self) -> bool {
        self.variables.len() == self.assignments.len()
    }

    fn unassigned_var(&mut self) -> Option<VariableName> {
        self
            .variable_queue
            .peek()
            .and_then(|(i, VariablePriority(_, isset))| if *isset { None } else { Some(*i) })
    }

    //
    // Backtracking
    //

    fn backtrack(&mut self) -> bool {
        debug!("Backtrack");
        self.bcp_queue.clear();
        loop {
            match self.assignments.pop() {
                None => {
                    debug!("Cannot backtrack, no assignments");
                    return false;
                }
                Some(Assignment(_, InitialUnit)) => {
                    debug!("Cannot backtrack, reached initial units");
                    return false;
                }
                Some(Assignment(to_negate, Decision)) => {
                    self.unset(to_negate);
                    self.store_assignment(!to_negate, NegatedDecision)
                        .expect("Negating decision lead to conflict");
                    return true;
                }
                Some(Assignment(to_unset, NegatedDecision)) => {
                    self.unset(to_unset);
                }
                Some(Assignment(to_unset, Consequence)) => {
                    self.unset(to_unset);
                }
            }
        }
    }

    fn unset(&mut self, to_unset: Literal) {
        self.variable_queue
            .change_priority_by(&to_unset.0, |prio| VariablePriority(prio.0, false));
        self.variables.get_mut(to_unset).state = VariableState::Open;
    }

    //
    // Unit Propagation
    //

    fn unit_propagate(&mut self) -> bool {
        trace!("\n\nBCP\n");
        while let Some(propagate) = self.bcp_queue.pop_front() {
            self.stats.propagations += 1;
            for clause in self.clauses_to_update(propagate) {
                let update_result = clause.borrow_mut().update_watched(&self.variables);
                trace!(
                    "propagate {:?} to {:?} yielded {:?}",
                    propagate,
                    clause,
                    update_result
                );
                match update_result {
                    WatchedUpdate::AlreadySat => {}
                    WatchedUpdate::AlreadyOk => {}
                    WatchedUpdate::Unsat => return false,
                    WatchedUpdate::NowUnit(literal) => {
                        if self.store_assignment(literal, Consequence).is_err() {
                            trace!("Contradiction from unit clause");
                            return false;
                        }
                    }
                    WatchedUpdate::NewWatched(literal) => {
                        let variable = self.variables.get_mut(literal);
                        variable.unwatch(literal.1, &clause);
                        variable.watch(literal.1, clause.clone());
                    }
                }
            }
        }
        return true;
    }

    fn clauses_to_update(&self, propagated: Literal) -> Vec<Rc<RefCell<Clause>>> {
        let variable = self.variables.get(propagated);
        if propagated.1 {
            variable.watched_neg.clone()
        } else {
            variable.watched_pos.clone()
        }
    }

    //
    // Utilities
    //

    fn store_assignment(&mut self, literal: Literal, a_type: AssignmentType) -> Result<(), ()> {
        let assignment = Assignment(literal, a_type);
        debug!("Store {:?}", assignment);

        let new_state = if literal.1 {
            VariableState::True
        } else {
            VariableState::False
        };

        let variable = self.variables.get_mut(literal);

        if variable.state == VariableState::Open {
            variable.state = new_state;
            self.variable_queue
                .change_priority_by(&literal.0, |prio| VariablePriority(prio.0, true));
            self.assignments.push(assignment);
            self.bcp_queue.push_back(literal);
            Ok(())
        } else if variable.state == new_state {
            Ok(())
        } else {
            Err(())
        }
    }
}
