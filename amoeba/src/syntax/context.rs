use super::ast::*;
use super::stratify::Stratum;
use std::collections::{HashSet, HashMap};

#[derive(Clone)]
pub struct Context {
    pub stratum: Stratum,
    pub edbs: HashMap<String, Rule>,
    pub idbs: HashMap<String, Vec<Rule>>,
    pub queries: HashMap<String, Rule>,
}

impl Context {
    pub fn new(program: &Program) -> Self {
        let mut edbs = HashMap::new();
        let mut idbs = HashMap::new();
        let mut queries = HashMap::new();
        program.iter().for_each(|rule| {
            let name = rule.head.predicate.clone();
            match rule.io {
                IO::Read(_) => {
                    edbs.insert(name, rule.clone());
                }
                IO::Write(_) => {
                    queries.insert(name, rule.clone());
                }
                IO::Silent => {
                    let rules = idbs.entry(name)
                        .or_insert(Vec::new());
                    rules.push(rule.clone());
                }
            };
        });
        let mut predicates = HashSet::new();
        // name resolution for edbs
        edbs.iter().for_each(|(name, _)| {
            if predicates.contains(name) {
                panic!("Duplicated predicate: {}", name);
            }
            predicates.insert(name.clone());
        });
        // check the validation of atom in head of an idb
        let check_head = |atom: &Atom| {
            atom.terms.iter().for_each(|term| {
                if let Term::Variable(Variable::Free) = term {
                    panic!("Free variable in head of idb: {}", atom.predicate)
                }
            });
        };
        // check the validation of atom in clauses of an idb
        let check_atom= |atom: &Atom| {
            let name = atom.predicate.clone();
            if !predicates.contains(&name) {
                // find a predicate in idb
                if !idbs.contains_key(&name) {
                    panic!("Undefined predicate: {}", name);
                }
            }
        };
        let mut dependencies = HashSet::new();
        idbs.iter().for_each(|(name, rules)| {
            if predicates.contains(name) {
                panic!("predicate declared as both idb and edb: {}", name);
            }
            for rule in rules {
                check_head(&rule.head);
                rule.body.iter().for_each(|clause| {
                    if let Clause::Atom(atom) = clause {
                        check_atom(atom);
                        dependencies.insert((name, &atom.predicate));
                    }
                });
            }
        });
        // check stratum
        let stratum = Stratum::new(predicates, dependencies);
        let check_stratum = |head_level: usize, clauses: &Vec<Clause>| {
            for clause in clauses.iter() {
                if let Clause::Atom(atom) = clause {
                    if !atom.negation {
                        continue;
                    }
                    let level = stratum.get_level(&atom.predicate);
                    match head_level.cmp(&level) {
                        std::cmp::Ordering::Less => panic!("Cyclic dependency: {:?}", atom),
                        std::cmp::Ordering::Equal => panic!("Mutual dependency: {:?}", atom),
                        std::cmp::Ordering::Greater => {}
                    }
                }
            }
        };
        idbs.iter().for_each(|(name, rules)| {
            let level = stratum.get_level(name);
            for rule in rules {
                check_stratum(level, &rule.body);
            }
        });
        // check variable safety
        // a rule is safe if:
        // 1. each distinguished variable
        // 2. each variable in arithmetic subgoal
        // 3. each variable in a negated subgoal
        // also appears in a non-negated, relational subgoal
        idbs.iter_mut().for_each(|(_, rules)| {
            rules.iter_mut().for_each(|rule| {
                rule.annotate_variable();
            })
        });
        // print stratum
        // stratum.strata.iter().enumerate().for_each(|(level, predicates)| {
        //     println!("Stratum {}", level);
        //     predicates.iter().for_each(|predicate| {
        //         println!("  {}", predicate);
        //     });
        // });
        Self { stratum, edbs, idbs, queries }
    }

    pub fn ordered_idbs(&self) -> Vec<String> {
        // give queue of idbs' name according to stratum's order
        // filter stratum's name that is an edb
        let mut queue = Vec::new();
        self.stratum.strata.iter().for_each(|predicates| {
            predicates.iter().for_each(|predicate| {
                if !self.edbs.contains_key(predicate) {
                    queue.push(predicate.clone());
                }
            });
        });
        queue
    }

    pub fn queries(&self) -> Vec<String> {
        // return all queries' name
        self.queries.keys().map(|name| name.clone()).collect()
    }
}