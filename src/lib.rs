//! An implementation of the [chain (tree) of responsibility] pattern.
//!
//! ```
//! use dptree::prelude::*;
//!
//! type WebHandler = Endpoint<'static, DependencyMap, String>;
//!
//! #[rustfmt::skip]
//! #[tokio::main]
//! async fn main() {
//!     let web_server = dptree::entry()
//!         .branch(smiles_handler())
//!         .branch(sqrt_handler())
//!         .branch(not_found_handler());
//!     
//!     assert_eq!(
//!         web_server.dispatch(dptree::deps!["/smile"]).await,
//!         ControlFlow::Break("🙃".to_owned())
//!     );
//!     assert_eq!(
//!         web_server.dispatch(dptree::deps!["/sqrt 16"]).await,
//!         ControlFlow::Break("4".to_owned())
//!     );
//!     assert_eq!(
//!         web_server.dispatch(dptree::deps!["/lol"]).await,
//!         ControlFlow::Break("404 Not Found".to_owned())
//!     );
//! }
//!
//! fn smiles_handler() -> WebHandler {
//!     dptree::filter(|req: &'static str| async move { req.starts_with("/smile") })
//!         .endpoint(|| async { "🙃".to_owned() })
//! }
//!
//! fn sqrt_handler() -> WebHandler {
//!     dptree::filter_map(|req: &'static str| async move {
//!         if req.starts_with("/sqrt") {
//!             let (_, n) = req.split_once(' ')?;
//!             n.parse::<f64>().ok()
//!         } else {
//!             None
//!         }
//!     })
//!     .endpoint(|n: f64| async move { format!("{}", n.sqrt()) })
//! }
//!
//! fn not_found_handler() -> WebHandler {
//!     dptree::endpoint(|| async { "404 Not Found".to_owned() })
//! }
//! ```
//!
//! For a high-level overview, please see [`README.md`](https://github.com/p0lunin/dptree).
//!
//! [chain (tree) of responsibility]: https://en.wikipedia.org/wiki/Chain-of-responsibility_pattern

mod handler;

pub mod di;
pub mod guides;
pub mod prelude;

pub use handler::*;
