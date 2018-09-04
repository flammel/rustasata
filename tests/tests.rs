extern crate fern;
extern crate log;
extern crate rustasata;

use std::env;
use std::fs;

use rustasata::parser::{parse, parse_file};
use rustasata::solver::{Solver, SolverResult};

fn setup_logger() -> Result<(), fern::InitError> {
    if env::var("LOG").is_ok() {
        fern::Dispatch::new()
            .format(|out, message, _| out.finish(format_args!("{}", message)))
            .level(log::LevelFilter::Trace)
            .chain(std::io::stdout())
            .chain(fern::log_file("output.log")?)
            .apply()?;
    }
    Ok(())
}

fn run_test(str: &str) -> SolverResult {
    let _ = setup_logger();
    let dimacs = parse(str).unwrap();
    Solver::from_dimacs(dimacs).solve()
}

fn run_test_file(str: &str) -> SolverResult {
    let _ = setup_logger();
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
fn test_file_trivial_1() {
    let result = run_test_file("test/trivial/coloring_pref_1000_2000.txt");
    assert_eq!(result, SolverResult::Sat);
}

#[test]
#[ignore]
fn test_file_trivial_2() {
    let result = run_test_file("test/trivial/graph4.txt");
    assert_eq!(result, SolverResult::Sat);
}

#[test]
#[ignore]
fn test_file_trivial_3() {
    let result = run_test_file("test/trivial/h_sudoku2.txt");
    assert_eq!(result, SolverResult::Sat);
}

#[test]
#[ignore]
fn test_file_trivial_4() {
    let result = run_test_file("test/trivial/officialSample.txt");
    assert_eq!(result, SolverResult::Sat);
}

#[test]
#[ignore]
fn test_file_trivial_5() {
    let result = run_test_file("test/trivial/r5.txt");
    assert_eq!(result, SolverResult::Sat);
}

#[test]
#[ignore]
fn test_file_trivial_6() {
    let result = run_test_file("test/trivial/random.txt");
    assert_eq!(result, SolverResult::Sat);
}

#[test]
#[ignore]
fn test_file_trivial_7() {
    let result = run_test_file("test/trivial/test10.dimacs");
    assert_eq!(result, SolverResult::Sat);
}

#[test]
#[ignore]
fn test_file_easy_1() {
    let result = run_test_file("test/easy/flat200-89.txt");
    assert_eq!(result, SolverResult::Sat);
}

#[test]
#[ignore]
fn test_file_satlib_uf20_91() {
    let files = fs::read_dir("test/satlib/uf20-91").unwrap();
    for file in files {
        let file = file.unwrap().path();
        let file = file.to_str().unwrap();
        assert_eq!(run_test_file(file), SolverResult::Sat, "file: {}", file);
    }
}

#[test]
#[ignore]
fn test_file_satlib_uf125_538() {
    let files = fs::read_dir("test/satlib/uf125-538").unwrap();
    for file in files {
        let file = file.unwrap().path();
        let file = file.to_str().unwrap();
        assert_eq!(run_test_file(file), SolverResult::Sat, "file: {}", file);
    }
}

#[test]
#[ignore]
fn test_file_satlib_uuf125_538() {
    let files = fs::read_dir("test/satlib/uuf125-538").unwrap();
    for file in files {
        let file = file.unwrap().path();
        let file = file.to_str().unwrap();
        assert_eq!(run_test_file(file), SolverResult::Unsat, "file: {}", file);
    }
}
