use std::{future::Future, ops::ControlFlow, sync::Arc};

use futures::future::BoxFuture;

/// An instance that receives an input and decides whether to break a chain or
/// pass the value further.
///
/// In order to create this structure, you can use the predefined functions from
/// [`crate`].
pub struct Handler<'a, Input, Output> {
    #[allow(clippy::type_complexity)]
    f: Arc<
        dyn Fn(Input, Cont<'a, Input, Output>) -> HandlerResult<'a, Input, Output>
            + Send
            + Sync
            + 'a,
    >,
}

/// A continuation representing the rest of a handler chain.
pub type Cont<'a, Input, Output> =
    Box<dyn Fn(Input) -> HandlerResult<'a, Input, Output> + Send + Sync + 'a>;

/// An output type produced by a handler.
pub type HandlerResult<'a, Input, Output> = BoxFuture<'a, ControlFlow<Output, Input>>;

// `#[derive(Clone)]` obligates all type parameters to satisfy `Clone` as well,
// but we do not need it here because of `Arc`.
impl<'a, Input, Output> Clone for Handler<'a, Input, Output> {
    fn clone(&self) -> Self {
        Handler { f: Arc::clone(&self.f) }
    }
}

impl<'a, Input, Output> Handler<'a, Input, Output>
where
    Input: Send + Sync + 'a,
    Output: Send + Sync + 'a,
{
    /// Chain two handlers to form a [chain of responsibility].
    ///
    /// First, `self` will be executed, and then, if `self` decides to continue
    /// execution, `next` will be executed.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[tokio::main]
    /// # async fn main() {
    /// use dptree::prelude::*;
    ///
    /// let handler = dptree::filter(|x: i32| x > 0).chain(dptree::endpoint(|| async { "done" }));
    ///
    /// assert_eq!(handler.dispatch(dptree::deps![10]).await, ControlFlow::Break("done"));
    /// assert_eq!(
    ///     handler.dispatch(dptree::deps![-10]).await,
    ///     ControlFlow::Continue(dptree::deps![-10])
    /// );
    ///
    /// # }
    /// ```
    ///
    /// [chain of responsibility]: https://en.wikipedia.org/wiki/Chain-of-responsibility_pattern
    #[must_use]
    pub fn chain(self, next: Self) -> Self {
        from_fn(move |event, cont| {
            let this = self.clone();
            let next = next.clone();
            let cont = Arc::new(cont);

            this.execute(event, move |event| {
                let next = next.clone();
                let cont = cont.clone();

                #[allow(clippy::redundant_closure)] // Clippy is a fucking donkey.
                next.execute(event, move |event| cont(event))
            })
        })
    }

    /// Chain two handlers to make a tree of responsibility.
    ///
    /// This function is the same as [`Handler::chain`] but instead of expanding
    /// a chain, it adds a new branch, thereby forming a tree.
    ///
    /// # Examples
    ///
    /// ```
    /// use dptree::prelude::*;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    ///
    /// #[derive(Debug, PartialEq)]
    /// enum Output {
    ///     Five,
    ///     One,
    ///     GT,
    /// }
    ///
    /// let dispatcher = dptree::entry()
    ///     .branch(dptree::filter(|num: i32| num == 5).endpoint(|| async move { Output::Five }))
    ///     .branch(dptree::filter(|num: i32| num == 1).endpoint(|| async move { Output::One }))
    ///     .branch(dptree::filter(|num: i32| num > 2).endpoint(|| async move { Output::GT }));
    ///
    /// assert_eq!(dispatcher.dispatch(dptree::deps![5]).await, ControlFlow::Break(Output::Five));
    /// assert_eq!(dispatcher.dispatch(dptree::deps![1]).await, ControlFlow::Break(Output::One));
    /// assert_eq!(dispatcher.dispatch(dptree::deps![3]).await, ControlFlow::Break(Output::GT));
    /// assert_eq!(
    ///     dispatcher.dispatch(dptree::deps![0]).await,
    ///     ControlFlow::Continue(dptree::deps![0])
    /// );
    /// # }
    /// ```
    #[must_use]
    pub fn branch(self, next: Self) -> Self {
        from_fn(move |event, cont| {
            let this = self.clone();
            let next = next.clone();
            let cont = Arc::new(cont);

            this.execute(event, move |event| {
                let next = next.clone();
                let cont = cont.clone();

                async move {
                    match next.dispatch(event).await {
                        ControlFlow::Continue(event) => cont(event).await,
                        done => done,
                    }
                }
            })
        })
    }

    /// Executes this handler with a continuation.
    ///
    /// Usually, you do not want to call this method by yourself, if you do not
    /// write your own handler implementation. If you wish to execute handler
    /// without a continuation, take a look at the [`Handler::dispatch`] method.
    ///
    /// # Examples
    ///
    /// ```
    /// # #[tokio::main]
    /// # async fn main() {
    /// use dptree::prelude::*;
    ///
    /// let handler = dptree::filter(|x: i32| x > 0);
    ///
    /// let output = handler.execute(dptree::deps![10], |_| async { ControlFlow::Break("done") }).await;
    /// assert_eq!(output, ControlFlow::Break("done"));
    ///
    /// # }
    /// ```
    pub async fn execute<Cont, ContFut>(
        self,
        container: Input,
        cont: Cont,
    ) -> ControlFlow<Output, Input>
    where
        Cont: Fn(Input) -> ContFut,
        Cont: Send + Sync + 'a,
        ContFut: Future<Output = ControlFlow<Output, Input>> + Send + 'a,
    {
        (self.f)(container, Box::new(move |event| Box::pin(cont(event)))).await
    }

    /// Executes this handler.
    ///
    /// Returns [`ControlFlow::Break`] when executed successfully,
    /// [`ControlFlow::Continue`] otherwise.
    pub async fn dispatch(&self, container: Input) -> ControlFlow<Output, Input> {
        self.clone().execute(container, |event| async move { ControlFlow::Continue(event) }).await
    }
}

/// Constructs a handler from a function.
///
/// Most of the time, you do not want to use this function. Take a look at more
/// specialised functions: [`crate::endpoint`], [`crate::filter`],
/// [`crate::filter_map`], etc.
#[must_use]
pub fn from_fn<'a, F, Fut, Input, Output>(f: F) -> Handler<'a, Input, Output>
where
    F: Fn(Input, Cont<'a, Input, Output>) -> Fut,
    F: Send + Sync + 'a,
    Fut: Future<Output = ControlFlow<Output, Input>> + Send + 'a,
{
    Handler { f: Arc::new(move |event, cont| Box::pin(f(event, cont))) }
}

/// Constructs an entry point handler.
///
/// This function is only used to specify other handlers upon it (see the root
/// examples).
#[must_use]
pub fn entry<'a, Input, Output>() -> Handler<'a, Input, Output>
where
    Input: Send + Sync + 'a,
    Output: Send + Sync + 'a,
{
    from_fn(|event, cont| cont(event))
}

#[cfg(test)]
mod tests {
    use crate::{
        deps,
        handler::{endpoint, filter, filter_async},
    };

    use super::*;

    #[tokio::test]
    async fn test_from_fn_break() {
        let input = 123;
        let output = "ABC";

        let result = from_fn(|event, _cont: Cont<i32, &'static str>| async move {
            assert_eq!(event, input);
            ControlFlow::Break(output)
        })
        .dispatch(input)
        .await;

        assert!(result == ControlFlow::Break(output));
    }

    #[tokio::test]
    async fn test_from_fn_continue() {
        let input = 123;
        type Output = &'static str;

        let result = from_fn(|event: i32, _cont: Cont<i32, &'static str>| async move {
            assert_eq!(event, input);
            ControlFlow::<Output, _>::Continue(event)
        })
        .dispatch(input)
        .await;

        assert!(result == ControlFlow::Continue(input));
    }

    #[tokio::test]
    async fn test_entry() {
        let input = 123;
        type Output = &'static str;

        let result = entry::<_, Output>().dispatch(input).await;

        assert!(result == ControlFlow::Continue(input));
    }

    #[tokio::test]
    async fn test_execute() {
        let input = 123;
        let output = "ABC";

        let result = from_fn(|event, cont| {
            assert!(event == input);
            cont(event)
        })
        .execute(input, |event| async move {
            assert!(event == input);
            ControlFlow::Break(output)
        })
        .await;

        assert!(result == ControlFlow::Break(output));
    }

    #[tokio::test]
    async fn test_deeply_nested_tree() {
        #[derive(Debug, PartialEq)]
        enum Output {
            LT,
            MinusOne,
            Zero,
            One,
            GT,
        }

        let negative_handler = filter(|num: i32| num < 0)
            .branch(
                filter_async(|num: i32| async move { num == -1 })
                    .endpoint(|| async move { Output::MinusOne }),
            )
            .branch(endpoint(|| async move { Output::LT }));

        let zero_handler = filter_async(|num: i32| async move { num == 0 })
            .endpoint(|| async move { Output::Zero });

        let positive_handler = filter_async(|num: i32| async move { num > 0 })
            .branch(
                filter_async(|num: i32| async move { num == 1 })
                    .endpoint(|| async move { Output::One }),
            )
            .branch(endpoint(|| async move { Output::GT }));

        let dispatcher =
            entry().branch(negative_handler).branch(zero_handler).branch(positive_handler);

        assert_eq!(dispatcher.dispatch(deps![2]).await, ControlFlow::Break(Output::GT));
        assert_eq!(dispatcher.dispatch(deps![1]).await, ControlFlow::Break(Output::One));
        assert_eq!(dispatcher.dispatch(deps![0]).await, ControlFlow::Break(Output::Zero));
        assert_eq!(dispatcher.dispatch(deps![-1]).await, ControlFlow::Break(Output::MinusOne));
        assert_eq!(dispatcher.dispatch(deps![-2]).await, ControlFlow::Break(Output::LT));
    }
}
