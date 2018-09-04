extern crate priority_queue;

use std::cmp::Ordering;

use self::priority_queue::PriorityQueue;

use literal::Literal;
use variable::{Variable, VariableName, VariableState, Variables};

#[derive(Debug, Eq, PartialEq)]
struct VariablePriority(usize, bool, usize, usize);

impl PartialOrd for VariablePriority {
    fn partial_cmp(&self, other: &VariablePriority) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VariablePriority {
    fn cmp(&self, other: &VariablePriority) -> Ordering {
        if self.1 && !other.1 {
            Ordering::Less
        } else if !self.1 && other.1 {
            Ordering::Greater
        } else {
            self.0.cmp(&other.0)
        }
    }
}

impl VariablePriority {
    fn new(var: &Variable) -> VariablePriority {
        VariablePriority(
            var.occurences,
            var.state != VariableState::Open,
            var.occurences_positive,
            var.occurences_negative,
        )
    }

    fn set(&self) -> VariablePriority {
        VariablePriority(self.0, true, self.2, self.3)
    }

    fn unset(&self) -> VariablePriority {
        VariablePriority(self.0, false, self.2, self.3)
    }

    fn literal(&self, vname: VariableName) -> Option<Literal> {
        if self.1 {
            None
        } else {
            Some(Literal(vname, self.2 > self.3))
        }
    }
}

#[derive(Debug)]
pub struct DecisionProvider {
    variable_queue: PriorityQueue<VariableName, VariablePriority>,
}

impl DecisionProvider {
    pub fn new() -> DecisionProvider {
        DecisionProvider {
            variable_queue: PriorityQueue::new(),
        }
    }

    pub fn get_next(&self) -> Option<Literal> {
        self.variable_queue
            .peek()
            .and_then(|(i, prio)| prio.literal(*i))
    }

    pub fn unset(&mut self, literal: Literal) {
        self.variable_queue
            .change_priority_by(&literal.0, |prio| prio.unset());
    }

    pub fn set(&mut self, literal: Literal) {
        self.variable_queue
            .change_priority_by(&literal.0, |prio| prio.set());
    }

    pub fn init(&mut self, variables: &Variables) {
        self.variable_queue = variables
            .values()
            .map(|var| (var.name, VariablePriority::new(var)))
            .collect();
    }
}
