#[macro_use]
extern crate log;
extern crate fern;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::BinaryHeap;
use std::collections::VecDeque;
use std::env;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::ops::Not;
use std::rc::Rc;
use std::time::Instant;

use self::AssignmentType::*;

fn setup_logger() -> Result<(), fern::InitError> {
    if env::var("NO_LOG").is_err() {
        fern::Dispatch::new()
            .format(|out, message, _| out.finish(format_args!("{}", message)))
            .level(log::LevelFilter::Trace)
            .chain(std::io::stdout())
            .chain(fern::log_file("output.log")?)
            .apply()?;
    }
    Ok(())
}

fn main() {
    setup_logger().unwrap();
    let args: Vec<String> = env::args().collect();
    let filepath = &args.get(1).expect("No file path given");

    let total_start = Instant::now();

    let start = Instant::now();
    let dimacs = parse_file(filepath).unwrap();
    let to_parse = start.elapsed();

    let start = Instant::now();
    let mut solver = Solver::from_dimacs(dimacs);
    let to_init = start.elapsed();

    let start = Instant::now();
    let result = solver.solve();
    let to_solve = start.elapsed();

    let total = total_start.elapsed();

    println!(
        "{} ===== {:?} in {:?} ===== {:?} to parse | {:?} to init | {:?} to solve",
        filepath, result, total, to_parse, to_init, to_solve
    )
}

//
// Literal
//

#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
struct Literal(VariableName, bool);

impl fmt::Debug for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_num())
    }
}

impl Not for Literal {
    type Output = Literal;
    fn not(self) -> Literal {
        Literal(self.0, !self.1)
    }
}

impl Literal {
    fn new(num: &i64) -> Literal {
        Literal(num.abs() as u64, *num >= 0)
    }

    fn falsified_by(&self, var_val: VariableState) -> bool {
        (var_val == VariableState::True && !self.1) || (self.1 && var_val == VariableState::False)
    }

    fn satisfied_by(&self, var_val: VariableState) -> bool {
        (var_val == VariableState::True && self.1) || (var_val == VariableState::False && !self.1)
    }

    fn as_num(&self) -> i64 {
        let mut num = self.0 as i64;
        if !self.1 {
            num = num.wrapping_neg();
        }
        num
    }
}

//
// Variable
//

type VariableName = u64;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum VariableState {
    True,
    False,
    Open,
}

#[derive(Debug, Eq, PartialEq)]
struct Variable {
    name: VariableName,
    state: VariableState,
    watched_pos: Vec<Rc<RefCell<Clause>>>,
    watched_neg: Vec<Rc<RefCell<Clause>>>,
    occurences: u64,
}

impl Variable {
    fn new(literal: &Literal) -> Variable {
        Variable {
            name: literal.0,
            state: VariableState::Open,
            watched_neg: Vec::new(),
            watched_pos: Vec::new(),
            occurences: 0,
        }
    }

    fn watch(&mut self, sign: bool, clause: Rc<RefCell<Clause>>) {
        if sign {
            self.watched_pos.push(clause);
        } else {
            self.watched_neg.push(clause);
        }
    }

    fn unwatch(&mut self, sign: bool, clause: &Rc<RefCell<Clause>>) {
        if sign {
            vec_remove(&mut self.watched_pos, clause)
        } else {
            vec_remove(&mut self.watched_neg, clause)
        }
    }
}

fn vec_remove<T: PartialEq>(vec: &mut Vec<T>, item: &T) {
    if let Some(pos) = vec.iter().position(|x| *x == *item) {
        vec.remove(pos);
    }
}

//
// Clause
//

#[derive(Debug)]
enum Watched {
    NowUnit(Literal),
    NewWatched(Literal),
    AlreadySat,
    AlreadyOk,
    Unsat,
}

#[derive(Eq, PartialEq, PartialOrd, Ord)]
struct Clause {
    watched: (usize, usize),
    literals: Vec<Literal>,
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
    fn new(literals: &mut Vec<i64>) -> Clause {
        literals.sort_unstable();
        literals.dedup();
        Clause {
            watched: (0, if literals.len() > 1 { 1 } else { 0 }),
            literals: literals.iter().map(Literal::new).collect(),
        }
    }

    fn update_watched(&mut self, variables: &BTreeMap<VariableName, Variable>) -> Watched {
        let fst_lit = self.literals[self.watched.0];

        if self.watched.0 == self.watched.1 {
            return Watched::NowUnit(fst_lit);
        }

        let snd_lit = self.literals[self.watched.1];
        let fst_val = variables.get(&fst_lit.0).unwrap().state;
        let snd_val = variables.get(&snd_lit.0).unwrap().state;

        if fst_lit.satisfied_by(fst_val) || snd_lit.satisfied_by(snd_val) {
            return Watched::AlreadySat;
        }

        if !fst_lit.falsified_by(fst_val) && !snd_lit.falsified_by(snd_val) {
            return Watched::AlreadyOk;
        }

        match self.next_unwatched(variables) {
            None => {
                if fst_lit.falsified_by(fst_val) {
                    if snd_lit.falsified_by(snd_val) {
                        Watched::Unsat
                    } else {
                        Watched::NowUnit(snd_lit)
                    }
                } else {
                    Watched::NowUnit(fst_lit)
                }
            }
            Some((idx, lit)) => {
                if fst_lit.falsified_by(fst_val) {
                    self.watched.0 = idx
                } else {
                    self.watched.1 = idx
                }
                Watched::NewWatched(lit)
            }
        }
    }

    fn next_unwatched(
        &self,
        variables: &BTreeMap<VariableName, Variable>,
    ) -> Option<(usize, Literal)> {
        for (idx, literal) in self.literals.iter().enumerate() {
            if self.watched.0 == idx || self.watched.1 == idx {
                continue;
            }
            if !literal.falsified_by(variables.get(&literal.0).unwrap().state) {
                return Some((idx, *literal));
            } else {
                debug!(
                    "{:?} is falsified by {:?}",
                    literal,
                    variables.get(&literal.0).unwrap().state
                );
            }
        }
        None
    }
}

//
// Solver
//

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

#[derive(Eq, PartialEq)]
struct Assignment(Literal, AssignmentType);

impl fmt::Debug for Assignment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}({:?})", self.1, self.0)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Solver {
    variables: BTreeMap<VariableName, Variable>,
    clauses: BTreeSet<Rc<RefCell<Clause>>>,
    assignments: Vec<Assignment>,
    trivially_unsat: bool,
    bcp_queue: VecDeque<Literal>,
}

impl Solver {
    //
    // Initialisation
    //

    fn new() -> Solver {
        Solver {
            variables: BTreeMap::new(),
            clauses: BTreeSet::new(),
            assignments: vec![],
            trivially_unsat: false,
            bcp_queue: VecDeque::new(),
        }
    }

    fn from_dimacs(mut dimacs: Dimacs) -> Solver {
        let mut solver = Solver::new();
        for mut literals in dimacs.clauses.iter_mut() {
            solver.add_clause(&mut literals);
        }
        solver
    }

    fn add_clause(&mut self, mut literals: &mut Vec<i64>) {
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
        self.clauses.insert(clause);
    }

    fn add_clause_variables(&mut self, clauseref: &Rc<RefCell<Clause>>) {
        let clause = clauseref.borrow();
        for (idx, literal) in clause.literals.iter().enumerate() {
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

    //
    // Main loop
    //

    fn solve(&mut self) -> SolverResult {
        if self.trivially_unsat {
            debug!("Trivially unsat");
            return SolverResult::Unsat;
        }
        if let SolverResult::Unsat = self.unit_propagate() {
            debug!("Unsat by initial bcp");
            return SolverResult::Unsat;
        }
        debug!("Start loop");
        while !self.done() {
            debug!("Not done");
            if let SolverResult::Unsat = self.unit_propagate() {
                debug!("BCP caused conflict");
                if !self.backtrack() {
                    return SolverResult::Unsat;
                }
            } else {
                debug!("BCP yielded sat");
                if let Some(var_name) = self.unassigned_var() {
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

    fn unassigned_var(&self) -> Option<u64> {
        self.variables
            .values()
            .filter(|v| v.state == VariableState::Open)
            .map(|v| (v.occurences, v.name))
            .collect::<BinaryHeap<(u64, VariableName)>>()
            .peek()
            .map(|x| x.1)
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
        self.get_var_mut(to_unset).state = VariableState::Open;
    }

    //
    // Unit Propagation
    //

    fn unit_propagate(&mut self) -> SolverResult {
        trace!("\n\nBCP\n");
        while let Some(propagate) = self.bcp_queue.pop_front() {
            for clause in self.clauses_to_update(propagate) {
                let update_result = clause.borrow_mut().update_watched(&self.variables);
                trace!(
                    "propagate {:?} to {:?} yielded {:?}",
                    propagate,
                    clause,
                    update_result
                );
                match update_result {
                    Watched::AlreadySat => {}
                    Watched::AlreadyOk => {}
                    Watched::Unsat => return SolverResult::Unsat,
                    Watched::NowUnit(literal) => {
                        if self.store_assignment(literal, Consequence).is_err() {
                            trace!("Contradiction from unit clause");
                            return SolverResult::Unsat;
                        }
                    }
                    Watched::NewWatched(literal) => {
                        let variable = self.get_var_mut(literal);
                        variable.unwatch(literal.1, &clause);
                        variable.watch(literal.1, clause.clone());
                    }
                }
            }
        }
        return SolverResult::Sat;
    }

    fn clauses_to_update(&self, propagated: Literal) -> Vec<Rc<RefCell<Clause>>> {
        let variable = self.get_var(propagated);
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

        let variable = self
            .variables
            .get_mut(&literal.0)
            .expect("Variable not found for assignment");

        if variable.state == VariableState::Open {
            variable.state = new_state;
            self.assignments.push(assignment);
            self.bcp_queue.push_back(literal);
            Ok(())
        } else if variable.state == new_state {
            Ok(())
        } else {
            Err(())
        }
    }

    fn get_var(&self, literal: Literal) -> &Variable {
        self.variables.get(&literal.0).expect("Variable not found")
    }

    fn get_var_mut(&mut self, literal: Literal) -> &mut Variable {
        self.variables
            .get_mut(&literal.0)
            .expect("Variable not found for mut")
    }
}

//
// DIMACS Parsing
//

type DimacsClauses = Vec<Vec<i64>>;

#[derive(Debug)]
struct Dimacs {
    clauses: DimacsClauses,
}

#[derive(Debug)]
struct DimacsError(&'static str);

fn parse_file(path: &str) -> Result<Dimacs, DimacsError> {
    if let Ok(mut file) = File::open(path) {
        let mut contents = String::new();
        if let Ok(_) = file.read_to_string(&mut contents) {
            parse(contents.as_str())
        } else {
            Err(DimacsError("Could not read file"))
        }
    } else {
        Err(DimacsError("Could not open file"))
    }
}

fn parse(dimacs: &str) -> Result<Dimacs, DimacsError> {
    dimacs
        .lines()
        .map(|line| line.trim())
        .filter(|line| {
            !line.starts_with("p")
                && !line.starts_with("c")
                && !line.starts_with("%")
                && !line.starts_with("0")
                && !line.is_empty()
        })
        .map(|line| {
            line
            .split_whitespace()
            .map(|num| num.parse::<i64>())
            // Keep all the errors so we know if something went wrong, but remove
            // successfully parsed 0s which end each line in DIMACS format.
            .filter(|num| match num {
                Ok(x) => *x != 0,
                Err(_) => true
            })
            .collect()
        })
        .collect::<Result<DimacsClauses, std::num::ParseIntError>>()
        .map(|clauses| Dimacs { clauses })
        .map_err(|_| DimacsError("Could not parse"))
}

//
// Tests
//
#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn run_test(str: &str) -> SolverResult {
        if env::var("LOG").is_ok() {
            let _ = setup_logger();
        }
        let dimacs = parse(str).unwrap();
        Solver::from_dimacs(dimacs).solve()
    }

    #[allow(dead_code)]
    fn run_test_file(str: &str) -> SolverResult {
        if env::var("LOG").is_ok() {
            let _ = setup_logger();
        }
        let dimacs = parse_file(str).unwrap();
        Solver::from_dimacs(dimacs).solve()
    }

    #[test]
    fn test_empty_formula() {
        let result = run_test("");
        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_contradiction() {
        let result = run_test("-1\n1");
        assert_eq!(result, SolverResult::Unsat);
    }

    #[test]
    fn test_double_positive() {
        let result = run_test("1\n1");
        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_double_negative() {
        let result = run_test("-1\n-1");
        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_one_clause_duplicate_literals() {
        let result = run_test("-1 -1 1 1");
        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_bcp_1() {
        let result = run_test("1\n-1 -2\n2");
        assert_eq!(result, SolverResult::Unsat);
    }

    #[test]
    fn test_bcp_2() {
        let result = run_test("1\n2\n-1 -2");
        assert_eq!(result, SolverResult::Unsat);
    }

    #[test]
    fn test_bcp_3() {
        let result = run_test("-1 -2\n1\n2");
        assert_eq!(result, SolverResult::Unsat);
    }

    #[test]
    fn test_bcp_4() {
        let result = run_test("-1\n1 2\n-2");
        assert_eq!(result, SolverResult::Unsat);
    }

    #[test]
    fn test_bcp_5() {
        let result = run_test("-1\n-2\n1 2");
        assert_eq!(result, SolverResult::Unsat);
    }

    #[test]
    fn test_bcp_6() {
        let result = run_test("-1 2\n-2\n1 2");
        assert_eq!(result, SolverResult::Unsat);
    }

    #[test]
    fn test_bcp_7() {
        let result = run_test("-1 2 3\n-2\n1 2");
        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_tiny_sat_instance_1() {
        let result = run_test(
            "
            1 2 -3
            -1 -2
        ",
        );
        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_tiny_sat_instance_2() {
        let result = run_test(
            "
            1 2 -3
            -1 -2
            -1 2 -3
        ",
        );
        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_tiny_sat_instance_3() {
        let result = run_test(
            "
            1 2 3
            -2 -3 4
            5 -3 -1
            -4 -5
        ",
        );
        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    fn test_tiny_sat_instance_4() {
        let result = run_test(
            "
            -1 2 -4
            -2 3 -4
        ",
        );
        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    #[ignore]
    fn test_file_trivial_official_sample() {
        let result = run_test_file("test/trivial/officialSample.txt");
        assert_eq!(result, SolverResult::Sat);
    }

    #[test]
    #[ignore]
    fn test_file_easy_queens() {
        let result = run_test_file("test/easy/19x19queens.txt");
        assert_eq!(result, SolverResult::Sat);
    }
}
