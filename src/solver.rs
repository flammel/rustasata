extern crate priority_queue;
extern crate vec_map;

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::rc::Rc;
use std::time::Duration;
use std::time::Instant;

use self::priority_queue::PriorityQueue;
use self::vec_map::{Entry, Values, VecMap};
use self::Assignment::*;

use clause::{Clause, WatchedUpdate};
use literal::Literal;
use parser::Dimacs;
use variable::{Variable, VariableName, VariableState};

#[derive(Debug, Eq, PartialEq)]
pub enum SolverResult {
    Sat,
    Unsat,
}

#[derive(Debug, Eq, PartialEq)]
enum Assignment {
    Decision(Literal),
    NegatedDecision(Literal),
    Consequence(Literal),
}

impl Assignment {
    fn literal(&self) -> Literal {
        match self {
            Decision(literal) => *literal,
            NegatedDecision(literal) => *literal,
            Consequence(literal) => *literal,
        }
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
        Variables {
            variables: VecMap::new(),
        }
    }

    fn entry(&mut self, key: usize) -> Entry<Variable> {
        self.variables.entry(key)
    }

    fn values(&self) -> Values<Variable> {
        self.variables.values()
    }

    pub fn get(&self, literal: Literal) -> &Variable {
        self.variables
            .get(literal.0)
            .expect("Could not get variable")
    }

    fn get_mut(&mut self, literal: Literal) -> &mut Variable {
        self.variables
            .get_mut(literal.0)
            .expect("Could not get_mut variable")
    }
}

#[derive(Debug)]
pub struct Solver {
    variables: Variables,
    clauses: Vec<Rc<RefCell<Clause>>>,
    assignments: Vec<Assignment>,
    trivially_unsat: bool,
    bcp_queue: VecDeque<(Literal, Rc<RefCell<Clause>>)>,
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

struct Conflict(Rc<RefCell<Clause>>, Literal);

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
                .or_insert(Variable::new(literal.0));
            if clause.watched.0 == idx || clause.watched.1 == idx {
                variable.watch(literal.1, clauseref.clone());
                variable.occurences = variable.occurences + 1;
            }
        }
    }

    fn check_initial_unit(&mut self, clauseref: &Rc<RefCell<Clause>>) {
        let literals = &clauseref.borrow().literals;
        if literals.len() == 1 {
            self.bcp_queue.push_back((literals[0], clauseref.clone()));
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
        if self.unit_propagate().is_some() {
            debug!("Unsat by initial bcp");
            return SolverResult::Unsat;
        }
        debug!("Start loop");
        while let Some(var_name) = self.unassigned_var() {
            self.stats.decisions += 1;
            self.store_assignment(Decision(Literal(var_name, true)))
                .expect("Storing new decision lead to conflict");
            if let Some(Conflict(_c, _l)) = self.unit_propagate() {
                debug!("BCP caused conflict");
                if !self.backtrack() {
                    return SolverResult::Unsat;
                }
                self.bcp_queue.clear();
            }
        }
        debug!("Formula is sat");
        SolverResult::Sat
    }

    fn unassigned_var(&mut self) -> Option<VariableName> {
        self.variable_queue
            .peek()
            .and_then(|(i, VariablePriority(_, isset))| if *isset { None } else { Some(*i) })
    }

    //
    // Unit Propagation
    //

    fn unit_propagate(&mut self) -> Option<Conflict> {
        trace!("BCP");
        while let Some((literal, clause)) = self.bcp_queue.pop_front() {
            trace!("Propagate {:?}", literal);
            self.stats.propagations += 1;
            if self.store_assignment(Consequence(literal)).is_err() {
                return Some(Conflict(clause, literal));
            }
        }
        None
    }

    //
    // Backtracking
    //

    fn backtrack(&mut self) -> bool {
        debug!("Backtrack");
        loop {
            match self.assignments.pop() {
                None => {
                    debug!("Cannot backtrack, no assignments");
                    return false;
                }
                Some(Decision(to_negate)) => {
                    self.unset(to_negate);
                    self.store_assignment(NegatedDecision(!to_negate))
                        .expect("Negating decision lead to conflict");
                    return true;
                }
                Some(NegatedDecision(to_unset)) => {
                    self.unset(to_unset);
                }
                Some(Consequence(to_unset)) => {
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
    // Utilities
    //

    fn store_assignment(&mut self, assignment: Assignment) -> Result<(), ()> {
        debug!("Store {:?}", assignment);

        let literal = assignment.literal();

        let updated = {
            let variable = self.variables.get_mut(literal);
            self.variable_queue
                .change_priority_by(&literal.0, |prio| VariablePriority(prio.0, true));
            self.assignments.push(assignment);
            variable.set(literal.1)
        };

        match updated {
            Ok(clauses_to_update) => {
                for clause in clauses_to_update {
                    let update_result = clause.borrow_mut().update_watched(&self.variables);
                    match update_result {
                        WatchedUpdate::AlreadySat => {}
                        WatchedUpdate::AlreadyOk => {}
                        WatchedUpdate::Unsat => return Err(()),
                        WatchedUpdate::NowUnit(unit) => {
                            trace!("literal {:?} is now unit in {:?}", unit, clause.borrow());
                            self.bcp_queue.push_back((unit, clause.clone()));
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
            Err(_) => Err(()),
        }
    }
}
