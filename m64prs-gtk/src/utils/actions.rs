use std::{borrow::Borrow, marker::PhantomData};

use gio::prelude::{ActionExt, FromVariant, ToVariant};
use glib::SignalHandlerId;

use gio::prelude::*;

use super::t_option::{TNone, TOption, TSome};

/// Type-safe wrapper around [`gio::SimpleAction`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypedAction<OS, OP>
where
    OS: OptionVariantType,
    OP: OptionVariantType,
{
    inner: gio::SimpleAction,
    _marker: PhantomData<(OS, OP)>,
}

pub type BaseAction = TypedAction<TNone, TNone>;
pub type StateAction<S> = TypedAction<TSome<S>, TNone>;
pub type ParamAction<P> = TypedAction<TNone, TSome<P>>;
pub type StateParamAction<S, P> = TypedAction<TSome<S>, TSome<P>>;

impl<OS, OP> TypedAction<OS, OP>
where
    OS: OptionVariantType,
    OP: OptionVariantType,
{
    /// Wraps an existing [`gio::SimpleAction`].
    /// # Safety
    /// The caller is responsible for ensuring the parameter type and state
    /// match up with the type parameters.
    pub unsafe fn with_inner(action: &gio::SimpleAction) -> Self {
        Self {
            inner: action.clone(),
            _marker: PhantomData,
        }
    }

    /// Retrieves the wrapped [`gio::SimpleAction`]. This allows you
    /// to use functionality not directly exposed by this API.
    pub fn inner(&self) -> &gio::SimpleAction {
        &self.inner
    }

    /// Checks whether the action is enabled. Actions may only
    /// be activated when they are enabled.
    /// 
    /// **See also:** [`gio::SimpleAction::is_enabled`]
    pub fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }

    /// Sets whether the action is enabled. Actions may only
    /// be activated when they are enabled.
    /// 
    /// **See also:** [`gio::SimpleAction::set_enabled`]
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.set_enabled(enabled);
    }
}

impl TypedAction<TNone, TNone> {
    /// Constructs a new action with no state or parameter.
    pub fn new(name: &str) -> Self {
        Self {
            inner: gio::SimpleAction::new(name, None),
            _marker: PhantomData,
        }
    }
}

impl<S> TypedAction<TSome<S>, TNone>
where
    S: FromVariant + ToVariant,
{
    /// Constructs a new action with state, but no parameter.
    pub fn new<RS: Borrow<S>>(name: &str, init_state: RS) -> Self {
        Self {
            inner: gio::SimpleAction::new_stateful(name, None, &init_state.borrow().to_variant()),
            _marker: PhantomData,
        }
    }
}

impl<P> TypedAction<TNone, TSome<P>>
where
    P: FromVariant + ToVariant,
{
    /// Constructs a new action with parameter, but no state.
    pub fn new(name: &str) -> Self {
        Self {
            inner: gio::SimpleAction::new(name, Some(&P::static_variant_type())),
            _marker: PhantomData,
        }
    }
}

impl<S, P> TypedAction<TSome<S>, TSome<P>>
where
    S: FromVariant + ToVariant,
    P: FromVariant + ToVariant,
{
    /// Constructs a new action with both state and parameter.
    pub fn new<RS: Borrow<S>>(name: &str, init_state: RS) -> Self {
        Self {
            inner: gio::SimpleAction::new_stateful(
                name,
                Some(&P::static_variant_type()),
                &init_state.borrow().to_variant(),
            ),
            _marker: PhantomData,
        }
    }
}

impl<OS> TypedAction<OS, TNone>
where
    OS: OptionVariantType,
{
    /// Binds a handler to this action. The closure should satisfy the following signature:
    /// ```rust,ignore
    /// fn activate_handler(action: &TypedAction<OS, TNone>);
    /// ```
    pub fn connect_activate<F: Fn(&Self) + 'static>(&self, f: F) -> SignalHandlerId {
        self.inner.connect_activate(move |action, _| {
            let action = unsafe { Self::with_inner(action) };
            f(&action)
        })
    }

    /// Triggers this action.
    pub fn activate(&self) {
        self.inner.activate(None);
    }
}

impl<OS, P> TypedAction<OS, TSome<P>>
where
    OS: OptionVariantType,
    P: ToVariant + FromVariant,
{
    /// Binds a handler to this action. The closure should satisfy the following signature:
    /// ```rust,ignore
    /// fn activate_handler(action: &TypedAction<OS, TSome<P>>, param: P);
    /// ```
    pub fn connect_activate<F: Fn(&Self, P) + 'static>(&self, f: F) -> SignalHandlerId {
        self.inner.connect_activate(move |action, param| {
            let action = unsafe { Self::with_inner(action) };
            let inner = P::from_variant(&param.expect("action should have parameter"))
                .expect("failed to convert variant to P");
            f(&action, inner)
        })
    }

    /// Triggers this action with the provided parameter.
    pub fn activate(&self, param: &P) {
        self.inner.activate(Some(&param.to_variant()));
    }
}

impl<S, OP> TypedAction<TSome<S>, OP>
where
    S: ToVariant + FromVariant,
    OP: OptionVariantType,
{
    /// Gets the action's current state.
    pub fn state(&self) -> S {
        S::from_variant(&self.inner.state().expect("action state should exist"))
            .expect("action state should match type S")
    }

    /// Sets the action's current state.
    pub fn set_state(&self, state: &S) {
        self.inner.set_state(&S::to_variant(state));
    }
}

pub trait ActionGroupTypedExt: IsA<gio::ActionMap> {
    /// Adds a typed action to an action map.
    fn register_action<OS, OP>(&self, action: &TypedAction<OS, OP>)
    where
        OS: OptionVariantType,
        OP: OptionVariantType,
    {
        self.add_action(action.inner());
    }
}
impl<T> ActionGroupTypedExt for T
where
    T: IsA<gio::ActionMap> {}


mod sealed {
    pub trait Sealed {}
}
impl<T: TOption> sealed::Sealed for T {}

pub trait OptionVariantType: sealed::Sealed {}
impl OptionVariantType for TNone {}
impl<T: ToVariant + FromVariant> OptionVariantType for TSome<T> {}