use super::ast::*;
use nom::IResult;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while1, take_until};
use nom::sequence::{delimited, tuple, preceded};
use nom::combinator::{opt, map, verify};
use nom::multi::{separated_list1, many0};
use nom::character::complete::multispace0;
use std::str::FromStr;
use ordered_float::NotNan;

fn parse_symbol(input: &str) -> IResult<&str, String> {
    let (input, symbol) = verify(
        take_while1(|c: char| c.is_alphanumeric() ||  c == '_'),
        |s: &str| s.chars().next().unwrap().is_ascii_lowercase() || s.chars().next().unwrap() == '_'
    )(input)?;
    let (input, _) = multispace0(input)?;
    Ok((input, symbol.to_string()))
}

fn parse_variable(input: &str) -> IResult<&str, Variable> {
    let (input, variable) = verify(
        take_while1(|c: char| c.is_alphanumeric() ||  c == '_'),
        |s: &str| s.chars().next().unwrap().is_ascii_uppercase() || s.chars().next().unwrap() == '_'
    )(input)?;
    if variable == "_" {
        return Ok((input, Variable::Free));
    }
    Ok((input, Variable::Undistinguished(variable.to_string())))
}

fn parse_float(input: &str) -> IResult<&str, f64> {
    let (input, (int_part, frac_part)) = tuple((
        take_while1(|c: char| c.is_numeric()),
        opt(tuple((tag("."), take_while1(|c: char| c.is_numeric()))))
    ))(input)?;
    let float = match frac_part {
        Some((_, frac_part)) => format!("{}.{}", int_part, frac_part),
        None => int_part.to_string(),
    };
    let float = f64::from_str(&float).unwrap();
    Ok((input, float))
}

fn parse_integer(input: &str) -> IResult<&str, i64> {
    let (input, integer) = take_while1(|c: char| c.is_numeric())(input)?;
    let integer = i64::from_str(integer).unwrap();
    Ok((input, integer))
}

fn parse_boolean(input: &str) -> IResult<&str, bool> {
    let (input, boolean) = alt((
        map(tag("true"), |_| true),
        map(tag("false"), |_| false),
    ))(input)?;
    Ok((input, boolean))
}

fn parse_term(input: &str) -> IResult<&str, Term> {
    let (input, term) = alt((
        map(parse_variable, |variable| Term::Variable(variable)),
        map(parse_float, |float| Term::Constant(Constant::Float(NotNan::new(float).unwrap()))),
        map(parse_integer, |integer| Term::Constant(Constant::Integer(integer))),
        map(parse_symbol, |symbol| Term::Constant(Constant::Symbol(symbol))),
        map(parse_boolean, |boolean| Term::Constant(Constant::Boolean(boolean))),
    ))(input)?;
    let (input, _) = multispace0(input)?;
    Ok((input, term))
}

fn parse_term_list(input: &str) -> IResult<&str, Vec<Term>> {
    let (input, terms) = delimited(
        tuple((multispace0, tag("("), multispace0)),
        separated_list1(tuple(
            (multispace0, tag(","), multispace0)
        ), parse_term),
        tuple((multispace0, tag(")"), multispace0))
    )(input)?;
    Ok((input, terms))
}

fn parse_annotator(input: &str) -> IResult<&str, IO> {
    let (input, io) = alt((
        map(delimited(multispace0, tag("@input"), multispace0), |_| IO::Read(None)),
        map(delimited(multispace0, tag("@output"), multispace0), |_| IO::Write(None)),
    ))(input)?;
    // let (input, io) = alt((
    //     map(delimited(
    //         tuple((tag("@input("), multispace0)),
    //         opt(parse_symbol),
    //         tuple((multispace0, tag(")"), multispace0))
    //     ), |symbol| IO::Read(symbol)),
    //     map(delimited(
    //         tuple((multispace0, tag("@output("), multispace0)),
    //         opt(parse_symbol),
    //         tuple((multispace0, tag(")"), multispace0))
    //     ), |symbol| IO::Write(symbol)),
    // ))(input)?;
    Ok((input, io))
}

fn parse_atom(input: &str) -> IResult<&str, Atom> {
    let (input, negation) = opt(
        tuple((tag("Not"), multispace0)
    ))(input)?;
    let (input, predicate) = parse_symbol(input)?;
    let (input, terms) = parse_term_list(input)?;
    let atom = Atom {
        predicate,
        terms,
        negation: negation.is_some(),
    };
    let (input, _) = multispace0(input)?;
    Ok((input, atom))
}

fn parse_expr(input: &str) -> IResult<&str, Arith> {
    let (input, _) = multispace0(input)?;
    let (input, lhs) = parse_and(input)?;
    let (input, _) = multispace0(input)?;
    let (input, operator) = opt(tag("||"))(input)?;
    let (input, _) = multispace0(input)?;
    match operator {
        Some(_) => {
            let (input, rhs) = parse_and(input)?;
            Ok((input, Arith {
                operator: Operator::Or,
                lhs: Some(Box::new(lhs)),
                rhs: Some(Box::new(rhs)),
            }))
        }
        None => Ok((input, lhs))
    }
}

fn parse_and(input: &str) -> IResult<&str, Arith> {
    let (input, _) = multispace0(input)?;
    let (input, lhs) = parse_equal(input)?;
    let (input, _) = multispace0(input)?;
    let (input, operator) = opt(tag("&&"))(input)?;
    let (input, _) = multispace0(input)?;
    match operator {
        Some(_) => {
            let (input, rhs) = parse_equal(input)?;
            Ok((input, Arith {
                operator: Operator::And,
                lhs: Some(Box::new(lhs)),
                rhs: Some(Box::new(rhs)),
            }))
        }
        None => Ok((input, lhs))
    }
}

fn parse_equal(input: &str) -> IResult<&str, Arith> {
    let (input, _) = multispace0(input)?;
    let (input, lhs) = parse_compare(input)?;
    let (input, _) = multispace0(input)?;
    let (input, operator) = opt(alt(
        (tag("=="), tag("!="))
    ))(input)?;
    let (input, _) = multispace0(input)?;
    match operator {
        Some(operator) => {
            let (input, rhs) = parse_compare(input)?;
            Ok((input, Arith {
                operator: match operator {
                    "==" => Operator::Unifier,
                    "!=" => Operator::Disunifier,
                    _ => unreachable!(),
                },
                lhs: Some(Box::new(lhs)),
                rhs: Some(Box::new(rhs)),
            }))
        }
        None => Ok((input, lhs))
    }
}

fn parse_compare(input: &str) -> IResult<&str, Arith> {
    let (input, _) = multispace0(input)?;
    let (input, lhs) = parse_plus_minus(input)?;
    let (input, _) = multispace0(input)?;
    let (input, operator) = opt(alt(
        (tag("<="), tag(">="), tag("<"), tag(">"))
    ))(input)?;
    let (input, _) = multispace0(input)?;
    match operator {
        Some(operator) => {
            let (input, rhs) = parse_plus_minus(input)?;
            Ok((input, Arith {
                operator: match operator {
                    "<=" => Operator::LessEqual,
                    ">=" => Operator::GreaterEqual,
                    "<" => Operator::Less,
                    ">" => Operator::Greater,
                    _ => unreachable!(),
                },
                lhs: Some(Box::new(lhs)),
                rhs: Some(Box::new(rhs)),
            }))
        }
        None => Ok((input, lhs))
    }
}

fn parse_plus_minus(input: &str) -> IResult<&str, Arith> {
    let (input, _) = multispace0(input)?;
    let (input, lhs) = parse_mul_div(input)?;
    let (input, _) = multispace0(input)?;
    let (input, operator) = opt(alt(
        (tag("+"), tag("-"))
    ))(input)?;
    let (input, _) = multispace0(input)?;
    match operator {
        Some(operator) => {
            let (input, rhs) = parse_mul_div(input)?;
            Ok((input, Arith {
                operator: match operator {
                    "+" => Operator::Add,
                    "-" => Operator::Sub,
                    _ => unreachable!(),
                },
                lhs: Some(Box::new(lhs)),
                rhs: Some(Box::new(rhs)),
            }))
        }
        None => Ok((input, lhs))
    }
}

fn parse_mul_div(input: &str) -> IResult<&str, Arith> {
    let (input, _) = multispace0(input)?;
    let (input, lhs) = parse_unary(input)?;
    let (input, _) = multispace0(input)?;
    let (input, operator) = opt(alt(
        (tag("*"), tag("/"))
    ))(input)?;
    let (input, _) = multispace0(input)?;
    match operator {
        Some(operator) => {
            let (input, rhs) = parse_unary(input)?;
            Ok((input, Arith {
                operator: match operator {
                    "*" => Operator::Mul,
                    "/" => Operator::Div,
                    _ => unreachable!(),
                },
                lhs: Some(Box::new(lhs)),
                rhs: Some(Box::new(rhs)),
            }))
        }
        None => Ok((input, lhs))
    }
}

fn parse_unary(input: &str) -> IResult<&str, Arith> {
    let (input, _) = multispace0(input)?;
    let (input, operator) = opt(alt(
        (tag("!"), tag("-"))
    ))(input)?;
    let (input, _) = multispace0(input)?;
    match operator {
        Some(operator) => {
            let (input, rhs) = parse_unary(input)?;
            Ok((input, Arith {
                operator: match operator {
                    "!" => Operator::Neg,
                    "-" => Operator::Sub,
                    _ => unreachable!(),
                },
                lhs: None,
                rhs: Some(Box::new(rhs)),
            }))
        }
        None => parse_primary(input)
    }
}

fn parse_primary(input: &str) -> IResult<&str, Arith> {
    let (input, _) = multispace0(input)?;
    let (input, parenthesis) = opt(tag("("))(input)?;
    let (input, _) = multispace0(input)?;
    let (input, term) = parse_term(input)?;
    match parenthesis {
        Some(_) => {
            let (input, _) = multispace0(input)?;
            let (input, _) = tag(")")(input)?;
            Ok((input, Arith {
                operator: Operator::Leaf(term),
                lhs: None,
                rhs: None
            }))
        }
        None => Ok((input, Arith {
            operator: Operator::Leaf(term),
            lhs: None,
            rhs: None
        }))
    }
}

fn parse_clause(input: &str) -> IResult<&str, Clause> {
    let (input, clause) = alt((
        map(parse_atom, |atom| Clause::Atom(atom)),
        map(parse_expr, |expr| Clause::Arithmetic(expr)),
    ))(input)?;
    Ok((input, clause))
}

fn parse_rules(input: &str) -> IResult<&str, Rule> {
    let (input, annotator) = opt(parse_annotator)(input)?;
    let io = annotator.unwrap_or(IO::Silent);
    let (input, head) = parse_atom(input)?;
    let (input, define) = opt(tag(":-"))(input)?;
    let (mut input, _) = multispace0(input)?;
    let mut body = Vec::new();
    if define.is_some() {
        let (input_inner, clauses) = delimited(
            multispace0,
            separated_list1(tuple(
                (multispace0, tag(","), multispace0)
            ), parse_clause),
            multispace0,
        )(input)?;
        body = clauses;
        input = input_inner;
    }
    let rule = Rule { io, head, body };
    Ok((input, rule))
}

fn parse_comment(input: &str) -> IResult<&str, &str> {
    let (input, comment) = preceded(
        tuple((multispace0, tag("%"), multispace0)),
        take_until("\n")
    )(input)?;
    let (input, _) = multispace0(input)?;
    Ok((input, comment))
}

pub fn parse_program(input: &str) -> IResult<&str, Program> {
    let (input, _) = multispace0(input)?;
    let (input, rules) = many0(alt((
        map(parse_comment, |_| None),
        map(parse_rules, Some),
    )))(input)?;
    let rules = rules.into_iter().flatten().collect();
    Ok((input, rules))
}
