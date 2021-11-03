use crate::container::DiContainer;

use crate::{from_fn, Handler};
use futures::{future::BoxFuture, FutureExt};
use std::{convert::Infallible, future::Future, ops::ControlFlow, sync::Arc};

impl<'a, Input, Output, Intermediate> Handler<'a, Input, Output, Intermediate>
where
    Input: Send + Sync + 'a,
    Output: Send + Sync + 'a,
    Intermediate: Send + Sync + 'a,
{
    /// Chain self handler with `endpoint` handler.
    pub fn endpoint<F, FnArgs>(self, endp: F) -> Endpoint<'a, Input, Output>
    where
        F: IntoDiFn<Intermediate, Output, FnArgs>,
    {
        self.chain(endpoint(endp))
    }
}

/// Create endpoint handler.
///
/// Endpoint is a handler that _always_ break execution after its completion.
pub fn endpoint<'a, F, Input, Output, FnArgs>(f: F) -> Endpoint<'a, Input, Output>
where
    Input: Send + Sync + 'a,
    Output: Send + Sync + 'a,
    F: IntoDiFn<Input, Output, FnArgs>,
{
    let func = f.into();
    from_fn(move |x, _cont| {
        let func = func.clone();
        async move {
            let x = x;
            let func2 = func;
            let res = func2(&x).map(ControlFlow::Break).await;
            res
        }
    })
}

pub trait IntoDiFn<Input, Output, FnArgs> {
    fn into(self) -> DiFn<Input, Output>;
}

pub type Endpoint<'a, Input, Output> = Handler<'a, Input, Output, Infallible>;
pub type DiFn<Input, Output> =
    Arc<dyn for<'a> Fn(&'a Input) -> BoxFuture<'a, Output> + Send + Sync + 'static>;

macro_rules! impl_into_di {
    ($($generic:ident),*) => {
        impl<Func, Input, Output, Fut, $($generic),*> IntoDiFn<Input, Output, (Fut, $($generic),*)> for Func
        where
            Input: $(DiContainer<$generic> +)*,
            Func: Fn($(Arc<$generic>),*) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Output> + Send + Sync + 'static,
            $($generic: Send + Sync),*
        {
            #[allow(non_snake_case)]
            #[allow(unused_variables)]
            fn into(self) -> DiFn<Input, Output> {
                let this = Arc::new(self);
                Arc::new(move |input: &Input| {
                    let this = this.clone();
                    $(let $generic = input.get();)*
                    let fut = this( $( $generic ),* );
                    Box::pin(fut)
                })
            }
        }
    };
}

impl_into_di!();
impl_into_di!(A);
impl_into_di!(A, B);
impl_into_di!(A, B, C);
impl_into_di!(A, B, C, D);
impl_into_di!(A, B, C, D, E);
impl_into_di!(A, B, C, D, E, F);
impl_into_di!(A, B, C, D, E, F, G);
impl_into_di!(A, B, C, D, E, F, G, H);
impl_into_di!(A, B, C, D, E, F, G, H, I);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::TypeMapDi;

    #[tokio::test]
    async fn test_endpoint() {
        let input = 123;
        let output = 7;

        let mut store = TypeMapDi::new();
        store.insert(input);

        let result = endpoint(move |num: Arc<i32>| async move {
            assert_eq!(*num, input);
            output
        })
        .dispatch(store)
        .await;

        let result = match result {
            ControlFlow::Break(b) => b,
            _ => panic!("Unexpected: handler return ControlFlow::Break"),
        };
        assert_eq!(result, output);
    }
}
