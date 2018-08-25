use std::cell::RefCell;
use std::rc::Rc;

use clause::Clause;

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
    pub antecedent: Option<Rc<RefCell<Clause>>>,
    pub decision_level: i64,
}

impl Variable {
    pub fn new(name: VariableName) -> Variable {
        Variable {
            name: name,
            state: VariableState::Open,
            watched_neg: Vec::new(),
            watched_pos: Vec::new(),
            occurences: 0,
            antecedent: None,
            decision_level: -1,
        }
    }

    pub fn set(&mut self, to_value: bool) -> Result<Vec<Rc<RefCell<Clause>>>, ()> {
        match (to_value, self.state) {
            (true, VariableState::False) => Err(()),
            (false, VariableState::True) => Err(()),
            (true, _) => {
                self.state = VariableState::True;
                Ok(self.watched_neg.clone())
            }
            (false, _) => {
                self.state = VariableState::False;
                Ok(self.watched_pos.clone())
            }
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
