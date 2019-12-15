// Inspired by this blog post: http://thume.ca/2019/04/18/writing-a-compiler-in-rust/

use crate::lang::ast::*;
use crate::utils::Scoped;

/// `Changes` collects abstract syntax changes and additions during a visitor pass
/// The way the changes are defined is specified by each function.
#[derive(Debug)]
pub struct Changes {
    new_comps: Vec<Component>,
    new_struct: Vec<Structure>,
    new_node: Scoped<Option<Control>>,
    new_input_ports: Vec<Portdef>,
    new_output_ports: Vec<Portdef>,
}

impl Changes {
    /// adds a new component to the current namespace
    /// You can call this anywhere during a pass
    pub fn add_component(&mut self, comp: Component) {
        self.new_comps.push(comp);
    }

    /// Adds new structure statements to the current component
    pub fn add_structure(&mut self, structure: Structure) {
        self.new_struct.push(structure);
    }

    /// Changes the control node that is being visited when this is called to `control`.
    /// This provides a way to change the actual nodes in the ast.
    /// This change is applied *after* the `finish_*` function is called for the current
    /// control node.
    pub fn change_node(&mut self, control: Control) {
        self.new_node.set(Some(control));
    }

    /// asdf
    pub fn add_input_port(&mut self, port: Portdef) {
        self.new_input_ports.push(port);
    }

    /// asdf
    pub fn add_output_port(&mut self, port: Portdef) {
        self.new_output_ports.push(port);
    }

    /// internal function that creates a new scope for Changes
    fn push_scope(&mut self) {
        self.new_node.push_scope();
    }

    /// internal function that goes out a scope for Changes
    fn pop_scope(&mut self) {
        self.new_node.pop_scope();
    }

    fn clear(&mut self) {
        self.new_struct = vec![];
        self.new_input_ports = vec![];
        self.new_output_ports = vec![];
    }

    fn new() -> Self {
        Changes {
            new_comps: vec![],
            new_struct: vec![],
            new_node: Scoped::new(),
            new_input_ports: vec![],
            new_output_ports: vec![],
        }
    }
}

/** The `Visitor` trait parameterized on an `Error` type.
For each node `x` in the Ast, there are the functions `start_x`
and `finish_x`. The start functions are called at the beginning
of the traversal for each node, and the finish functions are called
at the end of the traversal for each node. You can use the finish
functions to wrap error with more information. */
pub trait Visitor<Err: std::fmt::Debug> {
    fn name(&self) -> String;

    fn do_pass(&mut self, syntax: &mut Namespace) -> &mut Self
    where
        Self: Sized,
    {
        let mut changes = Changes::new();
        for comp in &mut syntax.components {
            let res = self.start(&mut changes);
            comp.control.visit(self, &mut changes).unwrap_or_else(|x| {
                eprintln!("The {} pass failed: {:?}", self.name(), x)
            });
            self.finish(&mut changes, res);

            // update changes
            comp.structure.append(&mut changes.new_struct);
            comp.inputs.append(&mut changes.new_input_ports);
            comp.outputs.append(&mut changes.new_output_ports);
            changes.clear();
        }
        syntax.components.append(&mut changes.new_comps);
        self
    }

    fn start(&mut self, _c: &mut Changes) -> Result<(), Err> {
        Ok(())
    }

    fn finish(&mut self, _c: &mut Changes, _res: Result<(), Err>) {}

    fn start_seq(&mut self, _s: &mut Seq, _c: &mut Changes) -> Result<(), Err> {
        Ok(())
    }

    fn finish_seq(
        &mut self,
        _s: &mut Seq,
        _c: &mut Changes,
        res: Result<(), Err>,
    ) -> Result<(), Err> {
        res
    }

    fn start_par(&mut self, _s: &mut Par, _c: &mut Changes) -> Result<(), Err> {
        Ok(())
    }

    fn finish_par(
        &mut self,
        _s: &mut Par,
        _x: &mut Changes,
        res: Result<(), Err>,
    ) -> Result<(), Err> {
        res
    }

    fn start_if(&mut self, _s: &mut If, _c: &mut Changes) -> Result<(), Err> {
        Ok(())
    }

    fn finish_if(
        &mut self,
        _s: &mut If,
        _x: &mut Changes,
        res: Result<(), Err>,
    ) -> Result<(), Err> {
        res
    }

    fn start_ifen(
        &mut self,
        _s: &mut Ifen,
        _c: &mut Changes,
    ) -> Result<(), Err> {
        Ok(())
    }

    fn finish_ifen(
        &mut self,
        _s: &mut Ifen,
        _x: &mut Changes,
        res: Result<(), Err>,
    ) -> Result<(), Err> {
        res
    }

    fn start_while(
        &mut self,
        _s: &mut While,
        _c: &mut Changes,
    ) -> Result<(), Err> {
        Ok(())
    }

    fn finish_while(
        &mut self,
        _s: &mut While,
        _x: &mut Changes,
        res: Result<(), Err>,
    ) -> Result<(), Err> {
        res
    }

    fn start_print(
        &mut self,
        _s: &mut Print,
        _x: &mut Changes,
    ) -> Result<(), Err> {
        Ok(())
    }

    fn finish_print(
        &mut self,
        _s: &mut Print,
        _x: &mut Changes,
        res: Result<(), Err>,
    ) -> Result<(), Err> {
        res
    }

    fn start_enable(
        &mut self,
        _s: &mut Enable,
        _x: &mut Changes,
    ) -> Result<(), Err> {
        Ok(())
    }

    fn finish_enable(
        &mut self,
        _s: &mut Enable,
        _x: &mut Changes,
        res: Result<(), Err>,
    ) -> Result<(), Err> {
        res
    }

    fn start_disable(
        &mut self,
        _s: &mut Disable,
        _x: &mut Changes,
    ) -> Result<(), Err> {
        Ok(())
    }

    fn finish_disable(
        &mut self,
        _s: &mut Disable,
        _x: &mut Changes,
        res: Result<(), Err>,
    ) -> Result<(), Err> {
        res
    }

    fn start_empty(
        &mut self,
        _s: &mut Empty,
        _x: &mut Changes,
    ) -> Result<(), Err> {
        Ok(())
    }

    fn finish_empty(
        &mut self,
        _s: &mut Empty,
        _x: &mut Changes,
        res: Result<(), Err>,
    ) -> Result<(), Err> {
        res
    }
}

/** `Visitable` describes types that can be visited by things
implementing `Visitor`. This performs a recursive walk of the tree.
It calls `Visitor::start_*` on the way down, and `Visitor::finish_*`
on the way up. */
pub trait Visitable {
    fn visit<Err: std::fmt::Debug>(
        &mut self,
        visitor: &mut dyn Visitor<Err>,
        changes: &mut Changes,
    ) -> Result<(), Err>;
}

// Blanket impl for Vectors of Visitables
impl<V: Visitable> Visitable for Vec<V> {
    fn visit<Err: std::fmt::Debug>(
        &mut self,
        visitor: &mut dyn Visitor<Err>,
        changes: &mut Changes,
    ) -> Result<(), Err> {
        for t in self {
            t.visit(visitor, changes)?;
        }
        Ok(())
    }
}

impl Visitable for Control {
    fn visit<Err: std::fmt::Debug>(
        &mut self,
        visitor: &mut dyn Visitor<Err>,
        changes: &mut Changes,
    ) -> Result<(), Err> {
        changes.push_scope();
        let res = match self {
            Control::Seq { data } => {
                visitor.start_seq(data, changes)?;
                let res = data.stmts.visit(visitor, changes);
                let res2 = visitor.finish_seq(data, changes, res);
                match &changes.new_node.get() {
                    Some(c) => *self = c.clone(),
                    None => (),
                }
                res2
            }
            Control::Par { data } => {
                visitor.start_par(data, changes)?;
                let res = data.stmts.visit(visitor, changes);
                let res2 = visitor.finish_par(data, changes, res);
                match &changes.new_node.get() {
                    Some(c) => *self = c.clone(),

                    None => (),
                }
                res2
            }
            Control::If { data } => {
                visitor.start_if(data, changes)?;
                // closure to combine the results
                let res = (|| {
                    data.tbranch.visit(visitor, changes)?;
                    data.fbranch.visit(visitor, changes)
                })();
                let res2 = visitor.finish_if(data, changes, res);
                match &changes.new_node.get() {
                    Some(c) => {
                        *self = c.clone();
                    }
                    None => (),
                }
                res2
            }
            Control::Ifen { data } => {
                visitor.start_ifen(data, changes)?;
                let res = (|| {
                    data.tbranch.visit(visitor, changes)?;
                    data.fbranch.visit(visitor, changes)
                })();
                let res2 = visitor.finish_ifen(data, changes, res);
                match &changes.new_node.get() {
                    Some(c) => {
                        *self = c.clone();
                    }
                    None => (),
                }
                res2
            }
            Control::While { data } => {
                visitor.start_while(data, changes)?;
                let res = data.body.visit(visitor, changes);
                let res2 = visitor.finish_while(data, changes, res);
                match &changes.new_node.get() {
                    Some(c) => {
                        *self = c.clone();
                    }
                    None => (),
                }
                res2
            }
            Control::Print { data } => {
                let res = visitor.start_print(data, changes);
                let res2 = visitor.finish_print(data, changes, res);
                match &changes.new_node.get() {
                    Some(c) => {
                        *self = c.clone();
                    }
                    None => (),
                }
                res2
            }
            Control::Enable { data } => {
                let res = visitor.start_enable(data, changes);
                let res2 = visitor.finish_enable(data, changes, res);
                match &changes.new_node.get() {
                    Some(c) => {
                        *self = c.clone();
                    }
                    None => (),
                }
                res2
            }
            Control::Disable { data } => {
                let res = visitor.start_disable(data, changes);
                let res2 = visitor.finish_disable(data, changes, res);
                match &changes.new_node.get() {
                    Some(c) => {
                        *self = c.clone();
                    }
                    None => (),
                }
                res2
            }
            Control::Empty { data } => {
                let res = visitor.start_empty(data, changes);
                let res2 = visitor.finish_empty(data, changes, res);
                match &changes.new_node.get() {
                    Some(c) => {
                        *self = c.clone();
                    }
                    None => (),
                }
                res2
            }
        };
        changes.pop_scope();
        res
    }
}