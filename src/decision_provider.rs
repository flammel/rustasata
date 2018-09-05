extern crate priority_queue;

use std::cmp::Ordering;

use self::priority_queue::PriorityQueue;

use literal::Literal;
use solver::VariableName;

#[derive(Debug, Eq, PartialEq)]
struct VariablePriority(bool, usize, usize, usize);

impl PartialOrd for VariablePriority {
    fn partial_cmp(&self, other: &VariablePriority) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VariablePriority {
    fn cmp(&self, other: &VariablePriority) -> Ordering {
        if self.0 && !other.0 {
            Ordering::Less
        } else if !self.0 && other.0 {
            Ordering::Greater
        } else {
            self.1.cmp(&other.1)
        }
    }
}

impl VariablePriority {
    fn new(literal: &Literal) -> VariablePriority {
        VariablePriority(
            false,
            1,
            if literal.sign() { 1 } else { 0 },
            if literal.sign() { 0 } else { 1 },
        )
    }

    fn occ(&self, literal: &Literal) -> VariablePriority {
        VariablePriority(
            self.0,
            self.1 + 1,
            self.2 + if literal.sign() { 1 } else { 0 },
            self.3 + if literal.sign() { 0 } else { 1 },
        )
    }

    fn set(&self) -> VariablePriority {
        VariablePriority(true, self.1, self.2, self.3)
    }

    fn unset(&self) -> VariablePriority {
        VariablePriority(false, self.1, self.2, self.3)
    }

    fn literal(&self, vname: VariableName) -> Option<Literal> {
        if self.0 {
            None
        } else {
            Some(Literal(if self.2 > self.3 {
                vname as i64
            } else {
                (vname as i64).wrapping_neg()
            }))
        }
    }
}

#[derive(Debug)]
pub struct DecisionProvider {
    queue: PriorityQueue<VariableName, VariablePriority>,
}

impl DecisionProvider {
    pub fn new() -> DecisionProvider {
        DecisionProvider {
            queue: PriorityQueue::new(),
        }
    }

    pub fn new_clause(&mut self, literals: &Vec<Literal>) {
        for literal in literals {
            if self.queue.get(&literal.var()).is_none() {
                self.queue
                    .push(literal.var(), VariablePriority::new(literal));
            } else {
                self.queue
                    .change_priority_by(&literal.var(), |prio| prio.occ(literal))
            }
        }
    }

    pub fn get_next(&self) -> Option<Literal> {
        self.queue.peek().and_then(|(i, prio)| prio.literal(*i))
    }

    pub fn unset(&mut self, var: VariableName) {
        self.queue.change_priority_by(&var, |prio| prio.unset());
    }

    pub fn set(&mut self, var: VariableName) {
        self.queue.change_priority_by(&var, |prio| prio.set());
    }
}
