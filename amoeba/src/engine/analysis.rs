use super::ast::*;
use super::context::Context;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub enum DataType {
    Integer,
    Float,
    Symbol,
}

#[derive(Clone, Debug)]
pub struct VarGroup {
    pub is_arith: bool,
    pub clause_index: usize,
    pub term_indexes: Vec<usize>,
}

impl VarGroup {
    pub fn contain_duplicate(&self) -> bool {
        self.term_indexes.len() > 1
    }
}

#[derive(Clone, Debug)]
pub struct VarDict {
    pub head_dict: HashMap<String, Vec<usize>>,
    pub clause_dict: HashMap<String, Vec<VarGroup>>
}

impl VarDict {
    pub fn new(rule: &Rule) -> Self {
        let mut clause_dict = HashMap::new();
        let mut head_dict = HashMap::new();
        rule.head.terms.iter().enumerate().for_each(|(index, term)| {
            if let Some(var) = term.is_nontrivial_variable() {
                head_dict.entry(var)
                    .or_insert(Vec::new())
                    .push(index);
            }
        });
        rule.body.iter().enumerate().for_each(|(clause_index, clause)| {
            match clause {
                Clause::Atom(atom) => {
                    let var_group_template = VarGroup {
                        is_arith: false,
                        clause_index,
                        term_indexes: Vec::new(),
                    };
                    atom.terms.iter().enumerate().for_each(|(term_index, term)| {
                        if let Some(var) = term.is_nontrivial_variable() {
                            let entry = clause_dict.entry(var)
                                .or_insert(Vec::new());
                            if entry.is_empty() {
                                let mut var_group = var_group_template.clone();
                                var_group.term_indexes.push(term_index);
                                entry.push(var_group);
                            } else {
                                let mut same_clause = false;
                                entry.iter_mut().for_each(|var_group| {
                                    if var_group.clause_index == clause_index {
                                        var_group.term_indexes.push(term_index);
                                        same_clause = true;
                                    }
                                });
                                if !same_clause {
                                    let mut var_group = var_group_template.clone();
                                    var_group.term_indexes.push(term_index);
                                    entry.push(var_group);
                                }
                            }
                        }
                    });
                }
                Clause::Arithmetic(arith) => {
                    let var_group_template = VarGroup {
                        is_arith: true,
                        clause_index,
                        term_indexes: Vec::new(),
                    };
                    arith.get_leaves().iter().enumerate().for_each(|(term_index, term)| {
                        if let Some(var) = term.is_nontrivial_variable() {
                            let entry = clause_dict.entry(var)
                                .or_insert(Vec::new());
                            if entry.is_empty() {
                                let mut var_group = var_group_template.clone();
                                var_group.term_indexes.push(term_index);
                                entry.push(var_group);
                            } else {
                                entry.iter_mut().for_each(|var_group| {
                                    if var_group.clause_index == clause_index {
                                        var_group.term_indexes.push(term_index);
                                    }
                                });
                            }
                        }
                    });
                }
            }
        });
        Self { head_dict, clause_dict }
    }

    pub fn alloc(&self, var: &String) -> HashSet<(usize, usize)> {
        let mut distinguished_vars = HashSet::new();
        let groups = self.clause_dict.get(var).expect("Invalid var");
        groups.iter().for_each(|group| {
            group.term_indexes.iter().for_each(|term_index| {
                distinguished_vars.insert((group.clause_index, *term_index));
            });
        });
        distinguished_vars
    }
}

pub struct Analyzer {
    pub data_types: HashMap<String, Vec<DataType>>,
}

impl Analyzer {
    pub fn new() -> Self {
        Self {
            data_types: HashMap::new(),
        }
    }

    pub fn type_inference(&mut self, context: &Context) {
        context.edbs.iter().for_each(|(name, rule)| {
            let mut types = Vec::new();
            rule.head.terms.iter().for_each(|term| {
                if let Term::Constant(Constant::Symbol(type_)) = term {
                    match type_.as_str() {
                        "int" => types.push(DataType::Integer),
                        "float" => types.push(DataType::Float),
                        "sym" => types.push(DataType::Symbol),
                        _ => panic!("Invalid type: {}", type_),
                    }
                } else {
                    panic!("Invalid type: {:?}", term);
                }
            });
            self.data_types.insert(name.clone(), types);
        });
        // inference types for IDBs
        // IDBs' term types should be inferred from base cases
        let mut previous = context.edbs
            .keys()
            .cloned()
            .collect::<Vec<String>>();
        let queue = context.ordered_idbs();
        queue.iter().for_each(|name| {
            let rules = context.idbs.get(name)
                .expect("IDB should be present in context");
            let base_cases = rules.iter()
                .filter(|rule| rule.is_base_case(&previous))
                .collect::<Vec<&Rule>>();
            base_cases.iter().for_each(|&rule| {
                // for each term in the body, if it is distinguished
                // then annotate it with the type of the declared type
                let mut types = HashMap::new();
                rule.body.iter().for_each(|clause| {
                    if let Clause::Atom(atom) = clause {
                        atom.terms.iter().enumerate().for_each(|(i, term)| {
                            if let Term::Variable(Variable::Distinguished(var)) = term {
                                let type_ = self.data_types.get(&atom.predicate)
                                    .expect("EDB should be present in context")
                                    .get(i)
                                    .expect("Term should be present in EDB");
                                // if var is already in types, then check if the type is the same
                                // else insert the type
                                types.entry(var).or_insert(type_);
                            }
                        });
                    }
                });
                // check if all terms in the head have been annotated
                rule.head.terms.iter().for_each(|term| {
                    if let Term::Variable(Variable::Distinguished(var)) = term {
                        if !types.contains_key(var) {
                            panic!("Term `{}` in `{}` should be annotated", var, rule.head.predicate);
                        }
                    }
                });
                // convert types into vector following the order of the head terms
                let types_vec = rule.head.terms.iter().map(|term| {
                    if let Term::Variable(Variable::Distinguished(var)) = term {
                        let type_ = types.get(var)
                            .expect("Term should be present in types").clone();
                        type_.to_owned()
                    } else {
                        panic!("Term should be distinguished variable");
                    }
                }).collect::<Vec<DataType>>();
                self.data_types.insert(rule.head.predicate.clone(), types_vec);
            });
            previous.push(name.clone());
        });
    }
}