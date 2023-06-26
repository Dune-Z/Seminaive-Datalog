use ordered_float::NotNan;
use std::collections::HashSet;
use std::fmt::Display;

/// [`Rule`] can either be an edb or idb or query.
/// a Datalog program is a set of rules
pub type Program = Vec<Rule>;
#[derive(Debug, Clone)]
pub struct Rule {
    pub io: IO,
    pub head: Atom,
    pub body: Vec<Clause>,
}

impl Rule {
    pub fn annotate_variable(&mut self) {
        let mut distinguished_variables = HashSet::new();
        self.head.terms.iter_mut().for_each(|term| {
            if let Term::Variable(variable) = term {
                // convert variable to distinguished
                if let Variable::Undistinguished(name) = variable {
                    let distinguished = Variable::Distinguished(name.clone());
                    *variable = distinguished;
                }
                if let Variable::Distinguished(name) = variable {
                    distinguished_variables.insert(name);
                }
            }
        });
        self.body.iter_mut().for_each(|clause| {
            match clause {
                Clause::Atom(atom) => {
                    atom.terms.iter_mut().for_each(|term| {
                        if let Term::Variable(variable) = term {
                            if let Variable::Undistinguished(name) = variable {
                                if distinguished_variables.contains(name) {
                                    let distinguished = Variable::Distinguished(name.clone());
                                    *variable = distinguished;
                                }
                            }
                        }
                    });
                }
                Clause::Arithmetic(arith) => {
                    arith.get_leaves().iter_mut().for_each(|leaf| {
                        if let Term::Variable(variable) = leaf {
                            if let Variable::Undistinguished(name) = variable {
                                if distinguished_variables.contains(name) {
                                    let distinguished = Variable::Distinguished(name.clone());
                                    *variable = distinguished;
                                }
                            }
                        }
                    });
                }
            }
        });
    }

    pub fn is_base_case(&self, predicates: &Vec<String>) -> bool {
        // body only contains edb
        self.body.iter().all(|clause| {
            match clause {
                Clause::Atom(atom) => predicates.contains(&atom.predicate),
                Clause::Arithmetic(_) => false,
            }
        })
    }
}

/// [`Clause`] is an atom or a arithmetic expression.
/// arithmetic expression is used in the body of a idb.
/// only atom in the body of a idb can be negated.
#[derive(Debug, Clone)]
pub enum Clause {
    Atom(Atom),
    Arithmetic(Arith),
}

impl Clause {
    pub fn to_string(&self) -> String {
        match self {
            Clause::Atom(atom) => atom.predicate.clone(),
            Clause::Arithmetic(_) => String::from("arith")
        }
    }
}

impl Display for Clause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Clause::Atom(atom) => write!(f, "{}", atom.to_string()),
            Clause::Arithmetic(_) => write!(f, "arith")
        }
    }
}

/// [`IO`] marks the input or output of a predicate.
/// IO annotation starts with @.
/// @input(file) reads file.csv as input to edb.
/// @output(file) writes output of query to file.csv.
/// @output() writes output of query to stdout.
#[derive(Debug, Clone)]
pub enum IO {
    Read(Option<String>),
    Write(Option<String>),
    Silent
}

/// [`Atom`] is a predicate with terms.
/// path(X, b) is a predicate with terms X and b.
#[derive(Debug, Clone)]
pub struct Atom {
    pub negation: bool,
    pub predicate: String,
    pub terms: Vec<Term>
}

impl Atom {
    pub fn to_string(&self) -> String {
        let mut string = String::new();
        string.push_str(&self.predicate);
        string.push('(');
        for (i, _) in self.terms.iter().enumerate() {
            let term_string = String::from(format!("column_{}", i));
            string.push_str(&term_string);
            if i != self.terms.len() - 1 {
                string.push_str(", ");
            }
        }
        string.push(')');
        string
    }
}

impl Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = String::new();
        string.push_str(&self.predicate);
        string.push('(');
        self.terms.iter().enumerate().for_each(|(i, term)| {
            string.push_str(&term.to_string());
            if i != self.terms.len() - 1 {
                string.push_str(", ");
            }
        });
        string.push(')');
        if self.negation {
            write!(f, "not {}", string)
        } else {
            write!(f, "{}", string)
        }
    }
}

/// [`Term`] represents a term of a predicate.
/// path(X, b) has terms X and b.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Term {
    Variable(Variable),
    Constant(Constant)
}

impl Term {
    pub fn is_nontrivial_variable(&self) -> Option<String> {
        // if the term contains distinguished variables or undistinguished variables
        if let Term::Variable(Variable::Distinguished(var)) |
        Term::Variable(Variable::Undistinguished(var)) = self {
            Some(var.clone())
        } else {
            None
        }
    }
}

impl Display for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Term::Variable(variable) => write!(f, "{}", variable),
            Term::Constant(constant) => write!(f, "{}", constant),
        }
    }
}

/// [`Variable`] represents a variable of a term.
/// path(X, Y) has variables X and Y.
/// variable appears in head of a idb predicate is distinguished.
/// variable appears in body of a idb predicate is undistinguished.
/// underscore(_) represents a free variable.
/// variable should be capitalized.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Variable {
    Distinguished(String),
    Undistinguished(String),
    Free
}

impl Display for Variable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variable::Distinguished(name) => write!(f, "{}", name),
            Variable::Undistinguished(name) => write!(f, "{}", name),
            Variable::Free => write!(f, "_"),
        }
    }
}

/// [`Constant`] represents a constant value of a term.
/// edge(a, b) has constant value a and b, with type `Constant::Symbol`.
/// constant value should be lowercase.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Constant {
    Integer(i64),
    Float(NotNan<f64>),
    Symbol(String),
    Boolean(bool),
}

// impl Display for constant
impl Display for Constant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Constant::Integer(value) => write!(f, "{}", value),
            Constant::Float(value) => write!(f, "{}", value),
            Constant::Symbol(value) => write!(f, "'{}'", value),
            Constant::Boolean(value) => write!(f, "{}", value),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Operator {
    Unifier,
    Disunifier,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    Neg,
    Add,
    Sub,
    Mul,
    Div,
    Leaf(Term),
}

#[derive(Debug, Clone)]
pub struct Arith {
    pub operator: Operator,
    pub lhs: Option<Box<Arith>>,
    pub rhs: Option<Box<Arith>>,
}

impl Arith {
    pub fn get_leaves(&self) -> Vec<Term> {
        let mut leaves = Vec::new();
        match &self.operator {
            Operator::Leaf(term) => {
                leaves.push(term.clone());
            },
            _ => {
                if let Some(lhs) = &self.lhs {
                    leaves.append(&mut lhs.get_leaves());
                }
                if let Some(rhs) = &self.rhs {
                    leaves.append(&mut rhs.get_leaves());
                }
            }
        }
        leaves
    }
}
