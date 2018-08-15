use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, Read};
use std::ops::Not;
use std::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum Result { Sat, Unsat }

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
struct Literal(u64, bool);

impl Not for Literal {
    type Output = Literal;
    fn not(self) -> Literal {
        Literal(self.0, !self.1)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
struct Clause {
    literals: BTreeSet<Literal>,
}

impl Clause {
    fn is_empty(&self) -> bool {
        self.literals.is_empty()
    }

    fn is_unit(&self) -> bool {
        self.literals.len() == 1
    }
}

#[derive(Debug, Clone)]
pub struct Formula {
    clauses: BTreeSet<Clause>,
}

impl Formula {
    fn is_consistent(&self) -> bool {
        self.clauses.is_empty()
    }

    fn is_contradictory(&self) -> bool {
        self.clauses.iter().any(Clause::is_empty)
    }
}

fn main() {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer).unwrap();
    let mut formula = parse(buffer.as_str());
    let start = Instant::now();
    let result = solve(&mut formula);
    let elapsed = start.elapsed();
    println!("{:?} in {:?}", result, elapsed);
}

pub fn solve(formula: &mut Formula) -> Result {
    // println!("{:?}", formula);
    if formula.is_consistent() {
        Result::Sat
    } else if formula.is_contradictory() {
        Result::Unsat
    } else {
        unit_propagate(formula);
        if formula.is_consistent() {
            Result::Sat
        } else if formula.is_contradictory() {
            Result::Unsat
        } else {
            let literal = choose_literal(formula);
            let mut f1 = formula.clone();
            set_value(&mut f1, literal);
            if solve(&mut f1) == Result::Sat {
                Result::Sat
            } else {
                let mut f2 = formula.clone();
                set_value(&mut f2, &!literal.clone());
                solve(&mut f2)
            }
        }
    }
}

fn set_value(formula: &mut Formula, literal: &Literal) {
    formula.clauses = formula.clauses.iter()
        .filter(|&clause| !clause.literals.contains(literal))
        .map(|clause| {let mut new_c = clause.clone(); new_c.literals.remove(&!literal.clone()); new_c})
        .collect();
}

fn choose_literal(formula: &Formula) -> &Literal {
    formula.clauses.iter().next().unwrap().literals.iter().next().unwrap()
}

fn unit_propagate(formula: &mut Formula) {
    let mut unit = find_unit(formula);
    while unit.is_some() {
        set_value(formula, &unit.unwrap());
        unit = find_unit(formula);
    }
}

fn find_unit(formula: &Formula) -> Option<Literal> {
    formula.clauses.iter()
        .find(|c| c.is_unit())
        .and_then(|clause| clause.literals.iter().next())
        .cloned()
}

pub fn parse_file(path: &str) -> Formula {
    let mut file = File::open(path).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    parse(contents.as_str())
}

pub fn parse(dimacs: &str) -> Formula {
    Formula {
        clauses: dimacs
            .lines()
            .skip_while(|line| line.starts_with("p") || line.starts_with("c"))
            .map(|line| parse_clause(line))
            .collect()
    }
}

fn parse_clause(line: &str) -> Clause {
    Clause {
        literals: line
            .split_whitespace()
            .map(|num| num.parse::<i64>().unwrap())
            .filter(|num| *num != 0)
            .map(|num| Literal(num.abs() as u64, num >= 0))
            .collect()
    }
}

#[test]
fn test_empty_formula() {
    let mut formula = parse("");
    assert_eq!(solve(&mut formula), Result::Sat);
}

#[test]
fn test_empty_clause() {
    let mut formula = parse("\n");
    assert_eq!(solve(&mut formula), Result::Unsat);
}

#[test]
fn test_contradiction() {
    let mut formula = parse("-1\n1");
    assert_eq!(solve(&mut formula), Result::Unsat);
}

#[test]
fn test_double_positive() {
    let mut formula = parse("1\n1");
    assert_eq!(solve(&mut formula), Result::Sat);
}

#[test]
fn test_double_negative() {
    let mut formula = parse("-1\n-1");
    assert_eq!(solve(&mut formula), Result::Sat);
}

#[test]
fn test_one_clause() {
    let mut formula = parse("-1 -1 1 1");
    assert_eq!(solve(&mut formula), Result::Sat);
}

#[test]
fn test_easy() {
    let mut formula = parse("-1 2 -4\n-2 3 -4");
    assert_eq!(solve(&mut formula), Result::Sat);
}

#[test]
fn test_hard() {
    let mut formula = parse_file("test/hard/flat200-43.txt");
    assert_eq!(solve(&mut formula), Result::Sat);
}
