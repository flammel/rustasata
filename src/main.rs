#[macro_use]
extern crate log;
extern crate fern;

use std::env;
use std::time::Instant;

mod clause;
mod literal;
mod parser;
mod solver;
mod variable;

use parser::parse_file;
use solver::Solver;

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

#[cfg(test)]
mod tests {
    use super::*;
    use parser::parse;
    use solver::SolverResult;

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
