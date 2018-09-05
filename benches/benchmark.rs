#[macro_use]
extern crate criterion;
extern crate rustasata;

use criterion::Criterion;

use rustasata::parser::parse_file;
use rustasata::solver::Solver;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("uf125-538-01", |b| {
        b.iter(|| {
            let dimacs = parse_file("test/satlib/uf125-538/uf125-01.cnf").unwrap();
            Solver::from_dimacs(&dimacs).solve()
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
