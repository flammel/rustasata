use std::fmt;
use std::ops::Not;

use solver::VariableName;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Literal(pub i64);

impl Not for Literal {
    type Output = Literal;
    fn not(self) -> Literal {
        Literal(self.0.wrapping_neg())
    }
}

impl fmt::Debug for Literal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Literal {
    pub fn new(num: &i64) -> Literal {
        Literal(*num)
    }

    pub fn index(&self) -> usize {
        if self.sign() {
            self.0 as usize * 2
        } else {
            self.0.abs() as usize * 2 - 1
        }
    }

    pub fn sign(&self) -> bool {
        self.0.is_positive()
    }

    pub fn var(&self) -> VariableName {
        self.0.abs() as usize
    }

    pub fn falsified_by(&self, var_val: Option<&bool>) -> bool {
        match var_val {
            None => false,
            Some(val) => *val != self.sign(),
        }
    }

    pub fn satisfied_by(&self, var_val: Option<&bool>) -> bool {
        match var_val {
            None => false,
            Some(val) => *val == self.sign(),
        }
    }
}
