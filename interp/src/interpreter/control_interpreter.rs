use super::group_interpreter::{finish_interpretation, AssignmentInterpreter};
use super::utils::{get_done_port, get_go_port};
use super::Interpreter;
use crate::debugger::name_tree::ActiveTreeNode;
use crate::errors::InterpreterError;
use crate::interpreter_ir as iir;
use crate::structures::names::{
    ComponentQualifiedInstanceName, GroupQIN, GroupQualifiedInstanceName,
};
use crate::utils::AsRaw;
use crate::{
    environment::{
        CompositeView, InterpreterState, MutCompositeView, MutStateView,
        StateView,
    },
    errors::InterpreterResult,
    interpreter::utils::ConstPort,
    values::Value,
};
use calyx::ir::{self, Assignment, Guard, RRC};
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub continuous_assignments: iir::ContinuousAssignments,
    pub input_ports: Rc<HashSet<*const ir::Port>>,
    pub qin: ComponentQualifiedInstanceName,
}

impl ComponentInfo {
    pub fn new(
        continuous_assignments: iir::ContinuousAssignments,
        input_ports: Rc<HashSet<*const ir::Port>>,
        qin: ComponentQualifiedInstanceName,
    ) -> Self {
        Self {
            continuous_assignments,
            input_ports,
            qin,
        }
    }
}

pub struct EmptyInterpreter {
    pub(super) env: InterpreterState,
}

impl EmptyInterpreter {
    pub fn new(env: InterpreterState) -> Self {
        Self { env }
    }
}

impl Interpreter for EmptyInterpreter {
    fn step(&mut self) -> InterpreterResult<()> {
        Ok(())
    }

    fn run(&mut self) -> InterpreterResult<()> {
        Ok(())
    }

    fn deconstruct(self) -> InterpreterResult<InterpreterState> {
        Ok(self.env)
    }

    fn is_done(&self) -> bool {
        true
    }

    fn get_env(&self) -> StateView<'_> {
        (&self.env).into()
    }

    fn currently_executing_group(&self) -> HashSet<GroupQIN> {
        HashSet::new()
    }

    fn get_env_mut(&mut self) -> MutStateView<'_> {
        (&mut self.env).into()
    }

    fn converge(&mut self) -> InterpreterResult<()> {
        Ok(())
    }

    fn get_active_tree(&self) -> Vec<ActiveTreeNode> {
        vec![]
    }
}

#[derive(Clone)]
pub enum EnableHolder {
    Group(RRC<ir::Group>),
    CombGroup(RRC<ir::CombGroup>),
    Vec(Rc<Vec<ir::Assignment>>),
}

impl EnableHolder {
    fn done_port(&self) -> Option<RRC<ir::Port>> {
        match self {
            EnableHolder::Group(g) => Some(get_done_port(&g.borrow())),
            EnableHolder::CombGroup(_) | EnableHolder::Vec(_) => None,
        }
    }

    fn go_port(&self) -> Option<RRC<ir::Port>> {
        match self {
            EnableHolder::Group(g) => Some(get_go_port(&g.borrow())),
            EnableHolder::CombGroup(_) | EnableHolder::Vec(_) => None,
        }
    }
}

impl From<RRC<ir::Group>> for EnableHolder {
    fn from(gr: RRC<ir::Group>) -> Self {
        Self::Group(gr)
    }
}

impl From<RRC<ir::CombGroup>> for EnableHolder {
    fn from(cb: RRC<ir::CombGroup>) -> Self {
        Self::CombGroup(cb)
    }
}

impl From<&RRC<ir::Group>> for EnableHolder {
    fn from(gr: &RRC<ir::Group>) -> Self {
        Self::Group(Rc::clone(gr))
    }
}

impl From<&RRC<ir::CombGroup>> for EnableHolder {
    fn from(cb: &RRC<ir::CombGroup>) -> Self {
        Self::CombGroup(Rc::clone(cb))
    }
}

impl From<Vec<ir::Assignment>> for EnableHolder {
    fn from(v: Vec<ir::Assignment>) -> Self {
        Self::Vec(Rc::new(v))
    }
}

impl From<&iir::Enable> for EnableHolder {
    fn from(en: &iir::Enable) -> Self {
        (&en.group).into()
    }
}

pub struct EnableInterpreter {
    enable: EnableHolder,
    group_name: Option<ir::Id>,
    interp: AssignmentInterpreter,
    qin: ComponentQualifiedInstanceName,
}

impl EnableInterpreter {
    pub fn new<E>(
        enable: E,
        group_name: Option<ir::Id>,
        mut env: InterpreterState,
        continuous: iir::ContinuousAssignments,
        qin: &ComponentQualifiedInstanceName,
    ) -> Self
    where
        E: Into<EnableHolder>,
    {
        let enable: EnableHolder = enable.into();

        if let Some(go) = enable.go_port() {
            env.insert(go, Value::bit_high())
        }

        let assigns = enable.clone();
        let done = enable.done_port();
        let interp = AssignmentInterpreter::new(env, done, assigns, continuous);
        Self {
            enable,
            group_name,
            interp,
            qin: qin.clone(),
        }
    }
}

impl EnableInterpreter {
    fn reset(mut self) -> InterpreterResult<InterpreterState> {
        if let Some(go) = self.enable.go_port() {
            self.interp.get_mut_env().insert(go, Value::bit_low())
        }

        self.interp.reset()
    }
    fn get(&self, port: impl AsRaw<ir::Port>) -> &Value {
        self.interp.get(port)
    }
}

impl Interpreter for EnableInterpreter {
    fn step(&mut self) -> InterpreterResult<()> {
        self.interp.step()
    }

    fn run(&mut self) -> InterpreterResult<()> {
        self.interp.run()
    }

    fn deconstruct(self) -> InterpreterResult<InterpreterState> {
        self.reset()
    }

    fn is_done(&self) -> bool {
        self.interp.is_deconstructable()
    }

    fn get_env(&self) -> StateView<'_> {
        (self.interp.get_env()).into()
    }

    fn currently_executing_group(&self) -> HashSet<GroupQIN> {
        let mut set = HashSet::new();
        if let Some(name) = &self.group_name {
            set.insert(GroupQIN::new(&self.qin, name));
        }
        set
    }

    fn get_env_mut(&mut self) -> MutStateView<'_> {
        (self.interp.get_mut_env()).into()
    }

    fn converge(&mut self) -> InterpreterResult<()> {
        self.interp.step_convergence()
    }

    fn get_active_tree(&self) -> Vec<ActiveTreeNode> {
        let name = match &self.group_name {
            Some(name) => {
                GroupQualifiedInstanceName::new_group(&self.qin, name)
            }
            None => GroupQualifiedInstanceName::new_empty(&self.qin),
        };

        vec![ActiveTreeNode::new(name)]
    }
}

enum SeqFsm {
    Err, // Transient error state
    Iterating(ControlInterpreter, usize),
    Done(InterpreterState),
}

impl Default for SeqFsm {
    fn default() -> Self {
        Self::Err
    }
}

pub struct SeqInterpreter {
    internal_state: SeqFsm,
    info: ComponentInfo,
    seq: Rc<iir::Seq>,
}
impl SeqInterpreter {
    pub fn new(
        seq: Rc<iir::Seq>,
        env: InterpreterState,
        info: ComponentInfo,
    ) -> Self {
        let internal_state = if seq.stmts.is_empty() {
            SeqFsm::Done(env)
        } else {
            let first = seq.stmts[0].clone();
            let interp = ControlInterpreter::new(first, env, &info);
            SeqFsm::Iterating(interp, 1)
        };

        Self {
            seq,
            internal_state,
            info,
        }
    }
}

impl Interpreter for SeqInterpreter {
    fn step(&mut self) -> InterpreterResult<()> {
        match &mut self.internal_state {
            SeqFsm::Iterating(interp, _) => {
                // step the interpreter
                if !interp.is_done() {
                    interp.step()?;
                }
                // transition to next block or done
                else if let SeqFsm::Iterating(interp, idx) =
                    std::mem::take(&mut self.internal_state)
                {
                    let env = interp.deconstruct()?;

                    if idx < self.seq.stmts.len() {
                        let next = self.seq.stmts[idx].clone();
                        let mut interp =
                            ControlInterpreter::new(next, env, &self.info);
                        interp.step()?;
                        self.internal_state =
                            SeqFsm::Iterating(interp, idx + 1);
                    } else {
                        self.internal_state = SeqFsm::Done(env);
                    }
                } else {
                    // this is genuinely unreachable
                    unreachable!();
                }
                Ok(())
            }
            SeqFsm::Done(_) => Ok(()),
            SeqFsm::Err => unreachable!("There is an error in the Seq state transition. Please report this."),
        }
    }

    fn is_done(&self) -> bool {
        matches!(&self.internal_state, SeqFsm::Done(_))
    }

    fn deconstruct(self) -> InterpreterResult<InterpreterState> {
        match self.internal_state {
            SeqFsm::Iterating(_, _) => Err(InterpreterError::InvalidSeqState),
            SeqFsm::Done(e) => Ok(e),
            SeqFsm::Err => unreachable!("There is an error in the Seq state transition. Please report this."),
        }
    }

    fn get_env(&self) -> StateView<'_> {
        match &self.internal_state {
            SeqFsm::Iterating(i, _) => i.get_env(),
            SeqFsm::Done(e) => e.into(),
            SeqFsm::Err => unreachable!("There is an error in the Seq state transition. Please report this."),
        }
    }

    fn currently_executing_group(&self) -> HashSet<GroupQIN> {
        match &self.internal_state {
            SeqFsm::Iterating(i, _) => i.currently_executing_group(),
            SeqFsm::Done(_) => HashSet::new(),
            SeqFsm::Err => unreachable!("There is an error in the Seq state transition. Please report this."),
        }
    }

    fn get_env_mut(&mut self) -> MutStateView<'_> {
        match &mut self.internal_state {
            SeqFsm::Iterating(i, _) => i.get_env_mut(),
            SeqFsm::Done(e) => e.into(),
            SeqFsm::Err => unreachable!("There is an error in the Seq state transition. Please report this."),
        }
    }

    fn converge(&mut self) -> InterpreterResult<()> {
        if let SeqFsm::Iterating(i, _) = &mut self.internal_state {
            i.converge()
        } else {
            Ok(())
        }
    }

    fn run(&mut self) -> InterpreterResult<()> {
        match &mut self.internal_state {
            SeqFsm::Err => unreachable!("There is an error in the Seq state transition. Please report this."),
            SeqFsm::Iterating(_, _) => {
                if let SeqFsm::Iterating(i, mut idx) =
                    std::mem::take(&mut self.internal_state)
                {
                    let mut env = i.run_and_deconstruct()?;
                    while idx < self.seq.stmts.len() {
                        let next = self.seq.stmts[idx].clone();
                        idx += 1;
                        env = ControlInterpreter::new(next, env, &self.info)
                            .run_and_deconstruct()?;
                    }
                    self.internal_state = SeqFsm::Done(env);
                    Ok(())
                } else {
                    unreachable!()
                }
            }
            SeqFsm::Done(_) => Ok(()),
        }
    }

    fn get_active_tree(&self) -> Vec<ActiveTreeNode> {
        match &self.internal_state {
            SeqFsm::Iterating(i, _) => i.get_active_tree(),
            SeqFsm::Done(_) => vec![],
            SeqFsm::Err => unreachable!("There is an error in the Seq state transition. Please report this."),
        }
    }
}

pub struct ParInterpreter {
    interpreters: Vec<ControlInterpreter>,
    in_state: InterpreterState,
    info: ComponentInfo,
}

impl ParInterpreter {
    pub fn new(
        par: Rc<iir::Par>,
        mut env: InterpreterState,
        info: ComponentInfo,
    ) -> Self {
        let mut env = env.force_fork();
        let interpreters = par
            .stmts
            .iter()
            .cloned()
            .map(|x| ControlInterpreter::new(x, env.fork(), &info))
            .collect();

        Self {
            interpreters,
            in_state: env,
            info,
        }
    }
}

impl Interpreter for ParInterpreter {
    fn step(&mut self) -> InterpreterResult<()> {
        for i in &mut self.interpreters {
            i.step()?;
        }
        Ok(())
    }

    fn deconstruct(self) -> InterpreterResult<InterpreterState> {
        assert!(self.interpreters.iter().all(|x| x.is_done()));
        let envs = self
            .interpreters
            .into_iter()
            .map(ControlInterpreter::deconstruct)
            .collect::<InterpreterResult<Vec<InterpreterState>>>()?;

        self.in_state.merge_many(envs, &self.info.input_ports)
    }

    fn is_done(&self) -> bool {
        self.interpreters.iter().all(|x| x.is_done())
    }

    fn get_env(&self) -> StateView<'_> {
        CompositeView::new(
            &self.in_state,
            self.interpreters.iter().map(|x| x.get_env()).collect(),
        )
        .into()
    }

    fn currently_executing_group(&self) -> HashSet<GroupQIN> {
        self.interpreters
            .iter()
            .flat_map(|x| x.currently_executing_group())
            .collect()
    }

    fn get_env_mut(&mut self) -> MutStateView<'_> {
        MutCompositeView::new(
            &mut self.in_state,
            self.interpreters
                .iter_mut()
                .map(|x| x.get_env_mut())
                .collect(),
        )
        .into()
    }

    fn converge(&mut self) -> InterpreterResult<()> {
        for res in self
            .interpreters
            .iter_mut()
            .map(ControlInterpreter::converge)
        {
            // return first error
            if let err @ Err(_) = res {
                return err;
            }
        }
        Ok(())
    }

    fn get_active_tree(&self) -> Vec<ActiveTreeNode> {
        self.interpreters
            .iter()
            .flat_map(|x| x.get_active_tree())
            .collect()
    }
}

enum IfFsm {
    Err, // transient error state
    Start(InterpreterState),
    Body(ControlInterpreter),
    Done(InterpreterState),
}

impl Default for IfFsm {
    fn default() -> Self {
        Self::Err
    }
}

pub struct IfInterpreter {
    state: IfFsm,
    ctrl_if: Rc<iir::If>,
    info: ComponentInfo,
}

impl IfInterpreter {
    pub fn new(
        ctrl_if: Rc<iir::If>,
        env: InterpreterState,
        info: ComponentInfo,
    ) -> Self {
        let state = IfFsm::Start(env);

        Self {
            state,
            ctrl_if,
            info,
        }
    }
}

impl Interpreter for IfInterpreter {
    fn step(&mut self) -> InterpreterResult<()> {
        match &mut self.state {
            IfFsm::Start(_) => {
                if let IfFsm::Start(mut env) = std::mem::take(&mut self.state) {
                    let branch_condition;

                    if let Some(cond) = &self.ctrl_if.cond {
                        let mut interp = EnableInterpreter::new(
                            cond,
                            Some(cond.borrow().name().clone()),
                            env,
                            self.info.continuous_assignments.clone(),
                            &self.info.qin,
                        );
                        interp.converge()?;
                        branch_condition =
                            interp.get(&self.ctrl_if.port).as_bool();
                        env = interp.deconstruct()?;
                    } else {
                        branch_condition =
                            env.get_from_port(&self.ctrl_if.port).as_bool();
                    }

                    let target = if branch_condition {
                        &self.ctrl_if.tbranch
                    } else {
                        &self.ctrl_if.fbranch
                    };

                    let mut interp = ControlInterpreter::new(
                        target.clone(),
                        env,
                        &self.info,
                    );

                    interp.step()?;
                    self.state = IfFsm::Body(interp);

                    Ok(())
                } else {
                    unreachable!();
                }
            }
            IfFsm::Body(b_interp) => {
                b_interp.step()?;
                if b_interp.is_done() {
                    if let IfFsm::Body(b_interp) =
                        std::mem::take(&mut self.state)
                    {
                        let env = b_interp.deconstruct()?;
                        self.state = IfFsm::Done(env);
                    } else {
                        unreachable!();
                    }
                }
                Ok(())
            }
            IfFsm::Done(_) => Ok(()),
            IfFsm::Err => {
                unimplemented!("There is an error in the If state transition. Please report this.")
            }
        }
    }

    fn deconstruct(self) -> InterpreterResult<InterpreterState> {
        match self.state {
            IfFsm::Done(e) => Ok(e),
            _ => Err(InterpreterError::InvalidIfState),
        }
    }

    fn is_done(&self) -> bool {
        matches!(self.state, IfFsm::Done(_))
    }

    fn get_env(&self) -> StateView<'_> {
        match &self.state {
            IfFsm::Done(e) | IfFsm::Start(e) => e.into(),
            IfFsm::Body(b) => b.get_env(),
            IfFsm::Err => unreachable!("There is an error in the If state transition. Please report this."),
        }
    }

    fn currently_executing_group(&self) -> HashSet<GroupQIN> {
        match &self.state {
            IfFsm::Done(_) | IfFsm::Start(_) => HashSet::new(),
            IfFsm::Body(b) => b.currently_executing_group(),
            IfFsm::Err => unreachable!("There is an error in the If state transition. Please report this."),
        }
    }

    fn get_env_mut(&mut self) -> MutStateView<'_> {
        match &mut self.state {
            IfFsm::Done(e) | IfFsm::Start(e) => e.into(),
            IfFsm::Body(b) => b.get_env_mut(),
            IfFsm::Err => unreachable!("There is an error in the If state transition. Please report this."),
        }
    }

    fn converge(&mut self) -> InterpreterResult<()> {
        match &mut self.state {
            IfFsm::Err => unreachable!("There is an error in the If state transition. Please report this."),
            IfFsm::Body(b_interp) => b_interp.converge(),
            IfFsm::Start(_) | IfFsm::Done(_) => {
                let is_done = matches!(self.state, IfFsm::Done(_));
                if let IfFsm::Start(env) | IfFsm::Done(env) =
                    std::mem::take(&mut self.state)
                {
                    let mut interp = EnableInterpreter::new(
                        vec![],
                        None,
                        env,
                        self.info.continuous_assignments.clone(),
                        &self.info.qin,
                    );
                    interp.converge()?;
                    let env = interp.deconstruct()?;

                    if is_done {
                        self.state = IfFsm::Done(env)
                    } else {
                        self.state = IfFsm::Start(env)
                    }
                    Ok(())
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn get_active_tree(&self) -> Vec<ActiveTreeNode> {
        match &self.state {
            IfFsm::Done(_) | IfFsm::Start(_) => Vec::new(),
            IfFsm::Body(b) => b.get_active_tree(),
            IfFsm::Err => unreachable!("There is an error in the If state transition. Please report this."),
        }
    }
}

enum WhileFsm {
    Err, // transient error state
    Start(InterpreterState),
    Body(ControlInterpreter),
    Done(InterpreterState),
}

impl Default for WhileFsm {
    fn default() -> Self {
        Self::Err
    }
}
pub struct WhileInterpreter {
    state: WhileFsm,
    wh: Rc<iir::While>,
    info: ComponentInfo,
}

impl WhileInterpreter {
    pub fn new(
        ctrl_while: Rc<iir::While>,
        env: InterpreterState,
        info: ComponentInfo,
    ) -> Self {
        Self {
            info,
            state: WhileFsm::Start(env),
            wh: ctrl_while,
        }
    }
}

impl Interpreter for WhileInterpreter {
    fn step(&mut self) -> InterpreterResult<()> {
        match &mut self.state {
            WhileFsm::Err => unreachable!("There is an error in the If state transition. Please report this."),
            WhileFsm::Start(_) => {
                if let WhileFsm::Start(mut env) =
                    std::mem::take(&mut self.state)
                {
                    let branch_condition;

                    if let Some(cond) = &self.wh.cond {
                        let mut interp = EnableInterpreter::new(
                            cond,
                            Some(cond.borrow().name().clone()),
                            env,
                            self.info.continuous_assignments.clone(),
                            &self.info.qin,
                        );
                        interp.converge()?;
                        branch_condition = interp.get(&self.wh.port).as_bool();
                        env = interp.deconstruct()?;
                    } else {
                        branch_condition =
                            env.get_from_port(&self.wh.port).as_bool();
                    }

                    if !branch_condition {
                        self.state = WhileFsm::Done(env);
                    } else {
                        let mut interp = ControlInterpreter::new(
                            self.wh.body.clone(),
                            env,
                            &self.info,
                        );
                        interp.step()?;
                        self.state = WhileFsm::Body(interp);
                    }
                    Ok(())
                } else {
                    unreachable!()
                }
            }
            WhileFsm::Body(b) => {
                b.step()?;
                if b.is_done() {
                    if let WhileFsm::Body(b) = std::mem::take(&mut self.state) {
                        let env = b.deconstruct()?;
                        self.state = WhileFsm::Start(env);
                    } else {
                        unreachable!()
                    }
                }
                Ok(())
            }
            WhileFsm::Done(_) => Ok(()),
        }
    }

    fn deconstruct(self) -> InterpreterResult<InterpreterState> {
        match self.state {
            WhileFsm::Done(e) => Ok(e),
            _ => Err(InterpreterError::InvalidWhileState),
        }
    }

    fn is_done(&self) -> bool {
        matches!(self.state, WhileFsm::Done(_))
    }

    fn get_env(&self) -> StateView<'_> {
        match &self.state {
            WhileFsm::Err => unreachable!("There is an error in the While state transition. Please report this."),
            WhileFsm::Start(e) | WhileFsm::Done(e) => e.into(),
            WhileFsm::Body(b) => b.get_env(),
        }
    }

    fn currently_executing_group(&self) -> HashSet<GroupQIN> {
        match &self.state {
            WhileFsm::Err => unreachable!("There is an error in the While state transition. Please report this."),
            WhileFsm::Start(_) | WhileFsm::Done(_) => HashSet::new(),
            WhileFsm::Body(b) => b.currently_executing_group(),
        }
    }

    fn get_env_mut(&mut self) -> MutStateView<'_> {
        match &mut self.state {
            WhileFsm::Err => unreachable!("There is an error in the While state transition. Please report this."),
            WhileFsm::Start(e) | WhileFsm::Done(e) => e.into(),
            WhileFsm::Body(b) => b.get_env_mut(),
        }
    }

    fn converge(&mut self) -> InterpreterResult<()> {
        match &mut self.state {
            WhileFsm::Err => unreachable!("There is an error in the While state transition. Please report this."),
            WhileFsm::Body(b) => b.converge(),
            WhileFsm::Start(_) | WhileFsm::Done(_) => {
                let is_done = matches!(self.state, WhileFsm::Done(_));
                if let WhileFsm::Start(env) | WhileFsm::Done(env) =
                    std::mem::take(&mut self.state)
                {
                    let mut interp = EnableInterpreter::new(
                        vec![],
                        None,
                        env,
                        self.info.continuous_assignments.clone(),
                        &self.info.qin,
                    );
                    interp.converge()?;
                    let env = interp.deconstruct()?;

                    if is_done {
                        self.state = WhileFsm::Done(env)
                    } else {
                        self.state = WhileFsm::Start(env)
                    }
                    Ok(())
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn get_active_tree(&self) -> Vec<ActiveTreeNode> {
        match &self.state {
            WhileFsm::Err => unreachable!("There is an error in the while state transition. Please report this."),
            WhileFsm::Start(_) | WhileFsm::Done(_) => vec![],
            WhileFsm::Body(b) => b.get_active_tree(),
        }
    }
}
pub struct InvokeInterpreter {
    invoke: Rc<iir::Invoke>,
    assign_interp: AssignmentInterpreter,
    qin: ComponentQualifiedInstanceName,
}

impl InvokeInterpreter {
    pub fn new(
        invoke: Rc<iir::Invoke>,
        mut env: InterpreterState,
        continuous_assignments: iir::ContinuousAssignments,
        qin: ComponentQualifiedInstanceName,
    ) -> Self {
        let mut assignment_vec: Vec<Assignment> = vec![];
        let comp_cell = invoke.comp.borrow();

        //first connect the inputs (from connection -> input)
        for (input_port, connection) in &invoke.inputs {
            let comp_input_port = comp_cell.get(input_port);
            assignment_vec.push(Assignment {
                dst: comp_input_port,
                src: Rc::clone(connection),
                guard: Guard::default().into(),
                attributes: ir::Attributes::default(),
            });
        }

        //second connect the output ports (from output -> connection)
        for (output_port, connection) in &invoke.outputs {
            let comp_output_port = comp_cell.get(output_port);
            assignment_vec.push(Assignment {
                dst: Rc::clone(connection),
                src: comp_output_port,
                guard: Guard::default().into(),
                attributes: ir::Attributes::default(),
            })
        }

        // insert with assignments, if present
        if let Some(with) = &invoke.comb_group {
            let w_ref = with.borrow();
            // TODO (Griffin): probably should avoid duplicating these.
            assignment_vec.extend(w_ref.assignments.iter().cloned());
        }

        let go_port = comp_cell.get_with_attr("go");
        // insert one into the go_port
        // should probably replace with an actual assignment from a constant one
        env.insert(go_port, Value::bit_high());

        let comp_done_port = comp_cell.get_with_attr("done");
        let interp = AssignmentInterpreter::new(
            env,
            comp_done_port.into(),
            assignment_vec,
            continuous_assignments,
        );

        drop(comp_cell);

        Self {
            invoke,
            assign_interp: interp,
            qin,
        }
    }
}

impl Interpreter for InvokeInterpreter {
    fn step(&mut self) -> InterpreterResult<()> {
        self.assign_interp.step()
    }

    fn run(&mut self) -> InterpreterResult<()> {
        self.assign_interp.run()
    }

    fn deconstruct(self) -> InterpreterResult<InterpreterState> {
        let mut env = self.assign_interp.reset()?;

        // set go low
        let go_port = self.invoke.comp.borrow().get_with_attr("go");
        // insert one into the go_port
        // should probably replace with an actual assignment from a constant one
        env.insert(go_port, Value::bit_low());

        Ok(env)
    }

    fn is_done(&self) -> bool {
        self.assign_interp.is_deconstructable()
    }

    fn get_env(&self) -> StateView<'_> {
        self.assign_interp.get_env().into()
    }

    fn currently_executing_group(&self) -> HashSet<GroupQIN> {
        HashSet::new()
    }

    fn get_env_mut(&mut self) -> MutStateView<'_> {
        self.assign_interp.get_mut_env().into()
    }

    fn converge(&mut self) -> InterpreterResult<()> {
        self.assign_interp.step_convergence()
    }

    fn get_active_tree(&self) -> Vec<ActiveTreeNode> {
        let name = GroupQualifiedInstanceName::new_phantom(
            &self.qin,
            &(format!("invoke {}", self.invoke.comp.borrow().name()).into()),
        );

        vec![ActiveTreeNode::new(name)]
    }
}

// internal use macro that just captures the same name and expression for each
// arm of the control interpreter match. This is largely as a convenience
macro_rules! control_match {
    ($matched: ident, $name:ident, $exp:expr) => {{
        match $matched {
            ControlInterpreter::Empty($name) => $exp,
            ControlInterpreter::Enable($name) => $exp,
            ControlInterpreter::Seq($name) => $exp,
            ControlInterpreter::Par($name) => $exp,
            ControlInterpreter::If($name) => $exp,
            ControlInterpreter::While($name) => $exp,
            ControlInterpreter::Invoke($name) => $exp,
        }
    }};
}

pub enum ControlInterpreter {
    Empty(Box<EmptyInterpreter>),
    Enable(Box<EnableInterpreter>),
    Seq(Box<SeqInterpreter>),
    Par(Box<ParInterpreter>),
    If(Box<IfInterpreter>),
    While(Box<WhileInterpreter>),
    Invoke(Box<InvokeInterpreter>),
}

impl ControlInterpreter {
    pub fn new(
        control: iir::Control,
        env: InterpreterState,
        info: &ComponentInfo,
    ) -> Self {
        match control {
            iir::Control::Seq(s) => {
                Self::Seq(Box::new(SeqInterpreter::new(s, env, info.clone())))
            }
            iir::Control::Par(par) => {
                Self::Par(Box::new(ParInterpreter::new(par, env, info.clone())))
            }
            iir::Control::If(i) => {
                Self::If(Box::new(IfInterpreter::new(i, env, info.clone())))
            }
            iir::Control::While(w) => Self::While(Box::new(
                WhileInterpreter::new(w, env, info.clone()),
            )),
            iir::Control::Invoke(i) => {
                Self::Invoke(Box::new(InvokeInterpreter::new(
                    i,
                    env,
                    info.continuous_assignments.clone(),
                    info.qin.clone(),
                )))
            }
            iir::Control::Enable(e) => {
                Self::Enable(Box::new(EnableInterpreter::new(
                    &e.group,
                    Some(e.group.borrow().name().clone()),
                    env,
                    info.continuous_assignments.clone(),
                    &info.qin,
                )))
            }
            iir::Control::Empty(_) => {
                Self::Empty(Box::new(EmptyInterpreter::new(env)))
            }
        }
    }
}

impl Interpreter for ControlInterpreter {
    fn step(&mut self) -> InterpreterResult<()> {
        control_match!(self, i, i.step())
    }

    fn run(&mut self) -> InterpreterResult<()> {
        control_match!(self, i, i.run())
    }

    fn deconstruct(self) -> InterpreterResult<InterpreterState> {
        control_match!(self, i, i.deconstruct())
    }

    fn is_done(&self) -> bool {
        control_match!(self, i, i.is_done())
    }

    fn get_env(&self) -> StateView<'_> {
        control_match!(self, i, i.get_env())
    }

    fn currently_executing_group(&self) -> HashSet<GroupQIN> {
        control_match!(self, i, i.currently_executing_group())
    }

    fn get_env_mut(&mut self) -> MutStateView<'_> {
        control_match!(self, i, i.get_env_mut())
    }

    fn converge(&mut self) -> InterpreterResult<()> {
        control_match!(self, i, i.converge())
    }

    fn get_active_tree(&self) -> Vec<ActiveTreeNode> {
        control_match!(self, i, i.get_active_tree())
    }
}

pub struct StructuralInterpreter {
    interp: AssignmentInterpreter,
    continuous: iir::ContinuousAssignments,
    done_port: ConstPort,
}

impl StructuralInterpreter {
    pub fn from_component(
        comp: &Rc<iir::Component>,
        env: InterpreterState,
    ) -> Self {
        let comp_sig = comp.signature.borrow();
        let done_port = comp_sig.get_with_attr("done");
        let done_raw = done_port.as_raw();
        let continuous = Rc::clone(&comp.continuous_assignments);
        let assigns: Vec<ir::Assignment> = vec![];

        let interp = AssignmentInterpreter::new(
            env,
            Some(done_port),
            assigns,
            continuous.clone(),
        );

        Self {
            interp,
            continuous,
            done_port: done_raw,
        }
    }
}

impl Interpreter for StructuralInterpreter {
    fn step(&mut self) -> InterpreterResult<()> {
        self.interp.force_step_cycle()
    }

    fn deconstruct(self) -> InterpreterResult<InterpreterState> {
        let final_env = self.interp.deconstruct()?;
        finish_interpretation(
            final_env,
            Some(self.done_port),
            self.continuous.iter(),
        )
    }

    fn run(&mut self) -> InterpreterResult<()> {
        self.interp.run()
    }

    fn is_done(&self) -> bool {
        self.interp.is_deconstructable()
    }

    fn get_env(&self) -> StateView<'_> {
        self.interp.get_env().into()
    }

    fn currently_executing_group(&self) -> HashSet<GroupQIN> {
        HashSet::new()
    }

    fn get_env_mut(&mut self) -> MutStateView<'_> {
        self.interp.get_mut_env().into()
    }

    fn converge(&mut self) -> InterpreterResult<()> {
        self.interp.step_convergence()
    }

    fn get_active_tree(&self) -> Vec<ActiveTreeNode> {
        vec![]
    }
}