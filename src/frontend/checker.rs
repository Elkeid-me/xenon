use super::{
    ast::*,
    expr::types::Type::{self, *},
};
use std::{cmp::max, collections::HashMap, iter::zip};
pub enum SymbolTableItem {
    ConstVariable(i32),
    Variable,
    ConstArray(Vec<usize>, Vec<i32>),
    Array(Vec<usize>),
    Function(Type, Vec<Type>),
    Pointer(Vec<usize>),
}

use SymbolTableItem::{Array, ConstArray, ConstVariable, Function, Variable};

pub type SymbolTable<'a> = Vec<HashMap<&'a str, SymbolTableItem>>;

pub trait Scope<'a> {
    fn new() -> Self;
    fn search(&self, identifier: &str) -> Option<&SymbolTableItem>;

    fn insert_definition(&mut self, identifier: &'a str, symbol: SymbolTableItem) -> Result<(), String>;

    fn enter_scope(&mut self);
    fn exit_scope(&mut self);
}

impl<'a> Scope<'a> for SymbolTable<'a> {
    fn new() -> Self {
        vec![HashMap::new()]
    }

    fn search(&self, identifier: &str) -> Option<&SymbolTableItem> {
        for map in self.iter().rev() {
            if let Some(info) = map.get(identifier) {
                return Some(info);
            }
        }
        return None;
    }

    fn insert_definition(&mut self, identifier: &'a str, symbol: SymbolTableItem) -> Result<(), String> {
        match self.last_mut().unwrap().insert(identifier, symbol) {
            Some(_) => Err(format!("标识符 {} 在当前作用域中已存在", identifier)),
            None => Ok(()),
        }
    }

    fn enter_scope(&mut self) {
        self.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.pop();
    }
}

pub struct Checker<'a> {
    pub table: SymbolTable<'a>,
}

impl<'a> Checker<'a> {
    pub fn new() -> Self {
        Self {
            table: vec![HashMap::from([
                ("getint", Function(Int, Vec::new())),
                ("getch", Function(Int, Vec::new())),
                ("getarray", Function(Int, vec![Pointer(Vec::new())])),
                ("putint", Function(Void, vec![Int])),
                ("putch", Function(Void, vec![Int])),
                ("putarray", Function(Int, vec![Int, Pointer(Vec::new())])),
                ("starttime", Function(Void, Vec::new())),
                ("stoptime", Function(Void, Vec::new())),
            ])],
        }
    }

    fn process_init_list<const IS_CONST_EVAL: bool>(&mut self, init_list: &mut InitializerList) -> Result<Vec<usize>, String> {
        todo!()
    }

    fn process_definition(&mut self, definition: &'a mut Definition) -> Result<(), String> {
        match definition {
            Definition::ConstVariableDefinition(identifier, init) => self
                .table
                .insert_definition(identifier, ConstVariable(init.const_eval(&self.table)?)),
            Definition::ConstArrayDefinition {
                identifier,
                lengths,
                init_list,
            } => todo!(),
            Definition::VariableDefinition(identifier, init) => {
                if let Some(expr) = init {
                    if !matches!(expr.expr_type(&self.table)?, Int) {
                        return Err(format!("{:?} 不是整型表达式", expr));
                    }
                }
                self.table.insert_definition(identifier, Variable)
            }
            Definition::ArrayDefinition {
                identifier,
                lengths,
                init_list,
            } => todo!(),
        }
    }

    fn process_block(&mut self, block: &'a mut Block, return_void: bool, in_while: bool) -> Result<(), String> {
        self.table.enter_scope();
        for block_item in block.iter_mut() {
            match block_item {
                BlockItem::Definition(definition) => self.process_definition(definition)?,
                BlockItem::Block(block) => self.process_block(block, return_void, in_while)?,
                BlockItem::Statement(statement) => match statement.as_mut() {
                    Statement::Expr(expr) => expr.check_expr(&self.table)?,
                    Statement::If {
                        condition,
                        then_block,
                        else_block,
                    } => match condition.expr_type(&self.table)? {
                        Void => return Err(format!("{:?} 不能作为 if 的条件", condition)),
                        _ => {
                            self.process_block(then_block, return_void, in_while)?;
                            self.process_block(else_block, return_void, in_while)?;
                        }
                    },
                    Statement::While { condition, block } => match condition.expr_type(&self.table)? {
                        Void => return Err(format!("{:?} 不能作为 if 的条件", condition)),
                        _ => self.process_block(block, return_void, in_while)?,
                    },
                    Statement::Return(expr) => match (expr, return_void) {
                        (None, true) => (),
                        (None, false) => return Err("int 函数中的 return 语句未返回表达式".to_string()),
                        (Some(expr), true) => return Err(format!("在 void 函数中返回了表达式 {:?}", expr)),
                        (Some(expr), false) => {
                            if !matches!(expr.expr_type(&self.table)?, Int) {
                                return Err(format!("return 语句返回的 {:?} 类型与函数定义不匹配", expr));
                            }
                        }
                    },
                    Statement::Break | Statement::Continue => {
                        if !in_while {
                            return Err("在 while 语句外使用了 break 或 continue".to_string());
                        }
                    }
                },
            }
        }
        self.table.exit_scope();
        Ok(())
    }

    pub fn check(&mut self, ast: &'a mut TranslationUnit) -> Result<(), String> {
        for i in ast.iter_mut() {
            match i.as_mut() {
                GlobalItem::Definition(definition) => self.process_definition(definition)?,
                GlobalItem::FunctionDefinition {
                    return_void,
                    identifier,
                    parameter_list,
                    block,
                } => {
                    for p in parameter_list.iter_mut() {
                        if let Parameter::Pointer(_, exprs) = p {
                            for expr in exprs.iter_mut() {
                                expr.const_eval(&self.table)?;
                            }
                        }
                    }
                    let parameter_type = parameter_list
                        .iter()
                        .map(|p| match p {
                            Parameter::Int(_) => Int,
                            Parameter::Pointer(_, lengths) => Type::Pointer(
                                lengths
                                    .iter()
                                    .map(|p| if let Expr::Num(i) = p { *i as usize } else { unreachable!() })
                                    .collect(),
                            ),
                        })
                        .collect();
                    let return_type = if *return_void { Void } else { Int };
                    self.table
                        .insert_definition(identifier, Function(return_type, parameter_type))?;
                    self.table.enter_scope();
                    for p in parameter_list.iter() {
                        match p {
                            Parameter::Int(identifier) => self.table.insert_definition(identifier, Variable)?,
                            Parameter::Pointer(identifier, lengths) => self.table.insert_definition(
                                identifier,
                                SymbolTableItem::Pointer(
                                    lengths
                                        .iter()
                                        .map(|p| if let Expr::Num(i) = p { *i as usize } else { unreachable!() })
                                        .collect(),
                                ),
                            )?,
                        }
                    }
                    self.process_block(block, *return_void, false)?;
                    self.table.exit_scope();
                }
            }
        }
        Ok(())
    }
}
