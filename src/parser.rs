use std::fs::File;
use std::io::Read;
use std::num::ParseIntError;

#[derive(Debug)]
pub struct Dimacs {
    pub clauses: DimacsClauses,
}

type DimacsClauses = Vec<Vec<i64>>;

#[derive(Debug)]
pub struct DimacsError(&'static str);

pub fn parse_file(path: &str) -> Result<Dimacs, DimacsError> {
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

pub fn parse(dimacs: &str) -> Result<Dimacs, DimacsError> {
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
        .collect::<Result<DimacsClauses, ParseIntError>>()
        .map(|clauses| Dimacs { clauses })
        .map_err(|_| DimacsError("Could not parse"))
}
