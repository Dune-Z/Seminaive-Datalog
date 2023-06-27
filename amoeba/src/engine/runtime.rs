use super::context::Context;
use super::ast::*;
use super::analysis::*;
use super::parse;
use core::panic;
use rusqlite::{Connection, params, Result, backup::Backup};
use colored::Colorize;
use std::error::Error;
use std::time::Duration;
use std::collections::HashSet;
use std::collections::HashMap;

pub struct Runtime {
    source_db: String,
    verbose: bool,
    context: Context,
    analyzer: Analyzer,
    database: Connection
}

impl Runtime {
    pub fn new(source_path: &str, verbose: bool) -> Result<Self, Box<dyn Error>> {
        let context = parse(source_path);
        // database name is the same as source name, but replace postfix .amo with .db
        let mut parts = source_path.rsplitn(2, '.').collect::<Vec<&str>>();
        if let Some(index) = parts.iter_mut()
            .position(|&mut part| part == "amo") {
                parts[index] = "db";
        }
        let source_db = parts.into_iter()
            .rev()
            .collect::<Vec<&str>>()
            .join(".");
        if verbose {
            println!("{}: {}", "LOADING".green(), source_db);
        }
        let database_disk = Connection::open(source_db.clone())?;
        let mut database = Connection::open_in_memory()?;
        // check if all edbs are present in database
        for (table, rule) in context.edbs.iter() {
            let sql = format!("SELECT name FROM sqlite_master WHERE type='table' AND name='{}';", table);
            let mut stmt = database_disk.prepare(&sql)?;
            let mut rows = stmt.query(params![])?;
            let rows_exist = rows.next()?;
            if rows_exist.is_none() {
                panic!("EDB {} is not present in database", table);
            }
            let arity = rule.head.terms.len();
            // check if ebd table has the same arity as in the rule
            let count_column = format!("PRAGMA table_info({})", table);
            let mut count_stmt = database_disk.prepare(&count_column)?;
            let count_rows = count_stmt.query_map(params![], |row| {
                let name: String = row.get(1)?;
                Ok(name)
            })?;
            let actual_arity = count_rows.count();
            assert_eq!(arity, actual_arity);
        }
        // clone database to memory
        {
            let backup = Backup::new(&database_disk, &mut database)?;
            backup.run_to_completion(5, Duration::from_millis(1), None)?;
        }
        database_disk.close().unwrap();
        let mut analyzer = Analyzer::new();
        analyzer.type_inference(&context);
        Ok(Self {
            source_db,
            verbose,
            context,
            analyzer,
            database
        })
    }

    pub fn eval(&self) -> Result<(), Box<dyn Error>> {
        let mut previous = self.context.edbs
            .keys()
            .cloned()
            .collect::<Vec<String>>();
        let queue = self.context.ordered_idbs();
        queue.iter().for_each(|name| {
            let rules = self.context.idbs.get(name)
                .expect("IDB should be present in context");
            assert!(
                rules.iter()
                .all(|rule| rule.head.terms.len() == rules[0].head.terms.len())
            );
            self.apply_rules(rules, &previous);
            previous.push(name.to_string());
        });
        self.write_queries()?;
        // write whole database to disk
        let mut database_disk = Connection::open(self.source_db.clone())?;
        {
            let backup = Backup::new(&self.database, &mut database_disk)?;
            backup.run_to_completion(5, Duration::from_millis(1), None)?;
        }
        Ok(())
    }

    pub fn write_queries(&self) -> Result<(), Box<dyn Error>> {
        let queries = &self.context.queries;
        queries.iter().for_each(|(query, rules)| {
            for rule in rules {
                let sql = format!("SELECT name FROM sqlite_master WHERE type='table' AND name='{}';", query);
                let mut stmt = self.database.prepare(&sql).unwrap();
                let mut rows = stmt.query(params![]).unwrap();
                let rows_exist = rows.next().unwrap();
                if rows_exist.is_none() {
                    panic!("Query {} is not present in database", query);
                }
                // let rule = self.context.queries.get(query)
                //     .expect("Query should be present in context");
                let mut sql = format!("SELECT * FROM {}", query);
                let mut where_sql = Vec::new();
                let var_dict = VarDict::new(rule);
                // push constant terms to where clause
                rule.head.terms.iter().enumerate().for_each(|(term_index, term)| {
                    if let Term::Constant(constant) = term {
                        let column = format!("column_{}", term_index);
                        where_sql.push(format!("{} = {}", column, constant));
                    }
                });
                // push inner where_sql stmt
                var_dict.head_dict.iter().for_each(|(_, indexes)| {
                    indexes.iter().skip(1).for_each(|index| {
                        let column = format!("column_{}", index);
                        where_sql.push(format!("column_0 = {}", column));
                    });
                });
                if !where_sql.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(where_sql.join(" AND ").as_str());
                }
                sql.push_str(";");
                if self.verbose {
                    println!("{}: {}", "EXECUTE".green(), sql);
                }
                let mut stmt = self.database.prepare(sql.as_str()).unwrap();
                let rows = stmt.query_map([], |row| {
                    let mut values = Vec::new();
                    for i in 0..rule.head.terms.len() {
                        let value = row.get::<_, String>(i).unwrap();
                        values.push(value);
                    }
                    Ok(values)
                }).unwrap();
                let entities = rows.collect::<Result<Vec<Vec<String>>, _>>().unwrap();
                // if length of entities is less than 20, print all
                // else print the first 10 and last 10
                println!("{}: {}", "QUERY".green(), rule.head);
                if entities.len() <= 20 {
                    entities.iter().for_each(|entity| {
                        println!("{}", entity.join(", "));
                    });
                } else {
                    entities.iter().take(10).for_each(|entity| {
                        println!("{}", entity.join(", "));
                    });
                    println!("...");
                    entities.iter().rev().take(10).for_each(|entity| {
                        println!("{}", entity.join(", "));
                    });
                }
                println!("{}: {}", "COUNT".green(), entities.len());
            }
        });
        Ok(())
    }

    fn apply_rules(&self, rules: &Vec<Rule>, previous: &Vec<String>) {
        let base_cases = rules.iter()
            .filter(|rule| rule.is_base_case(previous))
            .collect::<Vec<&Rule>>();
        base_cases.iter().for_each(|&rule| {
            // create database tables for head if not present
            let head_table = &rule.head.predicate;
            let arity = rule.head.terms.len();
            let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (", head_table);
            let type_info = self.analyzer.data_types.get(head_table)
                .expect("Head table should be present in type info");
            for i in 0..arity {
                // get type from analyzer
                let data_type = type_info.get(i)
                    .expect("Type info should be present");
                let type_ = match data_type {
                    DataType::Integer => "INTEGER",
                    DataType::Symbol => "TEXT",
                    DataType::Float => "REAL",
                };
                sql.push_str(format!("column_{} {}", i, type_).as_str());
                if i < arity - 1 {
                    sql.push_str(", ");
                }
            }
            // unique constraint on all columns
            sql.push_str(", UNIQUE(");
            for i in 0..arity {
                sql.push_str(format!("column_{}", i).as_str());
                if i < arity - 1 {
                    sql.push_str(", ");
                }
            }
            sql.push_str("));");
            if self.verbose {
                println!("{}: {}", "EXECUTE".green(), sql);
            }
            self.database.execute(&sql, params![]).unwrap();
            // retrieve tuples from edb according to rule
            self.init_base(rule);
        });

        let recursive_cases = rules.iter()
            .filter(|rule| !rule.is_base_case(previous))
            .collect::<Vec<&Rule>>();
        recursive_cases.iter().for_each(|&rule| {
            self.semi_naive_evaluate(rule);
        });
    }

    fn init_base(&self, rule: &Rule) {
        let indent = " ".repeat(9);
        let mut sql = format!("INSERT OR IGNORE INTO {}\n", rule.head.to_string());
        let mut select_sql = Vec::new();
        let mut join_sql = HashMap::new();
        let mut where_sql = Vec::new();
        let mut first_predicate = String::new();
        let var_dict = VarDict::new(&rule);
        let mut distinguished_variables: Vec<HashSet::<(usize, usize)>> = Vec::new();
        distinguished_variables.resize(rule.head.terms.len(), HashSet::new());
        rule.head.terms.iter().enumerate().for_each(|(i, term)| {
            if let Some(var) = term.is_nontrivial_variable() {
                distinguished_variables[i] = var_dict.alloc(&var);
            }
        });
        // push select_sql stmts
        distinguished_variables.iter().enumerate().for_each(|(index, set)| {
            if set.is_empty() {
                panic!("Variable {} is not assigned", rule.head.terms[index]);
            }
            let (atom_index, term_index) = set.iter()
                .min_by_key(|(_, term_index)| term_index).unwrap();
            let atom_name = &rule.body[*atom_index].to_string();
            first_predicate = atom_name.clone();
            let stmt = String::from(format!("{}.column_{} AS column_{}", atom_name, term_index, index));
            select_sql.push(stmt);
        });
        // push inner where_sql stmts
        var_dict.clause_dict.iter().for_each(|(_, var_groups)| {
            var_groups.iter().for_each(|group| {
                if group.contain_duplicate() {
                    let atom_predicate = rule.body[group.clause_index].to_string();
                    let positions = &group.term_indexes;
                    positions.iter().skip(1).for_each(|position| {
                        let stmt = format!("{}.column_{} = {}.column_{}",
                            atom_predicate,
                            positions[0],
                            atom_predicate,
                            position);
                        where_sql.push(stmt);
                    });
                }
            });
        });
        // push constant where_sql stmts
        rule.body.iter().for_each(|clause| {
            if let Clause::Atom(atom) = clause {
                atom.terms.iter().enumerate().for_each(|(term_index, term)| {
                    if let Term::Constant(constant) = term {
                        let stmt = format!("{}.column_{} = {}",
                            atom.predicate,
                            term_index,
                            constant);
                        where_sql.push(stmt);
                    }
                });
            }
        });
        // push join_sql stmts
        var_dict.clause_dict.iter().for_each(|(_, var_groups)| {
            if var_groups.len() > 1 {
                let anchor = var_groups[0].clause_index;
                let anchor = &rule.body[anchor].to_string();
                let anchor_term_index = var_groups[0].term_indexes[0];
                var_groups.iter().skip(1).for_each(|group| {
                    let atom_predicate = rule.body[group.clause_index].to_string();
                    let positions = &group.term_indexes;
                    let stmt = format!("{}.column_{} = {}.column_{}",
                        anchor,
                        anchor_term_index,
                        atom_predicate,
                        positions[0]);
                    if *anchor != first_predicate {
                        join_sql.entry(anchor.clone()).or_insert(Vec::new()).push(stmt.clone());
                    }
                    if atom_predicate != first_predicate {
                        join_sql.entry(atom_predicate).or_insert(Vec::new()).push(stmt.clone());
                    }
                });
            }
        });
        let mut select_sql = select_sql.join(", ");
        select_sql = format!("{}SELECT {}\n{}FROM {}\n", indent, select_sql, indent, first_predicate);
        sql.push_str(&select_sql);
        if !join_sql.is_empty() {
            join_sql.iter().for_each(|(predicate, stmts)| {
                let mut stmts = stmts.join(" AND ");
                stmts = format!("{}JOIN {} ON {}\n", indent, predicate, stmts);
                sql.push_str(&stmts);
            });
        }
        if !where_sql.is_empty() {
            let mut where_sql = where_sql.join(" AND ");
            where_sql = format!("{}WHERE {}\n", indent, where_sql);
            sql.push_str(&where_sql);
        }
        if self.verbose {
            println!("{}: {}", "EXECUTE".green(), sql);
        }
        self.database.execute(&sql, params![]).unwrap();
    }

    fn semi_naive_evaluate(&self, rule: &Rule) {
        // copy rule to delta table
        let delta_table = format!("delta_{}", rule.head.predicate);
        let init_delta = format!("CREATE TABLE {} AS SELECT * FROM {}",
            delta_table,
            rule.head.predicate
        );
        if self.verbose {
            println!("{}: {}", "EXECUTE".green(), init_delta);
        }
        self.database.execute(&init_delta, params![]).unwrap();
        let temp_table = format!("temp_{}", rule.head.predicate);
        // create empty temp table
        let create_sql = format!("CREATE TABLE {} AS SELECT * FROM {} WHERE 1 = 0",
            temp_table,
            rule.head.predicate
        );
        if self.verbose {
            println!("{}: {}", "EXECUTE".green(), create_sql);
        }
        self.database.execute(&create_sql, params![]).unwrap();
        // evaluate rule util reaching fixpoint
        let mut fixpoint = false;
        let mut iterate_counter = 0;
        while !fixpoint {
            if self.verbose {
                println!("{}: {}({})", "ITERATE".yellow(), rule.head.predicate, iterate_counter.to_string().yellow());
            }
            self.iteration(rule);
            let count_sql = format!("SELECT COUNT(*) FROM {}", delta_table);
            let count: i64 = self.database.query_row(
                &count_sql,
                params![],
                |row| row.get(0)
            ).unwrap();
            fixpoint = count == 0;
            if !fixpoint {
                iterate_counter += 1;
            } else {
                if self.verbose {
                    println!("{}: {}({})", "FIXPOINT".yellow(), rule.head.predicate, iterate_counter.to_string().green());
                }
            }
        }
        // drop delta table and temp table
        let drop_delta = format!("DROP TABLE {};", delta_table);
        if self.verbose {
            println!("{}: {}", "EXECUTE".green(), drop_delta);
        }
        self.database.execute(&drop_delta, params![]).unwrap();
        let drop_temp = format!("DROP TABLE {};", temp_table);
        if self.verbose {
            println!("{}: {}", "EXECUTE".green(), drop_temp);
        }
        self.database.execute(&drop_temp, params![]).unwrap();
    }

    fn iteration(&self, rule: &Rule) {
        let indent = " ".repeat(9);
        let mut sql = format!("INSERT OR IGNORE INTO temp_{}\n", rule.head.to_string());
        let mut select_sql = Vec::new();
        let mut join_sql: HashMap<String, Vec<String>> = HashMap::new();
        let mut where_sql: Vec<String> = Vec::new();
        let mut first_predicate = String::new();
        let var_dict = VarDict::new(rule);
        let mut distinguished_variables: Vec<HashSet::<(usize, usize)>> = Vec::new();
        distinguished_variables.resize(rule.head.terms.len(), HashSet::new());
        rule.head.terms.iter().enumerate().for_each(|(i, term)| {
            if let Some(var) = term.is_nontrivial_variable() {
                distinguished_variables[i] = var_dict.alloc(&var);
            }
        });
        // push select_sql stmts
        distinguished_variables.iter().enumerate().for_each(|(index, set)| {
            if set.is_empty() {
                panic!("Variable {} is not assigned", rule.head.terms[index]);
            }
            let (atom_index, term_index) = set.iter()
                .min_by_key(|(_, term_index)| term_index).unwrap();
            let mut atom_name = rule.body[*atom_index].to_string();
            if atom_name == rule.head.predicate {
                atom_name = format!("delta_{}", atom_name);
            }
            first_predicate = atom_name.clone();
            let stmt = String::from(format!("{}.column_{} AS column_{}", atom_name, term_index, index));
            select_sql.push(stmt);
        });
        // push inner where_sql stmts
        var_dict.clause_dict.iter().for_each(|(_, var_groups)| {
            var_groups.iter().for_each(|group| {
                if group.contain_duplicate() {
                    let mut atom_predicate = rule.body[group.clause_index].to_string();
                    if atom_predicate == rule.head.predicate {
                        atom_predicate = format!("delta_{}", atom_predicate);
                    }
                    let positions = &group.term_indexes;
                    positions.iter().skip(1).for_each(|position| {
                        let stmt = format!("{}.column_{} = {}.column_{}",
                            atom_predicate,
                            positions[0],
                            atom_predicate,
                            position);
                        where_sql.push(stmt);
                    });
                }
            });
        });
        // push constant where_sql stmts
        rule.body.iter().for_each(|clause| {
            if let Clause::Atom(atom) = clause {
                atom.terms.iter().enumerate().for_each(|(term_index, term)| {
                    if let Term::Constant(constant) = term {
                        let mut atom_predicate = atom.predicate.clone();
                        if atom_predicate == rule.head.predicate {
                            atom_predicate = format!("delta_{}", atom_predicate);
                        }
                        let stmt = format!("{}.column_{} = {}",
                            atom_predicate,
                            term_index,
                            constant);
                        where_sql.push(stmt);
                    }
                });
            }
        });
        // push join_sql stmts
        var_dict.clause_dict.iter().for_each(|(_, var_groups)| {
            if var_groups.len() > 1 {
                let anchor = var_groups[0].clause_index;
                let mut anchor = rule.body[anchor].to_string();
                if anchor == rule.head.predicate {
                    anchor = format!("delta_{}", anchor);
                }
                let anchor_term_index = var_groups[0].term_indexes[0];
                var_groups.iter().skip(1).for_each(|group| {
                    let mut atom_predicate = rule.body[group.clause_index].to_string();
                    if atom_predicate == rule.head.predicate {
                        atom_predicate = format!("delta_{}", atom_predicate);
                    }
                    let positions = &group.term_indexes;
                    let stmt = format!("{}.column_{} = {}.column_{}",
                        anchor,
                        anchor_term_index,
                        atom_predicate,
                        positions[0]);
                    if *anchor != first_predicate {
                        join_sql.entry(anchor.clone()).or_insert(Vec::new()).push(stmt.clone());
                    }
                    if atom_predicate != first_predicate {
                        join_sql.entry(atom_predicate).or_insert(Vec::new()).push(stmt.clone());
                    }
                });
            }
        });
        let mut select_sql = select_sql.join(", ");
        select_sql = format!("{}SELECT {}\n{}FROM {}", indent, select_sql, indent, first_predicate);
        sql.push_str(&select_sql);
        if !join_sql.is_empty() {
            join_sql.iter().for_each(|(predicate, stmts)| {
                let mut stmts = stmts.join(" AND ");
                stmts = format!("\n{}JOIN {} ON {}", indent, predicate, stmts);
                sql.push_str(&stmts);
            });
        }
        if !where_sql.is_empty() {
            let mut where_sql = where_sql.join(" AND ");
            where_sql = format!("\n{}WHERE {}", indent, where_sql);
            sql.push_str(&where_sql);
        }
        if self.verbose {
            println!("{}: {}", "EXECUTE".green(), sql);
        }
        self.database.execute(&sql, params![]).unwrap();

        // update delta := temp - original
        let clear_delta = format!("DELETE FROM delta_{}", rule.head.predicate);
        if self.verbose {
            println!("{}: {}", "EXECUTE".green(), clear_delta);
        }
        self.database.execute(&clear_delta, params![]).unwrap();
        // use left join
        let mut update_sql = format!("INSERT OR IGNORE INTO delta_{}\n{}SELECT temp_{}.* FROM temp_{}\n{}",
            rule.head.predicate,
            indent,
            rule.head.predicate,
            rule.head.predicate,
            indent,
        );
        let wheres: Vec<String> = (0..distinguished_variables.len()).map(|i| format!("column_{}", i)).collect();
        // LEFT JOIN original ON temp.column_0 = original.column_0 AND ...
        // WHERE original.column_0 IS NULL AND ...
        update_sql.push_str(&format!("LEFT JOIN {} ON {}\n",
            rule.head.predicate,
            wheres.iter().enumerate().map(|(_, where_)| {
                format!("temp_{}.{} = {}.{}", rule.head.predicate, where_, rule.head.predicate, where_)
            }).collect::<Vec<String>>().join(" AND "),
        ));
        update_sql.push_str(&format!("{}WHERE {}",
            indent,
            wheres.iter().enumerate().map(|(_, where_)| {
                format!("{}.{} IS NULL", rule.head.predicate, where_)
            }).collect::<Vec<String>>().join(" AND "),
        ));
        if self.verbose {
            println!("{}: {}", "EXECUTE".green(), update_sql);
        }
        self.database.execute(&update_sql, params![]).unwrap();

        // update original := original + delta
        let update_sql = format!("INSERT OR IGNORE INTO {}\n{}SELECT * FROM delta_{};",
            rule.head.predicate,
            indent,
            rule.head.predicate,
        );
        if self.verbose {
            println!("{}: {}", "EXECUTE".green(), update_sql);
        }
        self.database.execute(&update_sql, params![]).unwrap();
    }
}

