use crate::template::Template as TransactionTemplate;

#[macro_use]
pub mod macros;
pub mod actions;
pub mod compiler;
pub mod object;

use super::template::*;
use bitcoin::util::amount::{Amount, CoinAmount};
pub use compiler::Compilable;
pub use object::Object as Compiled;
use std::collections::HashMap;

use std::error::Error;
use std::fmt;
#[derive(Debug)]
pub enum CompilationError {
    TerminateCompilation,
    MissingTemplates,
    EmptyPolicy,
    OutOfFunds,
    ParseAmountError(bitcoin::util::amount::ParseAmountError),
    Miniscript(miniscript::policy::compiler::CompilerError),
}

impl From<bitcoin::util::amount::ParseAmountError> for CompilationError {
    fn from(b: bitcoin::util::amount::ParseAmountError) -> Self {
        CompilationError::ParseAmountError(b)
    }
}
impl From<miniscript::policy::compiler::CompilerError> for CompilationError {
    fn from(v: miniscript::policy::compiler::CompilerError) -> Self {
        CompilationError::Miniscript(v)
    }
}

impl fmt::Display for CompilationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for CompilationError {}
/// An Iterator which yields TransactionTemplates.
/// It is boxed to permit flexibility when returning.
pub type TxTmplIt = Result<
    Box<dyn Iterator<Item = Result<TransactionTemplate, CompilationError>>>,
    CompilationError,
>;

/// A catch-all type for any function that is a FinishOrFunc.
/// Unfortunately, because type signatures must all match, it's not
/// possible to have differing types across FinishOrFunc for a contract at compile time.
/// Use an enum if need be.
///
/// TODO: use associated-type defaults here!
pub trait Contract
where
    Self: Sized + 'static,
    Option<Self::StatefulArguments>: Default,
{
    //! Main Contract Trait
    declare! {then}
    declare! { updatable<> }
    declare! {finish}
}

/// DynamicContract wraps a struct S with a set of methods (that can be constructed dynamically)
/// to form a contract. DynamicContract owns all its methods.
pub struct DynamicContract<T, S>
where
    S: 'static,
{
    pub then: Vec<fn() -> Option<actions::ThenFunc<S>>>,
    pub finish_or: Vec<fn() -> Option<actions::FinishOrFunc<S, T>>>,
    pub finish: Vec<fn() -> Option<actions::Guard<S>>>,
    pub data: S,
}

impl<T, S> AnyContract for DynamicContract<T, S> {
    type StatefulArguments = T;
    type Ref = S;
    fn then_fns<'a>(&'a self) -> &'a [fn() -> Option<actions::ThenFunc<S>>] {
        &self.then[..]
    }
    fn finish_or_fns<'a>(
        &'a self,
    ) -> &'a [fn() -> Option<actions::FinishOrFunc<S, Self::StatefulArguments>>] {
        &self.finish_or[..]
    }
    fn finish_fns<'a>(&'a self) -> &'a [fn() -> Option<actions::Guard<S>>] {
        &self.finish[..]
    }
    fn get_inner_ref<'a>(&self) -> &Self::Ref {
        &self.data
    }
}
/// AnyContract is a generic API for types which can be compiled, encapsulating default static
/// Contracts as well as DynamicContracts/DynamicContractRefs.
///
/// This assists in abstracting the layout/internals away from something that can be compiled.
pub trait AnyContract
where
    Self: Sized,
{
    type StatefulArguments;
    type Ref;
    fn then_fns<'a>(&'a self) -> &'a [fn() -> Option<actions::ThenFunc<Self::Ref>>];
    fn finish_or_fns<'a>(
        &'a self,
    ) -> &'a [fn() -> Option<actions::FinishOrFunc<Self::Ref, Self::StatefulArguments>>];
    fn finish_fns<'a>(&'a self) -> &'a [fn() -> Option<actions::Guard<Self::Ref>>];
    fn get_inner_ref<'a>(&'a self) -> &'a Self::Ref;
}

impl<C> AnyContract for C
where
    C: Contract + Sized,
{
    type StatefulArguments = C::StatefulArguments;
    type Ref = Self;
    fn then_fns<'a>(&'a self) -> &'a [fn() -> Option<actions::ThenFunc<Self::Ref>>] {
        Self::THEN_FNS
    }
    fn finish_or_fns<'a>(
        &'a self,
    ) -> &'a [fn() -> Option<actions::FinishOrFunc<Self::Ref, Self::StatefulArguments>>] {
        Self::FINISH_OR_FUNCS
    }
    fn finish_fns<'a>(&'a self) -> &'a [fn() -> Option<actions::Guard<Self::Ref>>] {
        Self::FINISH_FNS
    }
    fn get_inner_ref<'a>(&'a self) -> &Self::Ref {
        self
    }
}

#[derive(Clone)]
pub struct Context {
    /* TODO: Add Context Fields! */
    available_funds: Amount,
}

impl Context {
    pub fn new(amount: Amount) -> Self {
        Context {
            available_funds: amount,
        }
    }
    pub fn compile<A: Compilable>(&self, a: A) -> Result<Compiled, CompilationError> {
        a.compile(&self)
    }
    // TODO: Fix
    pub fn with_amount(&self, amount: Amount) -> Result<Self, CompilationError> {
        if self.available_funds < amount {
            Err(CompilationError::OutOfFunds)
        } else {
            Ok(Context {
                available_funds: amount,
                ..self.clone()
            })
        }
    }
    pub fn spend_amount(&mut self, amount: Amount) -> Result<(), CompilationError> {
        if self.available_funds < amount {
            Err(CompilationError::OutOfFunds)
        } else {
            self.available_funds -= amount;
            Ok(())
        }
    }

    pub fn add_amount(&mut self, amount: Amount) {
        self.available_funds += amount;
    }

    pub fn template(&self) -> crate::template::Builder {
        crate::template::Builder::new(self.clone())
    }
}
