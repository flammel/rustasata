# Rustasata

* [https://github.com/strelec/DPLL-with-Rust](https://github.com/strelec/DPLL-with-Rust)
* [https://en.wikipedia.org/wiki/DPLL_algorithm](https://en.wikipedia.org/wiki/DPLL_algorithm)
* [https://en.wikipedia.org/wiki/Conflict-Driven_Clause_Learning](https://en.wikipedia.org/wiki/Conflict-Driven_Clause_Learning)
* [https://baldur.iti.kit.edu/sat/](https://baldur.iti.kit.edu/sat/)
* [https://github.com/haz/sympy/blob/a4facd8d6d94f54c0fbea48806b7858bc82d99bb/sympy/logic/algorithms/dpll2.py](https://github.com/haz/sympy/blob/a4facd8d6d94f54c0fbea48806b7858bc82d99bb/sympy/logic/algorithms/dpll2.py)
* [http://haz-tech.blogspot.com/2010/07/clause-learning-flunk.html](http://haz-tech.blogspot.com/2010/07/clause-learning-flunk.html)
* [http://haz-tech.blogspot.com/2010/08/whos-watching-watch-literals.html](http://haz-tech.blogspot.com/2010/08/whos-watching-watch-literals.html)
* [http://www.cs.ubc.ca/~hoos/SATLIB/benchm.html](http://www.cs.ubc.ca/~hoos/SATLIB/benchm.html)
* [https://toughsat.appspot.com/](https://toughsat.appspot.com/)
* [https://www.cs.cmu.edu/afs/cs/project/jair/pub/volume21/dixon04a-html/node3.html](https://www.cs.cmu.edu/afs/cs/project/jair/pub/volume21/dixon04a-html/node3.html)


From [https://baldur.iti.kit.edu/sat/files/l07.pdf](https://baldur.iti.kit.edu/sat/files/l07.pdf)

```
boolean DPLL(ClauseSet S) {
    α = ∅, Trail = new Stack()
    while (not all variables assigned in α) {
        if (unitPropagation(S, α) = CONFLICT) {
            L = the last literal in Trail not tried both True and False
            if (no such L) return UNSATISFIABLE
            α = unassign all literals after L in Trail
            pop all literals after L in Trail
            α = (α \ {L}) ∪ {¬L}
        } else {
            L = pick an unassigned literal
            add {L = 1} to α
            Trail.push(L)
        }
    }
    return SATISFIABLE
}

boolean CDCL(ClauseSet S) {
    α = ∅, Trail = new Stack()
    while (not all variables assigned in α) {
        if (unitPropagation(S, α) = CONFLICT) {
            analyze the conflict which gives us:
            - a new learned clause C, S = S ∪ {C}
            - if C = ∅ return UNSATISFIABLE
            - a literal L in the Trail to which we backtrack
            update α and Trail according to L
        } else {
            L = pick an unassigned literal
            add {L = 1} to α
            Trail.push(L)
        }
    }
    return SATISFIABLE
}
```