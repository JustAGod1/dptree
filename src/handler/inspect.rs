use crate::{
    di::{Asyncify, Injectable},
    from_fn_with_description, Handler, HandlerDescription,
};

use std::sync::Arc;

/// Constructs a handler that inspects current state. Like [`map`] but does not
/// add return value of `f` to the container.
///
/// [`map`]: crate::map
#[must_use]
#[track_caller]
pub fn inspect<'a, F, Input, Output, Args, Descr>(f: F) -> Handler<'a, Input, Output, Descr>
where
    Asyncify<F>: Injectable<Input, (), Args> + Send + Sync + 'a,
    Input: Send + 'a,
    Output: 'a,
    Descr: HandlerDescription,
{
    inspect_with_description(Descr::inspect(), f)
}

/// The asynchronous version of [`inspect`].
#[must_use]
#[track_caller]
pub fn inspect_async<'a, F, Input, Output, Args, Descr>(f: F) -> Handler<'a, Input, Output, Descr>
where
    F: Injectable<Input, (), Args> + Send + Sync + 'a,
    Input: Send + 'a,
    Output: 'a,
    Descr: HandlerDescription,
{
    inspect_async_with_description(Descr::inspect_async(), f)
}

/// [`inspect`] with a custom description.
#[must_use]
pub fn inspect_with_description<'a, F, Input, Output, Args, Descr>(
    description: Descr,
    f: F,
) -> Handler<'a, Input, Output, Descr>
where
    Asyncify<F>: Injectable<Input, (), Args> + Send + Sync + 'a,
    Input: Send + 'a,
    Output: 'a,
{
    inspect_async_with_description(description, Asyncify(f))
}

/// [`inspect_async`] with a custom description.
#[must_use]
pub fn inspect_async_with_description<'a, F, Input, Output, Args, Descr>(
    description: Descr,
    f: F,
) -> Handler<'a, Input, Output, Descr>
where
    F: Injectable<Input, (), Args> + Send + Sync + 'a,
    Input: Send + 'a,
    Output: 'a,
{
    let f = Arc::new(f);

    from_fn_with_description(description, move |x, cont| {
        let f = Arc::clone(&f);
        async move {
            {
                let f = f.inject(&x);
                f().await;
            }

            cont(x).await
        }
    })
}
