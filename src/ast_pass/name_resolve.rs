use crate::ast::symbol::{ModSymTable, ScopedInsertErr, ScopedSymbolStack};
use crate::ast::{Expr, ExprKind, Function, Item, ItemKind, Module};
use crate::ast_pass::ModulePass;

pub struct AstNameResolver<'s> {
    scopes: ScopedSymbolStack<'s>,
    errs: Vec<ScopedInsertErr>,
}

pub enum AstResolutionErr {
    Redefinition,
    // ReturnBeforeBlockEnd,
}
pub type NameResolutionResult = Result<ModSymTable, Vec<ScopedInsertErr>>;

impl<'s> ModulePass<'s> for AstNameResolver<'s> {
    type Output = NameResolutionResult;
    fn run_pass(mut self, m: &Module<'s>) -> Self::Output {
        // root scope
        self.scopes.push_scope();

        // Phase 1, register all top-level symbols
        self.resolve_top_level_names(&m.body);

        // Phase 2, register the bodies of all top-level
        self.resolve_top_level_contents(&m.body);

        // pop root scope
        self.scopes.pop_scope();

        if !self.errs.is_empty() {
            Err(self.errs)
        } else {
            Ok(self.scopes.finish_resolve())
        }
    }
}

impl<'s> Default for AstNameResolver<'s> {
    fn default() -> Self {
        Self {
            scopes: ScopedSymbolStack::default(),
            errs: Vec::new(),
        }
    }
}

// TODO error collection and reporting
impl<'s> AstNameResolver<'s> {
    fn resolve_top_level_names(&mut self, items: &[Item<'s>]) {
        for item in items {
            let nid = item.nid;
            match &item.kind {
                ItemKind::Func(f) => {
                    if let Err(e) = self.scopes.insert_func(nid, f.proto.name.span) {
                        self.errs.push(e);
                    }
                }
                ItemKind::Extern(proto) => {
                    // add function symbol to global
                    if let Err(e) = self.scopes.insert_func(nid, proto.name.span) {
                        self.errs.push(e);
                    }
                }
            }
        }
    }

    fn resolve_top_level_contents(&mut self, items: &[Item<'s>]) {
        for item in items {
            match &item.kind {
                ItemKind::Func(f) => {
                    self.resolve_func_contents(f);
                }
                ItemKind::Extern(_) => (),
            }
        }
    }

    fn resolve_func_contents(&mut self, func: &Function<'s>) {
        // extra scope required just for the function args
        self.scopes.push_scope();

        for arg in &func.proto.args {
            if let Err(e) = self.scopes.insert_local(arg.nid, arg.name.span) {
                self.errs.push(e);
            }
        }

        self.resolve_expr(&func.body);

        self.scopes.pop_scope();
    }
    fn resolve_expr(&mut self, expr: &Expr<'s>) {
        match &*expr.kind {
            ExprKind::Block(b) => {
                self.scopes.push_scope();
                for sub in b {
                    self.resolve_expr(sub)
                }
                self.scopes.pop_scope();
            }
            ExprKind::Decl(vd) => {
                self.resolve_expr(&vd.value);
                if let Err(e) = self.scopes.insert_local(expr.nid, vd.bound.span) {
                    self.errs.push(e);
                }
            }
            ExprKind::Let { bound, let_body } => {
                // TODO WANT: use before definition should be acceptable in this block
                // assuming no cycles
                self.scopes.push_scope();
                for decl in bound {
                    self.resolve_expr(decl);
                }
                self.resolve_expr(let_body);
                self.scopes.pop_scope();
            }
            ExprKind::Var(td) => match self.scopes.lookup(td.span) {
                Some(sid_ref) => {
                    let sid = *sid_ref;
                    self.scopes.insert_local_reuse(expr.nid, sid);
                }
                None => unimplemented!("Unimplemented use before def"),
            },
            ExprKind::Call { callee, args } => {
                for arg in args {
                    self.resolve_expr(arg)
                }

                self.resolve_expr(callee)
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            ExprKind::If {
                cond,
                if_body,
                else_body,
            } => {
                self.resolve_expr(cond);
                self.resolve_expr(if_body);
                self.resolve_expr(else_body);
            }
            ExprKind::While { cond, while_body } => {
                self.resolve_expr(cond);
                self.resolve_expr(while_body);
            }
            ExprKind::Lit(_) => (),
        }
    }
}
