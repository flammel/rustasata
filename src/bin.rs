extern crate fern;
extern crate log;
extern crate rustasata;

use std::env;
use std::time::Instant;

use rustasata::parser::parse_file;
use rustasata::solver::Solver;

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
