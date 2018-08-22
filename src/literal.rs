use std::fmt;
use std::ops::Not;

use variable::{VariableName, VariableState};

#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Literal(pub VariableName, pub bool);

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
    pub fn new(num: &i64) -> Literal {
        Literal(num.abs() as usize, *num >= 0)
    }

    pub fn falsified_by(&self, var_val: VariableState) -> bool {
        (var_val == VariableState::True && !self.1) || (self.1 && var_val == VariableState::False)
    }

    pub fn satisfied_by(&self, var_val: VariableState) -> bool {
        (var_val == VariableState::True && self.1) || (var_val == VariableState::False && !self.1)
    }

    pub fn as_num(&self) -> i64 {
        let mut num = self.0 as i64;
        if !self.1 {
            num = num.wrapping_neg();
        }
        num
    }
}
