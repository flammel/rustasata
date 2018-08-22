use std::cell::RefCell;
use std::rc::Rc;

use clause::Clause;
use literal::Literal;

pub type VariableName = usize;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum VariableState {
    True,
    False,
    Open,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Variable {
    pub name: VariableName,
    pub state: VariableState,
    pub watched_pos: Vec<Rc<RefCell<Clause>>>,
    pub watched_neg: Vec<Rc<RefCell<Clause>>>,
    pub occurences: usize,
}

impl Variable {
    pub fn new(literal: &Literal) -> Variable {
        Variable {
            name: literal.0,
            state: VariableState::Open,
            watched_neg: Vec::new(),
            watched_pos: Vec::new(),
            occurences: 0,
        }
    }

    pub fn watch(&mut self, sign: bool, clause: Rc<RefCell<Clause>>) {
        if sign {
            self.watched_pos.push(clause);
        } else {
            self.watched_neg.push(clause);
        }
    }

    pub fn unwatch(&mut self, sign: bool, clause: &Rc<RefCell<Clause>>) {
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
