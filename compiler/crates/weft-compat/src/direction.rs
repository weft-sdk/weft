//! Who sends and who receives each type.
//!
//! A type in a request (an HTTP path, query, body or header, an rpc request, a GraphQL argument, a socket `client`
//! event) is *input*: callers send it. A type in a response, an error, an event or a socket `server` event is
//! *output*: receivers read it. The same change is safe in one direction and breaking in the other, so every
//! judgement starts here. A type used both ways gets both. A type no operation of the package uses may be imported
//! by another package, so it gets both too.

use std::collections::{HashMap, HashSet};

use weft_ir::{Ir, ResultDef, Side, TypeBody, TypeRef};

pub const INPUT: u8 = 1;
pub const OUTPUT: u8 = 2;
pub const BOTH: u8 = INPUT | OUTPUT;

pub fn is_input(d: u8) -> bool {
    d & INPUT != 0
}

pub fn is_output(d: u8) -> bool {
    d & OUTPUT != 0
}

/// The direction of every type of the package, by name.
pub fn directions(ir: &Ir) -> HashMap<String, u8> {
    let mut w = Walk {
        ir,
        out: HashMap::new(),
        seen: HashSet::new(),
    };
    for r in &ir.routes {
        for p in &r.path_params {
            w.visit(&p.ty, INPUT);
        }
        for t in [&r.query, &r.body, &r.headers].into_iter().flatten() {
            w.visit(t, INPUT);
        }
        w.result(&r.result);
    }
    for s in &ir.rpc_services {
        for m in &s.methods {
            w.visit(&m.request, INPUT);
            w.result(&m.result);
        }
    }
    for f in &ir.graphql {
        for a in &f.args {
            w.visit(&a.ty, INPUT);
        }
        w.result(&f.result);
    }
    for s in &ir.sockets {
        for e in &s.events {
            let d = match e.side {
                Side::Client => INPUT,
                Side::Server => OUTPUT,
            };
            w.visit(&e.payload, d);
        }
    }
    for e in &ir.events {
        w.visit(&e.payload, OUTPUT);
        if let Some(h) = &e.headers {
            w.visit(h, OUTPUT);
        }
    }
    for e in &ir.errors {
        for f in &e.fields {
            w.visit(&f.ty, OUTPUT);
        }
    }
    // Types nothing in this package uses: another package may.
    for t in &ir.types {
        w.out.entry(t.name.clone()).or_insert(BOTH);
    }
    w.out
}

struct Walk<'a> {
    ir: &'a Ir,
    out: HashMap<String, u8>,
    seen: HashSet<(String, u8)>,
}

impl Walk<'_> {
    fn result(&mut self, r: &ResultDef) {
        self.visit(&r.value, OUTPUT);
    }

    fn visit(&mut self, ty: &TypeRef, dir: u8) {
        match ty {
            TypeRef::Primitive { .. } | TypeRef::Param { .. } => {}
            TypeRef::List { item } => self.visit(item, dir),
            TypeRef::Map { key, value } => {
                self.visit(key, dir);
                self.visit(value, dir);
            }
            TypeRef::Named {
                package,
                name,
                args,
            } => {
                for a in args {
                    self.visit(a, dir);
                }
                if *package != self.ir.package {
                    return;
                }
                *self.out.entry(name.clone()).or_insert(0) |= dir;
                if !self.seen.insert((name.clone(), dir)) {
                    return;
                }
                let Some(def) = self.ir.types.iter().find(|t| t.name == *name) else {
                    return;
                };
                if let TypeBody::Record { fields } = &def.body {
                    for f in fields {
                        self.visit(&f.ty, dir);
                    }
                }
            }
        }
    }
}
